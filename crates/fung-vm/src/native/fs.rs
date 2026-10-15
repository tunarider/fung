//! `_native.fs` (Go `internal/native/fs.go`): the file and directory
//! primitives `extras/fs.fung` wraps (06). Every operation that can fail
//! for outside reasons returns `Ok`/`Err({kind, message})`; wrong argument
//! types are E0403.
//!
//! The operating system is called without the run token (`outside`), with
//! owned paths and bytes only; values are made after the token is back.
//! Go's `os` functions that do more than one system call (`MkdirAll`,
//! `Remove`, `RemoveAll`, `Rename`, `MkdirTemp`) are ported step by step,
//! so their failures carry the same errno (`oserr` gives Go's text).

use std::fs::{DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::rc::Rc;
use std::sync::OnceLock;

use fung_front::source::decode_go;

use fung_front::diag::Arg;

use super::file::{self, Fault, File, Mode};
use super::glob;
use super::nat::{self, native, opt, param};
use super::oserr::{self, EEXIST, EINVAL, ENOENT, ENOTDIR, errno};
use super::path;
use crate::value::{Lit, MapBuilder, Module, Native, Value};
use crate::vm::{Stop, Task, outside};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.fs` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            ("read", native("read", read_fn, vec![param("path")])),
            (
                "write",
                native("write", write_fn, vec![param("path"), param("text")]),
            ),
            (
                "append",
                native("append", append_fn, vec![param("path"), param("text")]),
            ),
            (
                "list",
                native(
                    "list",
                    list_fn,
                    vec![param("dir"), opt("full", Lit::Bool(false))],
                ),
            ),
            ("glob", native("glob", glob_fn, vec![param("pattern")])),
            ("walk", native("walk", walk_fn, vec![param("dir")])),
            ("exists", native("exists", exists_fn, vec![param("path")])),
            ("stat", native("stat", stat_fn, vec![param("path")])),
            ("mkdir", native("mkdir", mkdir_fn, vec![param("path")])),
            ("remove", native("remove", remove_fn, vec![param("path")])),
            (
                "remove_all",
                native("remove_all", remove_all_fn, vec![param("path")]),
            ),
            (
                "move",
                native("move", move_fn, vec![param("from"), param("to")]),
            ),
            (
                "copy",
                native("copy", copy_fn, vec![param("from"), param("to")]),
            ),
            (
                "open",
                native(
                    "open",
                    open_fn,
                    vec![param("path"), opt("mode", Lit::Sym("read"))],
                ),
            ),
            ("close", native("close", close_fn, vec![param("f")])),
            (
                "write_line",
                native("write_line", write_line_fn, vec![param("f"), param("line")]),
            ),
            ("temp_dir", native("temp_dir", temp_dir_fn, vec![])),
        ]
    });
    let mut m = Module::new("fs", "_native.fs");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

/// Go `fsErr`: `Err({kind, message})` with ``cannot <verb> `<path>`: <reason>``.
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

/// Go refuses a path holding a NUL byte with EINVAL before any system
/// call (`syscall.ByteSliceFromString`); Rust's own error has no errno.
fn checked(p: &str) -> io::Result<&str> {
    if p.contains('\0') {
        Err(errno(EINVAL))
    } else {
        Ok(p)
    }
}

/// Go `os.WriteFile`: create or truncate with `perm`, then write.
fn write_file(p: &str, bytes: &[u8], perm: u32) -> io::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(perm)
        .open(checked(p)?)?;
    f.write_all(bytes)
}

fn read_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let r = {
        let p = p.clone();
        outside(t, move || std::fs::read(checked(&p)?))
    };
    Ok(match r {
        Ok(bytes) => nat::ok(Value::str(&decode_go(&bytes))),
        Err(e) => fs_err("read", &p, &e),
    })
}

fn write_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [p, text] = nat::str_args(&a, ["path", "text"])?;
    let (p, text) = (p.to_string(), text.as_bytes().to_vec());
    let r = {
        let p = p.clone();
        outside(t, move || write_file(&p, &text, 0o644))
    };
    Ok(done("write", &p, r))
}

fn append_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [p, text] = nat::str_args(&a, ["path", "text"])?;
    let (p, text) = (p.to_string(), text.as_bytes().to_vec());
    let r = {
        let p = p.clone();
        outside(t, move || {
            OpenOptions::new()
                .append(true)
                .create(true)
                .mode(0o644)
                .open(checked(&p)?)?
                .write_all(&text)
        })
    };
    Ok(done("append to", &p, r))
}

/// A fung list of the strings `paths` holds (file names need not be
/// UTF-8; Go prints a bad byte as U+FFFD).
fn str_list(paths: &[Vec<u8>]) -> Value {
    nat::list(paths.iter().map(|p| Value::str(&decode_go(p))).collect())
}

fn os_path(p: &[u8]) -> &std::ffi::OsStr {
    std::ffi::OsStr::from_bytes(p)
}

/// Go `os.ReadDir`: the directory's entries in byte order of their
/// names, each with whether it is a directory itself (a link is not).
fn read_dir(dir: &[u8]) -> io::Result<Vec<(Vec<u8>, bool)>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(os_path(dir))? {
        let e = e?;
        let is_dir = match e.file_type() {
            Ok(t) => t.is_dir(),
            // Go drops an entry gone before its type could be read.
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        out.push((e.file_name().as_bytes().to_vec(), is_dir));
    }
    out.sort_unstable();
    Ok(out)
}

fn list_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let dir = nat::need_str("dir", &a[0])?.to_string();
    let Value::Bool(full) = a[1] else {
        return Err(nat::wrong_arg("full", "bool", &a[1]));
    };
    let r = {
        let dir = dir.clone();
        outside(t, move || {
            let names = read_dir(checked(&dir)?.as_bytes())?.into_iter();
            Ok::<_, io::Error>(if full {
                names
                    .map(|(n, _)| path::join_bytes(dir.as_bytes(), &n))
                    .collect::<Vec<_>>()
            } else {
                names.map(|(n, _)| n).collect()
            })
        })
    };
    Ok(match r {
        Ok(names) => nat::ok(str_list(&names)),
        Err(e) => fs_err("list", &dir, &e),
    })
}

fn glob_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let pattern = nat::need_str("pattern", &a[0])?.to_string();
    let r = {
        let pattern = pattern.clone();
        outside(t, move || glob::glob(pattern.as_bytes()))
    };
    Ok(match r {
        Ok(mut found) => {
            // Go sorts the whole result once more (`sort.Strings`).
            found.sort_unstable();
            nat::ok(str_list(&found))
        }
        Err(glob::BadPattern) => nat::err_value(
            "invalid",
            &format!("bad glob pattern `{pattern}`: syntax error in pattern"),
            Vec::new(),
        ),
    })
}

fn walk_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let dir = nat::need_str("dir", &a[0])?.to_string();
    let r = {
        let dir = dir.clone();
        outside(t, move || walk(checked(&dir)?.as_bytes()))
    };
    Ok(match r {
        Ok(files) => nat::ok(str_list(&files)),
        Err(e) => fs_err("walk", &dir, &e),
    })
}

/// Go `fsWalkFn` over `filepath.WalkDir`: every file below `root` (not
/// the directories), depth first in byte order, as paths joined onto
/// `root`. Links are not followed (a link is listed like a file), except
/// a root the system resolves itself (`link/`). The root is `lstat`ed;
/// a directory that cannot be read ends the walk with its error.
fn walk(root: &[u8]) -> io::Result<Vec<Vec<u8>>> {
    let mut files = Vec::new();
    if std::fs::symlink_metadata(os_path(root))?.is_dir() {
        walk_dir(root, &mut files)?;
    } else {
        files.push(root.to_vec());
    }
    Ok(files)
}

fn walk_dir(dir: &[u8], files: &mut Vec<Vec<u8>>) -> io::Result<()> {
    for (name, is_dir) in read_dir(dir)? {
        let p = path::join_bytes(dir, &name);
        if is_dir {
            walk_dir(&p, files)?;
        } else {
            files.push(p);
        }
    }
    Ok(())
}

fn exists_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let found = outside(t, move || checked(&p).and_then(std::fs::metadata).is_ok());
    Ok(Value::Bool(found))
}

fn stat_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let r = {
        let p = p.clone();
        outside(t, move || {
            std::fs::metadata(checked(&p)?).map(|m| (m.size(), m.is_dir()))
        })
    };
    let (size, is_dir) = match r {
        Ok(info) => info,
        Err(e) => return Ok(fs_err("stat", &p, &e)),
    };
    // Go's `FileInfo.Name` is `filepath.Base` of the path asked for.
    let mut m = MapBuilder::default();
    m.set(Rc::new("name".into()), Value::str(&path::base(&p)));
    m.set(Rc::new("size".into()), Value::Int(size as i64));
    m.set(Rc::new("is_dir".into()), Value::Bool(is_dir));
    Ok(nat::ok(Value::Map(Rc::new(m.finish()))))
}

fn mkdir_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let r = {
        let p = p.clone();
        outside(t, move || mkdir_all(checked(&p)?, 0o755))
    };
    Ok(done("create directory", &p, r))
}

/// Go `os.MkdirAll`: a directory already there is success, anything else
/// there is ENOTDIR; otherwise the parent first, then the directory.
fn mkdir_all(p: &str, perm: u32) -> io::Result<()> {
    if let Ok(m) = std::fs::metadata(p) {
        return if m.is_dir() {
            Ok(())
        } else {
            Err(errno(ENOTDIR))
        };
    }
    let b = p.as_bytes();
    let mut i = b.len();
    while i > 0 && b[i - 1] == b'/' {
        i -= 1;
    }
    while i > 0 && b[i - 1] != b'/' {
        i -= 1;
    }
    // `i` is now just past the parent's last separator; Go's parent is
    // the path up to that separator.
    let parent = &p[..i.saturating_sub(1)];
    if !parent.is_empty() {
        mkdir_all(parent, perm)?;
    }
    match DirBuilder::new().mode(perm).create(p) {
        Ok(()) => Ok(()),
        // "foo/." and the like: the directory may be there after all.
        Err(e) => match std::fs::symlink_metadata(p) {
            Ok(m) if m.is_dir() => Ok(()),
            _ => Err(e),
        },
    }
}

fn remove_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let r = {
        let p = p.clone();
        outside(t, move || remove(checked(&p)?))
    };
    Ok(done("remove", &p, r))
}

/// Go `os.Remove`: unlink, then rmdir; when both fail, rmdir's error
/// unless it is ENOTDIR (the path is a file, so unlink's error is real).
fn remove(p: &str) -> io::Result<()> {
    let Err(e) = std::fs::remove_file(p) else {
        return Ok(());
    };
    match std::fs::remove_dir(p) {
        Ok(()) => Ok(()),
        Err(e1) if e1.raw_os_error() != Some(ENOTDIR) => Err(e1),
        Err(_) => Err(e),
    }
}

fn remove_all_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let p = nat::need_str("path", &a[0])?.to_string();
    let r = {
        let p = p.clone();
        outside(t, move || remove_all(checked(&p)?))
    };
    Ok(done("remove", &p, r))
}

/// Go `os.RemoveAll`: nothing for `""` or a missing path, EINVAL for a
/// path ending in `.`, `Remove` when that works, and otherwise the
/// directory's contents first.
fn remove_all(p: &str) -> io::Result<()> {
    if p.is_empty() {
        return Ok(());
    }
    if p == "." || p.ends_with("/.") {
        return Err(errno(EINVAL));
    }
    let e = match remove(p) {
        Ok(()) => return Ok(()),
        Err(e) if e.raw_os_error() == Some(ENOENT) => return Ok(()),
        Err(e) => e,
    };
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => match std::fs::remove_dir_all(p) {
            Err(e) if e.raw_os_error() == Some(ENOENT) => Ok(()),
            r => r,
        },
        _ => Err(e),
    }
}

fn move_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [from, to] = nat::str_args(&a, ["from", "to"])?;
    let (from, to) = (from.to_string(), to.to_string());
    let r = {
        let (from, to) = (from.clone(), to.clone());
        outside(t, move || rename(checked(&from)?, checked(&to)?))
    };
    Ok(done("move", &from, r))
}

/// Go `os.Rename` (Unix): onto an existing directory only the same file
/// may go (a case-only rename); anything else is EEXIST, after the old
/// name's own error.
fn rename(from: &str, to: &str) -> io::Result<()> {
    if let Ok(target) = std::fs::symlink_metadata(to)
        && target.is_dir()
    {
        let source = std::fs::symlink_metadata(from)?;
        let same = source.dev() == target.dev() && source.ino() == target.ino();
        if from == to || !same {
            return Err(errno(EEXIST));
        }
    }
    std::fs::rename(from, to)
}

fn copy_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [from, to] = nat::str_args(&a, ["from", "to"])?;
    let (from, to) = (from.to_string(), to.to_string());
    let r = {
        let (from, to) = (from.clone(), to.clone());
        outside(t, move || copy(&from, &to))
    };
    Ok(match r {
        Ok(()) => nat::ok(Value::Nil),
        Err((true, e)) => fs_err("copy", &from, &e),
        Err((false, e)) => fs_err("copy to", &to, &e),
    })
}

/// Go `fsCopyFn`: the source's permission bits, its bytes, then a write.
/// The error says which side failed (`true` for the source).
fn copy(from: &str, to: &str) -> Result<(), (bool, io::Error)> {
    let src = |e| (true, e);
    let from = checked(from).map_err(src)?;
    let perm = std::fs::metadata(from).map_err(src)?.permissions().mode() & 0o777;
    let bytes = std::fs::read(from).map_err(src)?;
    write_file(to, &bytes, perm).map_err(|e| (false, e))
}

const MODES: &str = "'read | 'write | 'append";

/// Go `fsOpenFn`: `Ok(<file>)` or the open failure.
fn open_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
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
    let r = {
        let p = p.clone();
        outside(t, move || file::open(checked(&p)?, mode))
    };
    Ok(match r {
        Ok(io) => nat::ok(Value::File(Rc::new(File::new(p, io)))),
        Err(e) => fs_err("open", &p, &e),
    })
}

/// Go `needFile`.
pub(crate) fn need_file<'a>(name: &str, v: &'a Value) -> Result<&'a File, Stop> {
    match v {
        Value::File(f) => Ok(f),
        _ => Err(nat::wrong_arg(name, "file", v)),
    }
}

/// A failed handle operation as Go reports it (`closedErr`, the
/// `'invalid` direction errors, `fsErr` with `verb`).
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

/// Go `fsCloseFn`. The flush and the close run without the token.
fn close_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let f = need_file("f", &a[0])?;
    Ok(match outside(t, || f.close()) {
        Ok(()) => nat::ok(Value::Nil),
        Err(e) => fault_value(f, "close", e),
    })
}

