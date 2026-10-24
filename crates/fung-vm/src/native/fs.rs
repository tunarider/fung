//! `_native.fs`: the file and directory
//! primitives `extras/fs.fung` wraps (06). Every operation that can fail
//! for outside reasons returns `Ok`/`Err({kind, message})`; wrong argument
//! types are E0403.
//!
//! Every native hands its system calls off in one pool job, with owned
//! paths and bytes only (a handle by a clone of its `Arc`); values are
//! made in the offload's `finish`, back on the run thread. A cancellation or
//! a deadline (spec 15):
//! - lets go of the natives that only read a path (`read`, `list`,
//!   `glob`, `walk`, `exists`, `stat`: `nat::offload_abandoning`): the
//!   call unwinds at once, the job's result is dropped when it ends;
//! - waits for the others once they run (`nat::offload`), since they
//!   change something (a file, a directory, a handle's position) that
//!   must not change after the call gave up.
//!
//! The reasons in the errors are `oserr`'s texts.

use std::fs::{DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use fung_front::source::decode_lossy;

use fung_front::diag::Arg;

use super::file::{self, Fault, File, Mode, NEW_FILE_MODE};
use super::glob;
use super::nat::{self, native_step, opt, param};
use super::oserr::{self, EEXIST, EINVAL, errno};
use super::path;
use crate::value::{Lit, MapBuilder, Module, Native, Value};
use crate::vm::{Flow, Stop, Task};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.fs` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            ("read", native_step("read", read_fn, vec![param("path")])),
            (
                "write",
                native_step("write", write_fn, vec![param("path"), param("text")]),
            ),
            (
                "append",
                native_step("append", append_fn, vec![param("path"), param("text")]),
            ),
            (
                "list",
                native_step(
                    "list",
                    list_fn,
                    vec![param("dir"), opt("full", Lit::Bool(false))],
                ),
            ),
            ("glob", native_step("glob", glob_fn, vec![param("pattern")])),
            ("walk", native_step("walk", walk_fn, vec![param("dir")])),
            (
                "exists",
                native_step("exists", exists_fn, vec![param("path")]),
            ),
            ("stat", native_step("stat", stat_fn, vec![param("path")])),
            ("mkdir", native_step("mkdir", mkdir_fn, vec![param("path")])),
            (
                "remove",
                native_step("remove", remove_fn, vec![param("path")]),
            ),
            (
                "remove_all",
                native_step("remove_all", remove_all_fn, vec![param("path")]),
            ),
            (
                "move",
                native_step(
                    "move",
                    move_fn,
                    vec![
                        param("from"),
                        param("to"),
                        opt("overwrite", Lit::Bool(false)),
                    ],
                ),
            ),
            (
                "copy",
                native_step("copy", copy_fn, vec![param("from"), param("to")]),
            ),
            (
                "open",
                native_step(
                    "open",
                    open_fn,
                    vec![param("path"), opt("mode", Lit::Sym("read"))],
                ),
            ),
            ("close", native_step("close", close_fn, vec![param("f")])),
            (
                "write_line",
                native_step("write_line", write_line_fn, vec![param("f"), param("line")]),
            ),
            ("temp_dir", native_step("temp_dir", temp_dir_fn, vec![])),
        ]
    });
    let mut m = Module::new("fs", "_native.fs");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

/// `Err({kind, message})` with ``cannot <verb> `<path>`: <reason>``.
pub(super) fn fs_err(verb: &str, path: &str, e: &io::Error) -> Value {
    nat::err_value(
        oserr::kind(e),
        &format!("cannot {verb} `{path}`: {}", oserr::reason(e)),
        Vec::new(),
    )
}

/// `Ok(nil)`, or the failure as `fs_err` reports it.
fn done(verb: &str, path: &str, r: io::Result<()>) -> Value {
    match r {
        Ok(()) => nat::ok(Value::Nil),
        Err(e) => fs_err(verb, path, &e),
    }
}

/// A path holding a NUL byte is refused with EINVAL before any system
/// call (the standard library's own error for it has no errno).
fn checked(p: &str) -> io::Result<&str> {
    if p.contains('\0') {
        Err(errno(EINVAL))
    } else {
        Ok(p)
    }
}

/// Creates or truncates with `perm`, then writes.
fn write_file(p: &str, bytes: &[u8], perm: u32) -> io::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(perm)
        .open(checked(p)?)?;
    f.write_all(bytes)
}

fn read_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let job = p.clone();
    nat::offload_abandoning(
        t,
        move || std::fs::read(checked(&job)?),
        move |_, r| {
            Ok(match r {
                Ok(bytes) => nat::ok(Value::str(&decode_lossy(&bytes))),
                Err(e) => fs_err("read", &p, &e),
            })
        },
    )
}

fn write_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let [p, text] = nat::str_args(&a, ["path", "text"])?;
    let (p, text) = (p.to_string(), text.as_bytes().to_vec());
    let job = p.clone();
    nat::offload(
        t,
        move || write_file(&job, &text, NEW_FILE_MODE),
        move |_, r| Ok(done("write", &p, r)),
    )
}

fn append_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let [p, text] = nat::str_args(&a, ["path", "text"])?;
    let (p, text) = (p.to_string(), text.as_bytes().to_vec());
    let job = p.clone();
    nat::offload(
        t,
        move || {
            OpenOptions::new()
                .append(true)
                .create(true)
                .mode(NEW_FILE_MODE)
                .open(checked(&job)?)?
                .write_all(&text)
        },
        move |_, r| Ok(done("append to", &p, r)),
    )
}

/// A fung list of the strings `paths` holds (file names need not be
/// UTF-8; a bad byte reads as U+FFFD).
fn str_list(paths: &[Vec<u8>]) -> Value {
    nat::list(paths.iter().map(|p| Value::str(&decode_lossy(p))).collect())
}

fn os_path(p: &[u8]) -> &std::ffi::OsStr {
    std::ffi::OsStr::from_bytes(p)
}

/// The directory's entries in byte order of their
/// names, each with whether it is a directory itself (a link is not).
fn read_dir(dir: &[u8]) -> io::Result<Vec<(Vec<u8>, bool)>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(os_path(dir))? {
        let e = e?;
        let is_dir = match e.file_type() {
            Ok(t) => t.is_dir(),
            // An entry gone before its type could be read is dropped.
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        out.push((e.file_name().as_bytes().to_vec(), is_dir));
    }
    out.sort_unstable();
    Ok(out)
}

fn list_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let dir = nat::need_str("dir", &a[0])?.to_string();
    let Value::Bool(full) = a[1] else {
        return Err(nat::wrong_arg("full", "bool", &a[1]));
    };
    let job = dir.clone();
    nat::offload_abandoning(
        t,
        move || {
            let names = read_dir(checked(&job)?.as_bytes())?.into_iter();
            Ok::<_, io::Error>(if full {
                names
                    .map(|(n, _)| path::join_bytes(job.as_bytes(), &n))
                    .collect::<Vec<_>>()
            } else {
                names.map(|(n, _)| n).collect()
            })
        },
        move |_, r| {
            Ok(match r {
                Ok(names) => nat::ok(str_list(&names)),
                Err(e) => fs_err("list", &dir, &e),
            })
        },
    )
}

fn glob_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let pattern = nat::need_str("pattern", &a[0])?.to_string();
    let job = pattern.clone();
    nat::offload_abandoning(
        t,
        move || glob::glob(job.as_bytes()),
        move |_, r| {
            Ok(match r {
                Ok(mut found) => {
                    // The whole result in byte order, across directories too.
                    found.sort_unstable();
                    nat::ok(str_list(&found))
                }
                Err(glob::BadPattern) => nat::err_value(
                    "invalid",
                    &format!("bad glob pattern `{pattern}`: syntax error in pattern"),
                    Vec::new(),
                ),
            })
        },
    )
}

fn walk_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let dir = nat::need_str("dir", &a[0])?.to_string();
    nat::offload_abandoning(
        t,
        move || match checked(&dir) {
            Ok(d) => walk(d.as_bytes()),
            Err(e) => Err((dir.into_bytes(), e)),
        },
        |_, r| {
            Ok(match r {
                Ok(files) => nat::ok(str_list(&files)),
                Err((at, e)) => fs_err("walk", &String::from_utf8_lossy(&at), &e),
            })
        },
    )
}

/// Every file below `root` (not the directories), depth first in byte
/// order, as paths joined onto `root`. Links are not followed (a link is
/// listed like a file), except a root the system resolves itself
/// (`link/`). A directory that cannot be read ends the walk with its
/// error and its path.
fn walk(root: &[u8]) -> Result<Vec<Vec<u8>>, (Vec<u8>, io::Error)> {
    let mut files = Vec::new();
    match std::fs::symlink_metadata(os_path(root)) {
        Ok(m) if m.is_dir() => walk_dir(root, &mut files)?,
        Ok(_) => files.push(root.to_vec()),
        Err(e) => return Err((root.to_vec(), e)),
    }
    Ok(files)
}

fn walk_dir(dir: &[u8], files: &mut Vec<Vec<u8>>) -> Result<(), (Vec<u8>, io::Error)> {
    for (name, is_dir) in read_dir(dir).map_err(|e| (dir.to_vec(), e))? {
        let p = path::join_bytes(dir, &name);
        if is_dir {
            walk_dir(&p, files)?;
        } else {
            files.push(p);
        }
    }
    Ok(())
}

fn exists_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    nat::offload_abandoning(
        t,
        move || checked(&p).and_then(std::fs::metadata).is_ok(),
        |_, found| Ok(Value::Bool(found)),
    )
}

fn stat_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let job = p.clone();
    nat::offload_abandoning(
        t,
        move || std::fs::metadata(checked(&job)?).map(|m| (m.size(), m.is_dir())),
        move |_, r| {
            let (size, is_dir) = match r {
                Ok(info) => info,
                Err(e) => return Ok(fs_err("stat", &p, &e)),
            };
            // The name is the last element of the path asked for.
            let mut m = MapBuilder::default();
            m.set(Rc::new("name".into()), Value::str(&path::base(&p)));
            m.set(Rc::new("size".into()), Value::Int(size as i64));
            m.set(Rc::new("is_dir".into()), Value::Bool(is_dir));
            Ok(nat::ok(Value::Map(Rc::new(m.finish()))))
        },
    )
}

fn mkdir_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let job = p.clone();
    nat::offload(
        t,
        move || mkdir_all(checked(&job)?, 0o755),
        move |_, r| Ok(done("create directory", &p, r)),
    )
}

/// A directory and the missing ones above it; one already there is
/// success, a file there is "file exists", and a file above it is "not a
/// directory". The empty path names nothing.
fn mkdir_all(p: &str, perm: u32) -> io::Result<()> {
    if p.is_empty() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    DirBuilder::new().recursive(true).mode(perm).create(p)
}

fn remove_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let job = p.clone();
    nat::offload(
        t,
        move || remove(checked(&job)?),
        move |_, r| Ok(done("remove", &p, r)),
    )
}

/// A file, a link or an empty directory.
fn remove(p: &str) -> io::Result<()> {
    if std::fs::symlink_metadata(p)?.is_dir() {
        std::fs::remove_dir(p)
    } else {
        std::fs::remove_file(p)
    }
}

fn remove_all_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let job = p.clone();
    nat::offload(
        t,
        move || remove_all(checked(&job)?),
        move |_, r| Ok(done("remove", &p, r)),
    )
}

/// A path and everything below it. Nothing to do for `""` or a missing
/// path; a root, or a path whose last part is `.` or `..`, is refused (it
/// would name more than the path says). A link is removed itself, also
/// when given with a trailing `/`.
fn remove_all(p: &str) -> io::Result<()> {
    if p.is_empty() {
        return Ok(());
    }
    let p = p.trim_end_matches('/');
    let last = p.rsplit('/').next().unwrap_or("");
    if p.is_empty() || last == "." || last == ".." {
        return Err(errno(EINVAL));
    }
    match std::fs::symlink_metadata(p) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(m) if m.is_dir() => match std::fs::remove_dir_all(p) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            r => r,
        },
        Ok(_) => std::fs::remove_file(p),
    }
}

fn move_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let [from, to] = nat::str_args(&a, ["from", "to"])?;
    let Value::Bool(overwrite) = a[2] else {
        return Err(nat::wrong_arg("overwrite", "bool", &a[2]));
    };
    let (from, to) = (from.to_string(), to.to_string());
    let job = (from.clone(), to.clone());
    nat::offload(
        t,
        move || rename(checked(&job.0)?, checked(&job.1)?, overwrite),
        move |_, r| {
            Ok(match r {
                // The target is the path that exists: name both.
                Err(e) if e.raw_os_error() == Some(EEXIST) => nat::err_value(
                    oserr::kind(&e),
                    &format!("cannot move `{from}` to `{to}`: {}", oserr::reason(&e)),
                    Vec::new(),
                ),
                r => done("move", &from, r),
            })
        },
    )
}

/// `move` never replaces an existing directory: onto one, only the same
/// directory may go (a case-only rename); anything else is "file exists",
/// after the source's own error. A file is replaced only with
/// `overwrite`; otherwise the move is the system's no-replace rename, so
/// a file made between a check and the move is not lost either. The same
/// file under another name (a case-only rename on a case-blind file
/// system) is no replacement.
fn rename(from: &str, to: &str, overwrite: bool) -> io::Result<()> {
    if let Ok(target) = std::fs::symlink_metadata(to) {
        let source = std::fs::symlink_metadata(from)?;
        let same = source.dev() == target.dev() && source.ino() == target.ino();
        if target.is_dir() && (from == to || !same) {
            return Err(errno(EEXIST));
        }
        if same {
            return std::fs::rename(from, to);
        }
    }
    if overwrite {
        std::fs::rename(from, to)
    } else {
        rename_noreplace(from, to)
    }
}

/// A rename that fails with EEXIST when `to` exists, decided by the
/// system in one step. A file system without it (EINVAL, ENOTSUP, or no
/// such call) falls back to a hard link, which fails the same way, and
/// last to a check before the rename (the one step that can race).
fn rename_noreplace(from: &str, to: &str) -> io::Result<()> {
    match noreplace_call(from, to) {
        Some(Err(e)) if is_unsupported(&e) => {}
        Some(r) => return r,
        None => {}
    }
    if !std::fs::symlink_metadata(from)?.is_dir() {
        match std::fs::hard_link(from, to) {
            Ok(()) => return std::fs::remove_file(from),
            Err(e) if e.raw_os_error() == Some(EEXIST) => return Err(e),
            Err(_) => {}
        }
    }
    if std::fs::symlink_metadata(to).is_ok() {
        return Err(errno(EEXIST));
    }
    std::fs::rename(from, to)
}

fn is_unsupported(e: &io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(libc::EINVAL) | Some(libc::ENOTSUP) | Some(libc::ENOSYS)
    )
}

/// The system's own no-replace rename, or `None` where there is none.
#[cfg(target_os = "linux")]
fn noreplace_call(from: &str, to: &str) -> Option<io::Result<()>> {
    let (from, to) = (c_path(from), c_path(to));
    // SAFETY: two NUL-terminated paths that live across the call;
    // AT_FDCWD resolves them like `rename` does.
    let rc = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    Some(if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    })
}

#[cfg(target_os = "macos")]
fn noreplace_call(from: &str, to: &str) -> Option<io::Result<()>> {
    let (from, to) = (c_path(from), c_path(to));
    // SAFETY: two NUL-terminated paths that live across the call.
    let rc = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    Some(if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn noreplace_call(_: &str, _: &str) -> Option<io::Result<()>> {
    None
}

/// A path for a system call (`checked` refused a NUL already).
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn c_path(p: &str) -> std::ffi::CString {
    std::ffi::CString::new(p).expect("checked: no NUL")
}

fn copy_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let [from, to] = nat::str_args(&a, ["from", "to"])?;
    let (from, to) = (from.to_string(), to.to_string());
    let job = (from.clone(), to.clone());
    nat::offload(
        t,
        move || copy(&job.0, &job.1),
        move |_, r| {
            Ok(match r {
                Ok(()) => nat::ok(Value::Nil),
                Err((true, e)) => fs_err("copy", &from, &e),
                Err((false, e)) => fs_err("copy to", &to, &e),
            })
        },
    )
}

/// The source's permission bits, its bytes, then a write.
/// The error says which side failed (`true` for the source).
fn copy(from: &str, to: &str) -> Result<(), (bool, io::Error)> {
    let src = |e| (true, e);
    let from = checked(from).map_err(src)?;
    let perm = std::fs::metadata(from).map_err(src)?.permissions().mode() & 0o777;
    let bytes = std::fs::read(from).map_err(src)?;
    write_file(to, &bytes, perm).map_err(|e| (false, e))
}

const MODES: &str = "'read | 'write | 'append";

/// `Ok(<file>)` or the open failure.
fn open_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let Value::Sym(mode) = a[1] else {
        return Err(nat::wrong_arg("mode", MODES, &a[1]));
    };
    let mode = match mode.name() {
        "read" => Mode::Read,
        "write" => Mode::Write,
        "append" => Mode::Append,
        other => {
            return Err(nat::fail(
                "E0403",
                vec![nat::s("mode"), nat::s(MODES), Arg::Str(format!("'{other}"))],
            ));
        }
    };
    let job = p.clone();
    nat::offload(
        t,
        move || file::open(checked(&job)?, mode),
        move |_, r| {
            Ok(match r {
                Ok(io) => nat::ok(Value::File(Arc::new(File::new(p, io)))),
                Err(e) => fs_err("open", &p, &e),
            })
        },
    )
}

/// The file handle argument `name`, or E0403.
pub(crate) fn need_file<'a>(name: &str, v: &'a Value) -> Result<&'a Arc<File>, Stop> {
    match v {
        Value::File(f) => Ok(f),
        _ => Err(nat::wrong_arg(name, "file", v)),
    }
}

/// The `Err` of a failed handle operation: closed, the `'invalid`
/// direction errors, or the OS error with `verb`.
pub(crate) fn fault_value(f: &File, verb: &str, e: Fault) -> Value {
    let p = &f.path;
    match e {
        Fault::Closed => nat::err_value("closed", &format!("file `{p}` is closed"), Vec::new()),
        Fault::NotReading => nat::err_value(
            "invalid",
            &format!("file `{p}` is not open for reading"),
            Vec::new(),
        ),
        Fault::NotWriting => nat::err_value(
            "invalid",
            &format!("file `{p}` is not open for writing"),
            Vec::new(),
        ),
        Fault::Io(e) => fs_err(verb, p, &e),
    }
}

/// `fs.close`. The flush and the close run on a pool thread.
fn close_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let f = need_file("f", &a[0])?.clone();
    let job = f.clone();
    nat::offload(
        t,
        move || job.close(),
        move |_, r| {
            Ok(match r {
                Ok(()) => nat::ok(Value::Nil),
                Err(e) => fault_value(&f, "close", e),
            })
        },
    )
}

