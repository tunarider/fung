//! Which `.fung` files a command works on, chosen from its path arguments
//! the way Go's `cmd/fung` chooses them: `fung test` (Go `testFiles`) and
//! `fung fmt` (Go `fungFiles`). Both walk like Go's `filepath.WalkDir`:
//! a directory's entries in byte order, links not followed (a link is an
//! entry like a file, kept when its name ends in `.fung`), paths joined
//! and cleaned like `filepath.Join`, and the result sorted by bytes.
//! Paths are bytes: file names need not be UTF-8.
//!
//! An error is Go's text without the `error: ` prefix the commands add,
//! e.g. `stat nope: no such file or directory`.

use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;

use fung_vm::native::os_error_text;
use fung_vm::native::path::join_bytes;

/// The files `fung test [root]` runs: `root` itself when it is not a
/// directory, else every `.fung` file below it, skipping directories
/// whose name starts with `_` (fixtures). Directories starting with `.`
/// are walked (Go `testFiles` does). A root that is a link to a
/// directory is not followed (Go's `WalkDir` does not follow its root):
/// it is kept only when its own name ends in `.fung`.
pub fn test_files(root: &[u8]) -> Result<Vec<Vec<u8>>, Vec<u8>> {
    if !stat(root)?.is_dir() {
        return Ok(vec![root.to_vec()]);
    }
    let mut files = Vec::new();
    let r = walk(root, &mut files, &|name| name.starts_with(b"_"));
    files.sort();
    r.map(|()| files)
}

/// The files `fung fmt <path>...` works on: each file argument as given,
/// and every `.fung` file below each directory argument, skipping
/// directories whose name starts with `_` (fixtures) or `.` (`.git`).
/// A directory argument that is a link is followed (Go adds a trailing
/// separator so the system resolves it). The whole list is sorted; a
/// file named twice is listed twice. The first path that cannot be
/// read ends the expansion with its error.
pub fn fung_files(paths: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, Vec<u8>> {
    let mut files = Vec::new();
    for p in paths {
        if !stat(p)?.is_dir() {
            files.push(p.clone());
            continue;
        }
        let is_link = std::fs::symlink_metadata(os(p)).is_ok_and(|m| m.file_type().is_symlink());
        let root = if is_link {
            [&p[..], b"/"].concat()
        } else {
            p.clone()
        };
        walk(&root, &mut files, &|name| {
            name.starts_with(b"_") || name.starts_with(b".")
        })?;
    }
    files.sort();
    Ok(files)
}

fn os(p: &[u8]) -> &OsStr {
    OsStr::from_bytes(p)
}

/// Go `*PathError` text: `op path: reason`.
fn path_error(op: &str, path: &[u8], e: &io::Error) -> Vec<u8> {
    [
        op.as_bytes(),
        b" ",
        path,
        b": ",
        os_error_text(e).as_bytes(),
    ]
    .concat()
}

/// Go `os.Stat`.
fn stat(p: &[u8]) -> Result<std::fs::Metadata, Vec<u8>> {
    std::fs::metadata(os(p)).map_err(|e| path_error("stat", p, &e))
}

/// Go `filepath.Base` (Unix): the last element, trailing slashes dropped.
fn base(p: &[u8]) -> &[u8] {
    let end = p.iter().rposition(|&c| c != b'/').map_or(0, |i| i + 1);
    if end == 0 {
        return if p.is_empty() { b"." } else { b"/" };
    }
    let start = p[..end]
        .iter()
        .rposition(|&c| c == b'/')
        .map_or(0, |i| i + 1);
    &p[start..end]
}

/// Go `filepath.WalkDir(root, fn)` with the callback both commands use:
/// a directory below `root` whose name `skip` accepts is not entered, a
/// file (any entry that is not a directory) ending in `.fung` is
/// collected, and the first error ends the walk.
fn walk(
    root: &[u8],
    files: &mut Vec<Vec<u8>>,
    skip: &dyn Fn(&[u8]) -> bool,
) -> Result<(), Vec<u8>> {
    let info = std::fs::symlink_metadata(os(root)).map_err(|e| path_error("lstat", root, &e))?;
    walk_entry(root, root, base(root), info.is_dir(), files, skip)
}

fn walk_entry(
    root: &[u8],
    path: &[u8],
    name: &[u8],
    is_dir: bool,
    files: &mut Vec<Vec<u8>>,
    skip: &dyn Fn(&[u8]) -> bool,
) -> Result<(), Vec<u8>> {
    if !is_dir {
        if name.ends_with(b".fung") {
            files.push(path.to_vec());
        }
        return Ok(());
    }
    if path != root && skip(name) {
        return Ok(());
    }
    for (child, child_is_dir) in read_dir(path)? {
        let p = join_bytes(path, &child);
        walk_entry(root, &p, &child, child_is_dir, files, skip)?;
    }
    Ok(())
}

/// Go `os.ReadDir`: the entries sorted by name, each with whether it is a
/// directory (links are not followed).
fn read_dir(dir: &[u8]) -> Result<Vec<(Vec<u8>, bool)>, Vec<u8>> {
    let fail = |e: io::Error| path_error("open", dir, &e);
    let mut out = Vec::new();
    for e in std::fs::read_dir(os(dir)).map_err(fail)? {
        let e = e.map_err(fail)?;
        let is_dir = match e.file_type() {
            Ok(t) => t.is_dir(),
            // Go drops an entry gone before its type could be read.
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(fail(err)),
        };
        out.push((e.file_name().as_bytes().to_vec(), is_dir));
    }
    out.sort_unstable();
    Ok(out)
}
