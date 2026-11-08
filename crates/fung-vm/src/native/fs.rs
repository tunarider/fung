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
//! `exists` and `stat` of a task alone (`vm::alone`: no other task held
//! back, no deadline to keep) call the system on the run thread instead
//! (`nat::offload_short`, P8): the pool's round trip costs several times
//! the call itself.
//!
//! The reasons in the errors are `oserr`'s texts.

use std::fs::{DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use fung_front::diag::Arg;
use fung_front::text::lossy as show;

use super::file::{self, Fault, File, Io, Mode, os_path};
use super::glob;
use super::nat::{self, native_step, opt, param};
use super::oserr::{self, EEXIST, EINVAL, errno};
use super::path;
use super::pipe::{self, Pipe, Step};
use crate::value::{Lit, MapBuilder, Module, Native, Value};
use crate::vm::{Flow, Stop, Task};
use crate::vm::{IoStep, Kind, park_io};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.fs` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            ("read", native_step("read", read_fn, vec![param("path")])),
            (
                "write",
                native_step(
                    "write",
                    write_fn,
                    vec![param("path"), param("text"), opt("perm", Lit::Nil)],
                ),
            ),
            (
                "append",
                native_step(
                    "append",
                    append_fn,
                    vec![param("path"), param("text"), opt("perm", Lit::Nil)],
                ),
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
                native_step(
                    "copy",
                    copy_fn,
                    vec![
                        param("from"),
                        param("to"),
                        opt("overwrite", Lit::Bool(false)),
                    ],
                ),
            ),
            (
                "open",
                native_step(
                    "open",
                    open_fn,
                    vec![
                        param("path"),
                        opt("mode", Lit::Sym("read")),
                        opt("perm", Lit::Nil),
                    ],
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
pub(super) fn fs_err(verb: &str, path: &[u8], e: &io::Error) -> Value {
    nat::err_value(
        oserr::kind(e),
        &format!("cannot {verb} `{}`: {}", show(path), oserr::reason(e)),
        Vec::new(),
    )
}

/// `Ok(nil)`, or the failure as `fs_err` reports it.
fn done(verb: &str, path: &[u8], r: io::Result<()>) -> Value {
    match r {
        Ok(()) => nat::ok(Value::Nil),
        Err(e) => fs_err(verb, path, &e),
    }
}

/// A path holding a NUL byte is refused with EINVAL before any system
/// call (the standard library's own error for it has no errno).
fn checked(p: &[u8]) -> io::Result<&std::ffi::OsStr> {
    if p.contains(&0) {
        Err(errno(EINVAL))
    } else {
        Ok(os_path(p))
    }
}

/// The `perm` argument (06, F-14): nil, or the permission bits 0 to
/// 0o777 a written file gets (`file::open_writing`).
fn perm_arg(v: &Value) -> Result<Option<u32>, Stop> {
    match v {
        Value::Nil => Ok(None),
        Value::Int(n @ 0..=0o777) => Ok(Some(*n as u32)),
        Value::Int(n) => Err(nat::fail(
            "E0403",
            vec![
                nat::s("perm"),
                nat::s("an int from 0 to 0o777"),
                Arg::Str(octal(*n)),
            ],
        )),
        v => Err(nat::wrong_arg("perm", "int | nil", v)),
    }
}

/// `n` as permission bits are written: `0o1000`, or `-1`.
fn octal(n: i64) -> String {
    if n < 0 {
        n.to_string()
    } else {
        format!("0o{n:o}")
    }
}

/// Opens with `o` (and `perm`), writes `text` and closes, reporting the
/// close (L11): `fs.write` and `fs.append`.
fn write_closed(o: &mut OpenOptions, p: &[u8], perm: Option<u32>, text: &[u8]) -> io::Result<()> {
    let mut f = file::open_writing(o, checked(p)?, perm)?;
    f.write_all(text)?;
    file::close_checked(f, p)
}

/// Writes `text` to the regular file `path` whole or not at all (LB17;
/// `fs.write` and `fung fmt`): into a temporary file beside it, renamed
/// over it, so a crash, a full disk or a failing close never leaves half
/// a file and a reader sees the old text or the new. A link is followed
/// (the file it names is replaced, the link stays). Without `perm` an
/// existing file keeps its permission bits and a new one gets the
/// default less the umask; with it, exactly `perm` (F-14).
///
/// Where a new file would not be the same file to its users, it is
/// written in place instead (truncated, as `write_closed`): another hard
/// link (the other names would keep the old text), an owner or group the
/// new file would not get (someone else's file we may write), a file that
/// is not regular (a device; a FIFO never gets here), a link that names
/// nothing yet, or a directory where no temporary file can be made. A
/// file we may not write is refused as before, not replaced. Once the
/// temporary file is made, a failure is reported and the file is left as
/// it was.
pub fn write_whole(path: &[u8], text: &[u8], perm: Option<u32>) -> io::Result<()> {
    let p = checked(path)?;
    let in_place = || {
        let mut o = OpenOptions::new();
        o.write(true).create(true).truncate(true);
        write_closed(&mut o, path, perm, text)
    };
    let (target, existing) = match std::fs::canonicalize(p) {
        Ok(t) => {
            let m = std::fs::metadata(&t)?;
            let writable = OpenOptions::new().write(true).open(&t).is_ok();
            if !m.is_file() || m.nlink() != 1 || !writable {
                return in_place();
            }
            (t, Some(m))
        }
        // a link to nothing yet: the open creates the file it names
        Err(_) if std::fs::symlink_metadata(p).is_ok() => return in_place(),
        Err(_) => (std::path::PathBuf::from(p), None),
    };
    let (Some(dir), Some(name)) = (target.parent(), target.file_name()) else {
        return in_place();
    };
    let mode = perm.or(existing.as_ref().map(|m| m.permissions().mode() & 0o777));
    let (tmp, f) = match temp_beside(dir, name, mode.unwrap_or(file::NEW_FILE_MODE)) {
        Some(made) => made,
        None => return in_place(),
    };
    if let Some(m) = &existing
        && !f
            .metadata()
            .is_ok_and(|t| t.uid() == m.uid() && t.gid() == m.gid())
    {
        drop(f);
        let _ = std::fs::remove_file(&tmp);
        return in_place();
    }
    let written = (|| {
        let mut f = f;
        if let Some(mode) = mode {
            f.set_permissions(std::fs::Permissions::from_mode(mode))?;
        }
        f.write_all(text)?;
        sync(&f)?;
        file::close_checked(f, path)?;
        std::fs::rename(&tmp, &target)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

/// `fsync(2)`: the new text is on the disk before the rename makes it
/// the file's, so a power cut cannot leave the new name with lost text.
/// The plain system call, also on macOS (where the standard library's
/// `sync_all` asks the drive to empty its cache too, `F_FULLFSYNC`, at
/// about 10 ms a file).
fn sync(f: &std::fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: `f` owns an open descriptor for the whole call.
    if unsafe { libc::fsync(f.as_raw_fd()) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// A new temporary file `.<name>.<pid>-<n>.tmp` in `dir`, made with
/// `mode` (less the umask), or `None` when none can be made there.
fn temp_beside(
    dir: &std::path::Path,
    name: &std::ffi::OsStr,
    mode: u32,
) -> Option<(std::path::PathBuf, std::fs::File)> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    // a name a crashed run left behind is passed over
    for _ in 0..16 {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(format!(".{}-{n}.tmp", std::process::id()));
        let tmp = dir.join(tmp_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp)
        {
            Ok(f) => return Some((tmp, f)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

fn read_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_bytes("path", &a[0])?.to_vec();
    let job = p.clone();
    // A FIFO is read in the event loop (`pipe.rs`, L11); `None` says so.
    let read = move || -> io::Result<Option<Vec<u8>>> {
        let path = checked(&job)?;
        if pipe::is_fifo(&job) {
            return Ok(None);
        }
        std::fs::read(path).map(Some)
    };
    crate::vm::offload_kind(t, Kind::Abandon, read, move |t, r| match r {
        Ok(Some(bytes)) => Ok(Flow::Done(nat::ok(Value::bytes(bytes)))),
        Ok(None) => read_fifo(t, p),
        Err(e) => Ok(Flow::Done(fs_err("read", &p, &e))),
    })
}

/// `fs.read` of a FIFO: its read end, read to the end in the event loop.
fn read_fifo(t: &mut Task<'_>, p: Vec<u8>) -> Result<Flow, Stop> {
    let mut r = match Pipe::open_read(&p) {
        Ok(r) => r,
        Err(e) => return Ok(Flow::Done(fs_err("read", &p, &e))),
    };
    park_io(
        t,
        move |_, _| match r.read_to_end() {
            Step::Ready(bytes) => IoStep::Done(Ok(nat::ok(Value::bytes(bytes)))),
            Step::Wait(fd, interest) => pipe::io_step(fd, interest),
            Step::Fail(e) => IoStep::Done(Ok(fs_err("read", &p, &e))),
        },
        |_| {},
    )
}

/// `fs.write` and `fs.append` of a FIFO: once a reader has it open, the
/// text is sent in the event loop and the write end closed.
fn write_fifo(
    t: &mut Task<'_>,
    p: Vec<u8>,
    text: Vec<u8>,
    perm: Option<u32>,
    verb: &'static str,
) -> Result<Flow, Stop> {
    let mut w: Option<Pipe> = None;
    let mut text = Some(text);
    park_io(
        t,
        move |_, _| {
            if w.is_none() {
                match Pipe::open_write(&p, perm) {
                    Ok(Some(mut opened)) => {
                        opened.queue(&text.take().expect("queued once"));
                        w = Some(opened);
                    }
                    Ok(None) => return pipe::until_reader(),
                    Err(e) => return IoStep::Done(Ok(fs_err(verb, &p, &e))),
                }
            }
            let pending = w.as_mut().expect("opened");
            match pending.flush() {
                Step::Ready(()) => {
                    let sent = w.take().expect("opened");
                    IoStep::Done(Ok(done(verb, &p, sent.finish(&p))))
                }
                Step::Wait(fd, interest) => pipe::io_step(fd, interest),
                Step::Fail(e) => IoStep::Done(Ok(fs_err(verb, &p, &e))),
            }
        },
        |_| {},
    )
}

fn write_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let [p, text] = nat::bytes_args(&a, ["path", "text"])?;
    let perm = perm_arg(&a[2])?;
    let (p, text) = (p.to_vec(), text.to_vec());
    let job = p.clone();
    let write = move || -> io::Result<Option<Vec<u8>>> {
        checked(&job)?;
        if pipe::is_fifo(&job) {
            return Ok(Some(text));
        }
        write_whole(&job, &text, perm).map(|()| None)
    };
    crate::vm::offload_kind(t, Kind::Wait, write, move |t, r| match r {
        Ok(Some(text)) => write_fifo(t, p, text, perm, "write"),
        r => Ok(Flow::Done(done("write", &p, r.map(|_| ())))),
    })
}

fn append_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let [p, text] = nat::bytes_args(&a, ["path", "text"])?;
    let perm = perm_arg(&a[2])?;
    let (p, text) = (p.to_vec(), text.to_vec());
    let job = p.clone();
    let append = move || -> io::Result<Option<Vec<u8>>> {
        checked(&job)?;
        if pipe::is_fifo(&job) {
            return Ok(Some(text));
        }
        let mut o = OpenOptions::new();
        o.append(true).create(true);
        write_closed(&mut o, &job, perm, &text).map(|()| None)
    };
    crate::vm::offload_kind(t, Kind::Wait, append, move |t, r| match r {
        Ok(Some(text)) => write_fifo(t, p, text, perm, "append to"),
        r => Ok(Flow::Done(done("append to", &p, r.map(|_| ())))),
    })
}

/// A fung list of the strings `paths` holds, byte for byte (file names
/// need not be UTF-8, L4).
fn str_list(paths: &[Vec<u8>]) -> Value {
    nat::list(paths.iter().map(|p| Value::bytes(p.clone())).collect())
}

/// The directory's entries in byte order of their
/// names, each with whether it is a directory itself (a link is not).
/// An entry gone before its type could be read is no entry. `fs.list`,
/// `fs.walk` and the files `fung test`·`fung fmt` choose.
pub fn read_dir(dir: &[u8]) -> io::Result<Vec<(Vec<u8>, bool)>> {
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
    let dir = nat::need_bytes("dir", &a[0])?.to_vec();
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
                    .map(|(n, _)| path::join_bytes(&job, &n))
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
    let pattern = nat::need_bytes("pattern", &a[0])?.to_vec();
    let job = pattern.clone();
    nat::offload_abandoning(
        t,
        move || glob::glob(&job),
        move |_, r| {
            Ok(match r {
                Ok(mut found) => {
                    // The whole result in byte order, across directories too.
                    found.sort_unstable();
                    nat::ok(str_list(&found))
                }
                // A pattern is code: a bad one is a code error (LB9), as
                // a bad regex pattern is.
                Err(glob::Failure::Path(at, e)) => fs_err("glob", &at, &e),
                Err(glob::Failure::Pattern) => {
                    return Err(nat::fail(
                        "E0515",
                        vec![Arg::Str(format!(
                            "`{}`: a glob syntax error",
                            show(&pattern)
                        ))],
                    ));
                }
            })
        },
    )
}

fn walk_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let dir = nat::need_bytes("dir", &a[0])?.to_vec();
    nat::offload_abandoning(
        t,
        move || match checked(&dir) {
            Ok(d) => walk(d.as_bytes()),
            Err(e) => Err((dir, e)),
        },
        |_, r| {
            Ok(match r {
                Ok(files) => nat::ok(str_list(&files)),
                Err((at, e)) => fs_err("walk", &at, &e),
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
    let p = nat::need_bytes("path", &a[0])?.to_vec();
    nat::offload_short(
        t,
        move || checked(&p).and_then(std::fs::metadata).is_ok(),
        |_, found| Ok(Value::Bool(found)),
    )
}

fn stat_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_bytes("path", &a[0])?.to_vec();
    let job = p.clone();
    nat::offload_short(
        t,
        move || {
            std::fs::metadata(checked(&job)?).map(|m| (m.size(), m.is_dir(), m.modified().ok()))
        },
        move |_, r| {
            let (size, is_dir, modified) = match r {
                Ok(info) => info,
                Err(e) => return Ok(fs_err("stat", &p, &e)),
            };
            // The name is the last element of the path asked for.
            let mut m = MapBuilder::default();
            m.set(crate::value::key("name"), Value::bytes(path::base(&p)));
            m.set(crate::value::key("size"), Value::Int(size as i64));
            m.set(crate::value::key("is_dir"), Value::Bool(is_dir));
            // nil where the system keeps no time, or one past year 9999
            let mtime = modified.and_then(super::time::from_system);
            m.set(crate::value::key("mtime"), mtime.unwrap_or(Value::Nil));
            Ok(nat::ok(Value::Map(Rc::new(m.finish()))))
        },
    )
}

fn mkdir_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_bytes("path", &a[0])?.to_vec();
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
fn mkdir_all(p: &std::ffi::OsStr, perm: u32) -> io::Result<()> {
    if p.is_empty() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    DirBuilder::new().recursive(true).mode(perm).create(p)
}

fn remove_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_bytes("path", &a[0])?.to_vec();
    let job = p.clone();
    nat::offload(
        t,
        move || remove(checked(&job)?),
        move |_, r| Ok(done("remove", &p, r)),
    )
}

/// A file, a link or an empty directory.
fn remove(p: &std::ffi::OsStr) -> io::Result<()> {
    if std::fs::symlink_metadata(p)?.is_dir() {
        std::fs::remove_dir(p)
    } else {
        std::fs::remove_file(p)
    }
}

fn remove_all_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_bytes("path", &a[0])?.to_vec();
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
fn remove_all(p: &std::ffi::OsStr) -> io::Result<()> {
    if p.is_empty() {
        return Ok(());
    }
    let p = p.as_bytes();
    let p = &p[..p.iter().rposition(|&b| b != b'/').map_or(0, |i| i + 1)];
    let last = p.rsplit(|&b| b == b'/').next().unwrap_or(b"");
    if p.is_empty() || last == b"." || last == b".." {
        return Err(errno(EINVAL));
    }
    let p = os_path(p);
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
    let [from, to] = nat::bytes_args(&a, ["from", "to"])?;
    let Value::Bool(overwrite) = a[2] else {
        return Err(nat::wrong_arg("overwrite", "bool", &a[2]));
    };
    let (from, to) = (from.to_vec(), to.to_vec());
    let job = (from.clone(), to.clone());
    nat::offload(
        t,
        move || rename(checked(&job.0)?, checked(&job.1)?, overwrite),
        move |_, r| {
            Ok(match r {
                // The target is the path that exists: name both.
                Err(e) if e.raw_os_error() == Some(EEXIST) => nat::err_value(
                    oserr::kind(&e),
                    &format!(
                        "cannot move `{}` to `{}`: {}",
                        show(&from),
                        show(&to),
                        oserr::reason(&e)
                    ),
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
fn rename(from: &std::ffi::OsStr, to: &std::ffi::OsStr, overwrite: bool) -> io::Result<()> {
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
    let r = if overwrite {
        std::fs::rename(from, to)
    } else {
        rename_noreplace(from, to)
    };
    match r {
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => move_across(from, to, overwrite),
        r => r,
    }
}

/// A move to another file system (EXDEV, LB8), where no rename reaches:
/// a regular file is copied under the same replace rule and the source
/// removed, as the shell's `mv` does. A directory or a link stays EXDEV.
pub(super) fn move_across(
    from: &std::ffi::OsStr,
    to: &std::ffi::OsStr,
    overwrite: bool,
) -> io::Result<()> {
    if !std::fs::symlink_metadata(from)?.is_file() {
        return Err(errno(libc::EXDEV));
    }
    copy_paths(from, to, overwrite).map_err(|e| match e {
        CopyError::Source(e) | CopyError::Target(e) => e,
        CopyError::Exists => errno(EEXIST),
        CopyError::NotRegular(_) => errno(libc::EXDEV),
    })?;
    std::fs::remove_file(from)
}

/// A rename that fails with EEXIST when `to` exists, decided by the
/// system in one step. A file system without it (EINVAL, ENOTSUP, or no
/// such call) falls back to a hard link, which fails the same way, and
/// last to a check before the rename (the one step that can race).
fn rename_noreplace(from: &std::ffi::OsStr, to: &std::ffi::OsStr) -> io::Result<()> {
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
/// On Linux it is the system call itself, not the C library's
/// `renameat2`: the musl that Rust links statically has no such function
/// (glibc has had it since 2.28), while every kernel since 3.15 has the
/// call; an older kernel answers ENOSYS and the fallback runs.
#[cfg(target_os = "linux")]
fn noreplace_call(from: &std::ffi::OsStr, to: &std::ffi::OsStr) -> Option<io::Result<()>> {
    let (from, to) = (c_path(from), c_path(to));
    // SAFETY: renameat2(2) takes two directory descriptors, two
    // NUL-terminated paths that live across the call and the flags;
    // AT_FDCWD resolves the paths like `rename` does.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
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
fn noreplace_call(from: &std::ffi::OsStr, to: &std::ffi::OsStr) -> Option<io::Result<()>> {
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
fn noreplace_call(_: &std::ffi::OsStr, _: &std::ffi::OsStr) -> Option<io::Result<()>> {
    None
}

/// A path for a system call (`checked` refused a NUL already).
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn c_path(p: &std::ffi::OsStr) -> std::ffi::CString {
    std::ffi::CString::new(p.as_bytes()).expect("checked: no NUL")
}

fn copy_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let [from, to] = nat::bytes_args(&a, ["from", "to"])?;
    let Value::Bool(overwrite) = a[2] else {
        return Err(nat::wrong_arg("overwrite", "bool", &a[2]));
    };
    let (from, to) = (from.to_vec(), to.to_vec());
    let job = (from.clone(), to.clone());
    nat::offload(
        t,
        move || copy(&job.0, &job.1, overwrite),
        move |_, r| {
            Ok(match r {
                Ok(()) => nat::ok(Value::Nil),
                Err(CopyError::Source(e)) => fs_err("copy", &from, &e),
                Err(CopyError::Target(e)) => fs_err("copy to", &to, &e),
                // The target is the path that exists: name both, as move.
                Err(CopyError::Exists) => nat::err_value(
                    "exists",
                    &format!(
                        "cannot copy `{}` to `{}`: file exists",
                        show(&from),
                        show(&to)
                    ),
                    Vec::new(),
                ),
                Err(CopyError::NotRegular(source)) => nat::err_value(
                    "invalid",
                    &if source {
                        format!("cannot copy `{}`: not a regular file", show(&from))
                    } else {
                        format!("cannot copy to `{}`: not a regular file", show(&to))
                    },
                    Vec::new(),
                ),
            })
        },
    )
}

/// Why a copy failed, and on which side.
enum CopyError {
    Source(io::Error),
    Target(io::Error),
    /// The target exists and `overwrite` is false (LB7).
    Exists,
    /// A FIFO, device or socket: `true` for the source (CC3).
    NotRegular(bool),
}

fn copy(from: &[u8], to: &[u8], overwrite: bool) -> Result<(), CopyError> {
    let from = checked(from).map_err(CopyError::Source)?;
    let to = checked(to).map_err(CopyError::Target)?;
    copy_paths(from, to, overwrite)
}

/// Copies a regular file's bytes and permission bits. Nothing but a
/// regular file is read or written: a FIFO would block the pool thread
/// past any deadline or cancel (CC3). The target exists only with
/// `overwrite` (as `move`, LB7); the bytes stream through a buffer, so a
/// big file costs no memory of its size (LB6); a copy onto the same file
/// leaves it as it is.
fn copy_paths(
    from: &std::ffi::OsStr,
    to: &std::ffi::OsStr,
    overwrite: bool,
) -> Result<(), CopyError> {
    use std::io::Read;
    let meta = std::fs::metadata(from).map_err(CopyError::Source)?;
    if meta.is_dir() {
        return Err(CopyError::Source(errno(libc::EISDIR)));
    }
    if !meta.is_file() {
        return Err(CopyError::NotRegular(true));
    }
    if let Ok(target) = std::fs::metadata(to) {
        if target.is_dir() {
            return Err(CopyError::Target(errno(libc::EISDIR)));
        }
        if !target.is_file() {
            return Err(CopyError::NotRegular(false));
        }
        if !overwrite {
            return Err(CopyError::Exists);
        }
        if target.dev() == meta.dev() && target.ino() == meta.ino() {
            return Ok(());
        }
    }
    let perm = meta.permissions().mode() & 0o777;
    let mut src = std::fs::File::open(from).map_err(CopyError::Source)?;
    let mut o = OpenOptions::new();
    o.write(true).mode(perm);
    if overwrite {
        o.create(true).truncate(true);
    } else {
        o.create_new(true);
    }
    let mut dst = o.open(to).map_err(|e| {
        if e.raw_os_error() == Some(EEXIST) {
            CopyError::Exists
        } else {
            CopyError::Target(e)
        }
    })?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match src.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(CopyError::Source(e)),
        };
        dst.write_all(&buf[..n]).map_err(CopyError::Target)?;
    }
    file::close_checked(dst, to.as_bytes()).map_err(CopyError::Target)
}

const MODES: &str = "'read | 'write | 'append";

/// How long an open that ran out of descriptors waits for the closes of
/// dropped handles (`Closer::settle`).
const SETTLE_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);

/// `Ok(<file>)` or the open failure.
fn open_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let p = nat::need_bytes("path", &a[0])?.to_vec();
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
    let perm = perm_arg(&a[2])?;
    if let (Mode::Read, Some(bits)) = (mode, perm) {
        return Err(nat::fail(
            "E0403",
            vec![
                nat::s("perm"),
                nat::s("nil when mode is 'read"),
                Arg::Str(octal(bits.into())),
            ],
        ));
    }
    let job = p.clone();
    let closer = t.closer();
    // A FIFO is opened on the run thread (`pipe.rs`, L11); `None` says so.
    let open = move || -> io::Result<Option<Io>> {
        checked(&job)?;
        if pipe::is_fifo(&job) {
            return Ok(None);
        }
        match file::open(&job, mode, perm) {
            // Out of descriptors while dropped handles wait to be closed:
            // wait for them, then try once more.
            Err(e)
                if matches!(e.raw_os_error(), Some(libc::EMFILE | libc::ENFILE))
                    && closer.settle(SETTLE_LIMIT) =>
            {
                file::open(&job, mode, perm)
            }
            r => r,
        }
        .map(Some)
    };
    crate::vm::offload_kind(t, Kind::Wait, open, move |t, r| match r {
        Ok(Some(io)) => Ok(Flow::Done(handle(t, p, io))),
        Ok(None) => open_fifo(t, p, mode, perm),
        Err(e) => Ok(Flow::Done(fs_err("open", &p, &e))),
    })
}

/// `Ok(<file>)` for what `fs.open` opened.
fn handle(t: &Task<'_>, p: Vec<u8>, io: Io) -> Value {
    nat::ok(Value::File(Arc::new(File::new(p, io, t.closer()))))
}

/// `fs.open` of a FIFO: the read end opens at once; a write end waits
/// (in the event loop) until a reader has the FIFO open.
fn open_fifo(t: &mut Task<'_>, p: Vec<u8>, mode: Mode, perm: Option<u32>) -> Result<Flow, Stop> {
    if let Mode::Read = mode {
        return Ok(Flow::Done(match Pipe::open_read(&p) {
            Ok(r) => handle(t, p, Io::Pipe(r)),
            Err(e) => fs_err("open", &p, &e),
        }));
    }
    park_io(
        t,
        move |t, _| match Pipe::open_write(&p, perm) {
            Ok(Some(w)) => IoStep::Done(Ok(handle(t, p.clone(), Io::Pipe(w)))),
            Ok(None) => pipe::until_reader(),
            Err(e) => IoStep::Done(Ok(fs_err("open", &p, &e))),
        },
        |_| {},
    )
}

/// The file handle argument `name`, or E0403.
pub(crate) fn need_file<'a>(name: &str, v: &'a Value) -> Result<&'a Arc<File>, Stop> {
    match v {
        Value::File(f) => Ok(f),
        _ => Err(nat::wrong_arg(name, "file", v)),
    }
}

/// The `Err` of a failed handle operation: closed, or the OS error with
/// `verb`. A handle used against its mode (read from one opened for
/// writing) is a code error, E0520, not a failure from outside (LB9).
pub(crate) fn fault_value(f: &File, verb: &str, e: Fault) -> Result<Value, Stop> {
    let p = show(&f.path);
    let wrong_mode = |dir: &str| nat::fail("E0520", vec![Arg::Str(p.to_string()), nat::s(dir)]);
    Ok(match e {
        Fault::Closed => nat::err_value("closed", &format!("file `{p}` is closed"), Vec::new()),
        Fault::NotReading => return Err(wrong_mode("reading")),
        Fault::NotWriting => return Err(wrong_mode("writing")),
        Fault::Io(e) => fs_err(verb, &f.path, &e),
    })
}

/// `close`. The flush and the close run on a pool thread.
pub(crate) fn close_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let f = need_file("f", &a[0])?.clone();
    // Tasks waiting on a FIFO's descriptor wake and find it closed.
    if let Some(fd) = f.pipe_fd() {
        crate::vm::drop_io_waits(t, fd);
    }
    let job = f.clone();
    nat::offload(
        t,
        move || job.close(),
        move |_, r| match r {
            Ok(()) => Ok(nat::ok(Value::Nil)),
            Err(e) => fault_value(&f, "close", e),
        },
    )
}

/// `fs.write_line`: the line and its `\n` go in under the handle's
/// lock: here when they fit in the write buffer, else on a pool thread.
fn write_line_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let f = need_file("f", &a[0])?.clone();
    let line = nat::need_bytes("line", &a[1])?;
    let mut bytes = Vec::with_capacity(line.len() + 1);
    bytes.extend_from_slice(line);
    bytes.push(b'\n');
    if f.is_pipe() {
        return write_pipe_line(t, f, &bytes);
    }
    if let Some(r) = f.write_buffered_line(&bytes) {
        return written_value(&f, r).map(Flow::Done);
    }
    let job = f.clone();
    nat::offload(
        t,
        move || job.write_line(&bytes),
        move |_, r| written_value(&f, r),
    )
}

/// `fs.write_line` on a FIFO: the line is queued, then sent in the event
/// loop (`pipe.rs`). A cancelled send leaves the rest queued, to go
/// before the next line or at the close.
fn write_pipe_line(t: &mut Task<'_>, f: Arc<File>, line: &[u8]) -> Result<Flow, Stop> {
    if let Err(e) = f.pipe_queue(line) {
        return written_value(&f, Err(e)).map(Flow::Done);
    }
    park_io(
        t,
        move |_, _| match f.pipe_flush() {
            Ok(Step::Ready(())) => IoStep::Done(written_value(&f, Ok(()))),
            Ok(Step::Wait(fd, interest)) => pipe::io_step(fd, interest),
            Ok(Step::Fail(e)) => IoStep::Done(written_value(&f, Err(Fault::Io(e)))),
            Err(e) => IoStep::Done(written_value(&f, Err(e))),
        },
        |_| {},
    )
}

/// A handle's `write_line` result (or its fault) as `fs.write_line` gives it.
fn written_value(f: &File, r: Result<(), Fault>) -> Result<Value, Stop> {
    match r {
        Ok(()) => Ok(nat::ok(Value::Nil)),
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
                Ok(dir) => nat::ok(Value::bytes(dir)),
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
    match e {
        TempDirError::Os(e) => fs_err(verb, base, &e),
        TempDirError::Taken => nat::err_value(
            "exists",
            &format!("cannot {verb} `{}`: every name tried exists", show(base)),
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