/// `fs.write_line`: the line and its `\n` go in under the handle's
/// lock: here when they fit in the write buffer, else on a pool thread.
fn write_line_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let f = need_file("f", &a[0])?.clone();
    let line = nat::need_str("line", &a[1])?;
    let mut bytes = Vec::with_capacity(line.len() + 1);
    bytes.extend_from_slice(line.as_bytes());
    bytes.push(b'\n');
    if let Some(r) = f.write_buffered_line(&bytes) {
        return Ok(Flow::Done(written_value(&f, r)));
    }
    let job = f.clone();
    nat::offload(
        t,
        move || job.write_line(&bytes),
        move |_, r| Ok(written_value(&f, r)),
    )
}

/// A handle's `write_line` result (or its fault) as `fs.write_line` gives it.
fn written_value(f: &File, r: Result<(), Fault>) -> Value {
    match r {
        Ok(()) => nat::ok(Value::Nil),
        Err(e) => fault_value(f, "write", e),
    }
}

fn temp_dir_fn(t: &mut Task<'_>, _: Vec<Value>) -> Result<Flow, Stop> {
    let base = temp_base();
    let job = base.clone();
    nat::offload(
        t,
        move || make_temp_dir_in(&job),
        move |_, made| {
            Ok(match made {
                Ok(dir) => nat::ok(Value::str(&decode_lossy(&dir))),
                Err(e) => temp_dir_failure(e, &base),
            })
        },
    )
}

