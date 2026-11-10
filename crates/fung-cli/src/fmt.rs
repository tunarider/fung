//! `fung fmt [--check] <path>...|-` (08): formats
//! files in place and names the changed ones; `--check` only names them
//! (exit 1 if any, and only then); `-` formats standard input to standard
//! output (`--check -` names `<stdin>` instead, exit 1, if it needs
//! formatting). Sources are read as `run` and `check` read them: a file
//! that is not UTF-8 is refused and left alone (D31), as is one with a
//! syntax error, and a file that cannot be read is exit 2 (D30).

use std::io::Read;

use fung_front::diag::{self, Diag, EXIT_COMPILE, EXIT_RUNTIME, EXIT_USAGE};
use fung_front::format::format;
use fung_front::source::{SourceMap, decode_lossy};

use crate::args::{Spec, parse};
use crate::{Out, ReadFail, decode, paths, read_bytes, restore_name, usage_error};

/// `args` are the arguments after `fmt`, decoded; `raw` their bytes.
pub fn run_fmt(args: &[String], raw: &[Vec<u8>]) -> Out {
    const SPEC: Spec = Spec {
        flags: &["--check"],
        code: false,
        to_script: false,
    };
    let p = match parse(args, raw, &SPEC) {
        Ok(p) => p,
        Err(out) => return out,
    };
    let check = p.has("--check");
    let paths = p.raw;
    if paths.is_empty() {
        return usage_error(b"fmt needs a path, or - for standard input");
    }
    if paths.iter().any(|p| p == b"-") {
        if paths.len() > 1 {
            return usage_error(b"fmt reads standard input only when `-` is the only path");
        }
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
        let src = match read_bytes(path).and_then(decode) {
            Ok(s) => s,
            Err(fail) => {
                out.stderr.extend_from_slice(
                    &[
                        b"error: ",
                        &crate::cannot_read(path, &fail.reason)[..],
                        b"\n",
                    ]
                    .concat(),
                );
                worse(&mut out, EXIT_COMPILE);
                continue;
            }
        };
        let name = decode_lossy(path);
        let formatted = match format(&name, &src) {
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
        if let Err(e) = write(path, formatted.as_bytes()) {
            out.stderr.extend_from_slice(
                &[b"error: cannot write `", &path[..], b"`: ", &e, b"\n"].concat(),
            );
            worse(&mut out, EXIT_COMPILE);
        }
    }
    out
}

/// `fung fmt -`: standard input to standard output, named `<stdin>`.
/// With `--check` nothing is formatted out: `<stdin>` is named (exit 1)
/// if the input needs formatting, like a file.
fn stdin(check: bool) -> Out {
    let mut bytes = Vec::new();
    let read = std::io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .map_err(|e| ReadFail {
            reason: fung_vm::native::os_error_text(&e).into_bytes(),
            missing: false,
        })
        .and_then(|_| decode(bytes));
    let src = match read {
        Ok(s) => s,
        Err(fail) => {
            return Out {
                stderr: [
                    b"error: ",
                    &crate::cannot_read(b"<stdin>", &fail.reason)[..],
                    b"\n",
                ]
                .concat(),
                code: EXIT_COMPILE,
                ..Out::default()
            };
        }
    };
    match format("<stdin>", &src) {
        Ok(f) if !check => Out {
            stdout: f.into_bytes(),
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
/// (E0801) is the formatter's own error, anything else is the source's
/// syntax error with its excerpt; both are 2, since fmt's 1 means only
/// "needs formatting" (D30).
fn refused(e: &Diag, name: &str, raw: &[u8], src: &str) -> (Vec<u8>, i32) {
    if e.code == "E0801" {
        let text = [
            b"error: formatter changed the code of ",
            raw,
            b"; nothing was written\n",
        ]
        .concat();
        return (text, EXIT_COMPILE);
    }
    let mut sources = SourceMap::new();
    sources.add(name, src);
    let text = diag::format(e, &sources);
    (restore_name(&text, name, raw), EXIT_COMPILE)
}

/// Rewrites a file whole or not at all, as `fs.write` does
/// (`fung_vm::native::fs::write_whole`: a temporary file beside it renamed
/// over it, a link followed, the mode kept). A failure is the reason
/// alone, as the `fs` module words it.
fn write(path: &[u8], data: &[u8]) -> Result<(), Vec<u8>> {
    fung_vm::native::fs::write_whole(path, data, None)
        .map_err(|e| fung_vm::native::os_error_text(&e).into_bytes())
}
