//! `fung fmt [--check] <path>...|-` (08): formats
//! files in place and names the changed ones; `--check` only names them
//! (exit 1 if any); `-` formats standard input to standard output
//! (`--check -` names `<stdin>` instead, exit 1, if it needs formatting).
//! A file with a syntax error is reported and left alone (exit 2).

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use fung_front::diag::{self, Diag, EXIT_COMPILE, EXIT_RUNTIME, EXIT_USAGE};
use fung_front::format::format_bytes;
use fung_front::source::{SourceMap, decode_lossy};

use crate::{Out, USAGE, paths, read_bytes, restore_name};

/// `args` are the arguments after `fmt`, decoded; `raw` their bytes.
pub fn run_fmt(args: &[String], raw: &[Vec<u8>]) -> Out {
    let mut check = false;
    let mut paths: Vec<Vec<u8>> = Vec::new();
    for (a, r) in args.iter().zip(raw) {
        if a == "--check" {
            check = true;
        } else if a.starts_with('-') && a != "-" {
            return usage(&[b"unknown option ", &r[..]].concat());
        } else {
            paths.push(r.clone());
        }
    }
    if paths.is_empty() {
        return usage(b"fmt needs a path, or - for standard input");
    }
    if paths.len() == 1 && paths[0] == b"-" {
        return stdin(check);
    }
    let files = match paths::fung_files(&paths) {
        Ok(f) => f,
        Err(e) => {
            return Out {
                stderr: [b"error: ", &e[..], b"\n"].concat(),
                code: EXIT_USAGE,
                ..Out::default()
            };
        }
    };
    let mut out = Out::default();
    let worse = |out: &mut Out, c: i32| out.code = out.code.max(c);
    for path in &files {
        let src = match read_bytes(path) {
            Ok(s) => s,
            Err(msg) => {
                out.stderr.extend_from_slice(
                    &[b"error: ", &crate::cannot_read(path, &msg)[..], b"\n"].concat(),
                );
                worse(&mut out, EXIT_RUNTIME);
                continue;
            }
        };
        let name = decode_lossy(path);
        let formatted = match format_bytes(&name, &src) {
            Ok(f) => f,
            Err(e) => {
                let (text, code) = refused(&e, &name, path, &src);
                out.stderr.extend_from_slice(&text);
                worse(&mut out, code);
                continue;
            }
        };
        if formatted == src {
            continue;
        }
        out.stdout.extend_from_slice(path);
        out.stdout.push(b'\n');
        if check {
            worse(&mut out, EXIT_RUNTIME);
            continue;
        }
        if let Err(e) = write(path, &formatted) {
            out.stderr.extend_from_slice(
                &[b"error: cannot write `", &path[..], b"`: ", &e, b"\n"].concat(),
            );
            worse(&mut out, EXIT_RUNTIME);
        }
    }
    out
}

fn usage(msg: &[u8]) -> Out {
    Out {
        stderr: [b"error: ", msg, b"\n\n", USAGE.as_bytes()].concat(),
        code: EXIT_USAGE,
        ..Out::default()
    }
}

/// `fung fmt -`: standard input to standard output, named `<stdin>`.
/// With `--check` nothing is formatted out: `<stdin>` is named (exit 1)
/// if the input needs formatting, like a file.
fn stdin(check: bool) -> Out {
    let mut src = Vec::new();
    if let Err(e) = std::io::stdin().lock().read_to_end(&mut src) {
        return Out {
            stderr: format!(
                "error: cannot read <stdin>: {}\n",
                fung_vm::native::os_error_text(&e)
            )
            .into_bytes(),
            code: EXIT_RUNTIME,
            ..Out::default()
        };
    }
    match format_bytes("<stdin>", &src) {
        Ok(f) if !check => Out {
            stdout: f,
            ..Out::default()
        },
        Ok(f) if f == src => Out::default(),
        Ok(_) => Out {
            stdout: b"<stdin>\n".to_vec(),
            code: EXIT_RUNTIME,
            ..Out::default()
        },
        Err(e) => {
            let (stderr, code) = refused(&e, "<stdin>", b"<stdin>", &src);
            Out {
                stderr,
                code,
                ..Out::default()
            }
        }
    }
}

/// What a refused file prints and the exit code it adds: the safeguard
/// (E0801) is the formatter's own error (1), anything else is the
/// source's syntax error with its excerpt (2).
fn refused(e: &Diag, name: &str, raw: &[u8], src: &[u8]) -> (Vec<u8>, i32) {
    if e.code == "E0801" {
        let text = [
            b"error: formatter changed the code of ",
            raw,
            b"; nothing was written\n",
        ]
        .concat();
        return (text, EXIT_RUNTIME);
    }
    let mut sources = SourceMap::new();
    sources.add(name, &decode_lossy(src));
    let text = diag::format(e, &sources);
    (restore_name(&text, name, raw), EXIT_COMPILE)
}

/// Rewrites a file: into a temporary file beside it, renamed over it, so
/// a crash or a full disk never leaves half a file. A symbolic link is
/// followed (the file it names is rewritten, the link stays). Where a new
/// file would not be the same file to its users, it is rewritten in place
/// instead (truncated, with the old mode): several hard links (the other
/// names would keep the old text), or an owner or group the new file
/// would not get (someone else's file we may write). A file we may not
/// write is refused as before, not replaced. A failure is the
/// reason alone, as the `fs` module words it.
fn write(path: &[u8], data: &[u8]) -> Result<(), Vec<u8>> {
    use std::os::unix::fs::MetadataExt;
    let err = |e: std::io::Error| fung_vm::native::os_error_text(&e).into_bytes();
    let p = OsStr::from_bytes(path);
    let target = std::fs::canonicalize(p).unwrap_or_else(|_| p.into());
    let meta = std::fs::metadata(&target).ok();
    let mode = meta
        .as_ref()
        .map_or(0o644, |m| m.permissions().mode() & 0o777);
    // A file we may not write is not replaced either (a rename would
    // ignore its mode): the open below reports it.
    let writable = std::fs::OpenOptions::new()
        .write(true)
        .open(&target)
        .is_ok();
    if let Some(m) = &meta
        && writable
        && m.nlink() == 1
        && let Some(dir) = target.parent()
        && let Some(name) = target.file_name()
    {
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(format!(".fmt-{}", std::process::id()));
        let tmp = dir.join(tmp_name);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp)
        {
            let same_owner = f
                .metadata()
                .is_ok_and(|t| t.uid() == m.uid() && t.gid() == m.gid());
            let done = same_owner
                && f.set_permissions(std::fs::Permissions::from_mode(mode))
                    .is_ok()
                && f.write_all(data).is_ok()
                && f.sync_all().is_ok()
                && std::fs::rename(&tmp, &target).is_ok();
            if done {
                return Ok(());
            }
            // Not the same owner, or the write or rename failed: in
            // place below (which then reports a failure of its own).
            let _ = std::fs::remove_file(&tmp);
        }
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(&target)
        .map_err(err)?;
    f.write_all(data).map_err(err)
}
