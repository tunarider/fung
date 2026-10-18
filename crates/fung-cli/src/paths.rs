//! Which `.fung` files a command works on, chosen from its path
//! arguments: `fung test` and `fung fmt` follow one rule (08). A file
//! argument is itself; a directory argument (also through a link given
//! as the argument) is walked: its entries in byte order, directories
//! whose name starts with `_` (fixtures) or `.` (`.git`, caches) not
//! entered, links below it not followed (a link is an entry like a file,
//! kept when its name ends in `.fung` and it does not name a directory),
//! paths joined and cleaned, and the
//! result sorted by bytes. Paths are bytes: file names need not be UTF-8.
//!
//! An error is the `fs` module's form without the `error: ` prefix the
//! commands add: ``cannot read `nope`: no such file or directory``.

use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;

use fung_vm::native::os_error_text;
use fung_vm::native::path::join_bytes;

/// The files `fung test [root]` runs: those [`fung_files`] chooses for
/// the one root.
pub fn test_files(root: &[u8]) -> Result<Vec<Vec<u8>>, Vec<u8>> {
    fung_files(&[root.to_vec()])
}

/// The files `fung fmt <path>...` works on: each file argument as given,
/// and every `.fung` file below each directory argument (the module
/// rule). A directory argument that is a link is followed (a trailing
/// separator makes the system resolve it). The whole list is sorted; a
/// file named twice is listed twice. The first path that cannot be read
/// ends the expansion with its error.
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

/// ``cannot read `path`: reason``.
fn path_error(path: &[u8], e: &io::Error) -> Vec<u8> {
    [b"cannot read `", path, b"`: ", os_error_text(e).as_bytes()].concat()
}

/// What `p` is, following links.
fn stat(p: &[u8]) -> Result<std::fs::Metadata, Vec<u8>> {
    std::fs::metadata(os(p)).map_err(|e| path_error(p, &e))
}

/// The last element of a path, trailing slashes dropped.
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

/// Walks `root`: a directory below it whose name `skip` accepts is not
/// entered, a file (any entry that is not a directory) ending in `.fung`
/// is collected, and the first error ends the walk.
fn walk(
    root: &[u8],
    files: &mut Vec<Vec<u8>>,
    skip: &dyn Fn(&[u8]) -> bool,
) -> Result<(), Vec<u8>> {
    let info = std::fs::symlink_metadata(os(root)).map_err(|e| path_error(root, &e))?;
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
        // a link named like a script that names a directory is no script
        // (and is not followed)
        let names_dir = std::fs::metadata(os(path)).is_ok_and(|m| m.is_dir());
        if name.ends_with(b".fung") && (path == root || !names_dir) {
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

/// A directory's entries sorted by name, each with whether it is a
/// directory (links are not followed).
fn read_dir(dir: &[u8]) -> Result<Vec<(Vec<u8>, bool)>, Vec<u8>> {
    let fail = |e: io::Error| path_error(dir, &e);
    let mut out = Vec::new();
    for e in std::fs::read_dir(os(dir)).map_err(fail)? {
        let e = e.map_err(fail)?;
        let is_dir = match e.file_type() {
            Ok(t) => t.is_dir(),
            // an entry gone before its type could be read is no entry
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(fail(err)),
        };
        out.push((e.file_name().as_bytes().to_vec(), is_dir));
    }
    out.sort_unstable();
    Ok(out)
}