/// Go `fsWriteLineFn`: the line and its `\n` go in under the handle's
/// lock, without the token.
fn write_line_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let f = need_file("f", &a[0])?;
    let line = nat::need_str("line", &a[1])?;
    let mut bytes = Vec::with_capacity(line.len() + 1);
    bytes.extend_from_slice(line.as_bytes());
    bytes.push(b'\n');
    Ok(match outside(t, || f.write_line(&bytes)) {
        Ok(()) => nat::ok(Value::Nil),
        Err(e) => fault_value(f, "write", e),
    })
}

fn temp_dir_fn(t: &mut Task<'_>, _: Vec<Value>) -> Result<Value, Stop> {
    Ok(match outside(t, make_temp_dir) {
        Ok(dir) => nat::ok(Value::str(&decode_go(&dir))),
        Err(TempDirError::Os(e)) => fs_err("create temporary directory", "", &e),
        // Go's `fs.ErrExist` after 10,000 names already taken.
        Err(TempDirError::Taken) => nat::err_value(
            "exists",
            "cannot create temporary directory ``: file already exists",
            Vec::new(),
        ),
    })
}

enum TempDirError {
    Os(io::Error),
    Taken,
}

/// Go `os.MkdirTemp("", "fung-")`: `$TMPDIR` (`/tmp` when unset or
/// empty) joined with `fung-` and a random 32-bit number in decimal, made
/// with mode 0700; a name already taken is tried again, 10,000 times.
fn make_temp_dir() -> Result<Vec<u8>, TempDirError> {
    let mut prefix = std::env::var_os("TMPDIR")
        .filter(|d| !d.is_empty())
        .map_or_else(|| b"/tmp".to_vec(), |d| d.as_bytes().to_vec());
    if prefix.last() != Some(&b'/') {
        prefix.push(b'/');
    }
    prefix.extend_from_slice(b"fung-");
    for _ in 0..10_000 {
        let mut name = prefix.clone();
        name.extend_from_slice(random_u32().to_string().as_bytes());
        let os = std::ffi::OsStr::from_bytes(&name);
        match DirBuilder::new().mode(0o700).create(os) {
            Ok(()) => return Ok(name),
            Err(e) if e.raw_os_error() == Some(EEXIST) => continue,
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