#[derive(Debug)]
pub(super) enum TempDirError {
    Os(io::Error),
    Taken,
}

/// The `Err` for a temporary directory not made in `base`.
pub(super) fn temp_dir_failure(e: TempDirError, base: &[u8]) -> Value {
    let verb = "create temporary directory in";
    let base = String::from_utf8_lossy(base);
    match e {
        TempDirError::Os(e) => fs_err(verb, &base, &e),
        TempDirError::Taken => nat::err_value(
            "exists",
            &format!("cannot {verb} `{base}`: every name tried exists"),
            Vec::new(),
        ),
    }
}

/// Where temporary directories go: `$TMPDIR` (`/tmp` when unset or
/// empty), ending in `/`.
fn temp_base() -> Vec<u8> {
    let mut base = std::env::var_os("TMPDIR")
        .filter(|d| !d.is_empty())
        .map_or_else(|| b"/tmp".to_vec(), |d| d.as_bytes().to_vec());
    if base.last() != Some(&b'/') {
        base.push(b'/');
    }
    base
}

/// A new directory in `base`: `fung-` and a random 32-bit number in
/// decimal, made with mode 0700; a name already taken is tried again,
/// 10,000 times.
pub(super) fn make_temp_dir_in(base: &[u8]) -> Result<Vec<u8>, TempDirError> {
    let mut prefix = base.to_vec();
    prefix.extend_from_slice(b"fung-");
    for _ in 0..10_000 {
        let mut name = prefix.clone();
        name.extend_from_slice(random_u32().to_string().as_bytes());
        let os = std::ffi::OsStr::from_bytes(&name);
        match DirBuilder::new().mode(0o700).create(os) {
            Ok(()) => return Ok(name),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(TempDirError::Os(e)),
        }
    }
    Err(TempDirError::Taken)
}

/// A random 32-bit number from the standard library's per-hasher keys.
fn random_u32() -> u32 {
    use std::hash::{BuildHasher, RandomState};
    RandomState::new().hash_one(std::time::SystemTime::now()) as u32
}

#[cfg(test)]
#[path = "fs_tests.rs"]
mod tests;
