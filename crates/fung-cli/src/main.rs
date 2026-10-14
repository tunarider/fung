//! The `fung` command of the Rust port. Only `fung dump` exists so far;
//! it mirrors `cmd/fung` of the Go implementation byte for byte.

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};

use fung_front::bytecode::disassemble;
use fung_front::diag::{self, EXIT_COMPILE, EXIT_OK, EXIT_USAGE};
use fung_front::loader::{Loader, Stage};
use fung_front::source::decode_go;

/// Copied verbatim from `cmd/fung/main.go`.
const USAGE: &str = "usage:
  fung <script.fung> [args...]            run a script (same as: fung run <script.fung>)
  fung                                    start the REPL
  fung -e '<code>'                        run one line of code
  fung check <script.fung>                check syntax and names without running
  fung fmt [--check] <path>...|-          format files in place (--check: list only; -: stdin)
  fung test [path]                        run tests
  fung dump ast|core|bytecode <script>    print an internal stage
  fung version                            print the version
  fung help                               print this text
";

/// The front end recurses on the shape of the tree, so it runs on a
/// thread with a large stack: deeply nested input and long flat chains
/// (`1 + 1 + ...`, `x.b.b...`) recurse once per level in the later
/// passes. 4 GiB is only reserved address space (pages are committed
/// when touched) and leaves a margin over Go's 1 GB goroutine stack limit.
const STACK: usize = 4 << 30;

/// What a command produced: standard output, standard error, exit code.
/// The streams are bytes because Go echoes arguments that are not UTF-8
/// unchanged.
#[derive(Default)]
struct Out {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    code: i32,
}

fn main() {
    let raw: Vec<Vec<u8>> = std::env::args_os()
        .skip(1)
        .map(OsString::into_vec)
        .collect();
    let out = std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || run(&raw))
        .expect("spawn the front-end thread")
        .join()
        .unwrap_or_else(|_| Out {
            stderr: b"error: internal error in the front end\n".to_vec(),
            code: diag::EXIT_RUNTIME,
            ..Out::default()
        });
    let _ = std::io::stdout().write_all(&out.stdout);
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().write_all(&out.stderr);
    std::process::exit(out.code);
}

/// `raw` are the arguments as the OS gave them. They are decoded like Go
/// decodes a string (one U+FFFD per invalid byte) for matching and as
/// source code, and echoed unchanged in messages, as Go does.
fn run(raw: &[Vec<u8>]) -> Out {
    let args: Vec<String> = raw.iter().map(|a| decode_go(a)).collect();
    match args.first().map(String::as_str) {
        Some("dump") => run_dump(&args[1..], &raw[1..]),
        Some(_) => Out {
            stderr: [
                b"not implemented yet in the Rust port: ",
                &raw[0][..],
                b"\n",
            ]
            .concat(),
            code: EXIT_USAGE,
            ..Out::default()
        },
        None => Out {
            stderr: b"not implemented yet in the Rust port: the REPL\n".to_vec(),
            code: EXIT_USAGE,
            ..Out::default()
        },
    }
}

fn usage_error(msg: &[u8]) -> Out {
    Out {
        stderr: [b"error: ", msg, b"\n\n", USAGE.as_bytes()].concat(),
        code: EXIT_USAGE,
        ..Out::default()
    }
}

/// `fung dump STAGE FILE` and `fung dump STAGE -e CODE` (Go `runDump`).
/// `raw` holds the undecoded form of each of `args`.
fn run_dump(args: &[String], raw: &[Vec<u8>]) -> Out {
    let Some(stage) = args.first() else {
        return usage_error(b"dump needs a stage: ast, core or bytecode");
    };
    let up_to = match stage.as_str() {
        "ast" => Stage::Ast,
        "core" => Stage::Core,
        "bytecode" => Stage::All,
        _ => {
            return usage_error(
                &[
                    b"unknown dump stage `",
                    &raw[0][..],
                    b"` (want ast, core or bytecode)",
                ]
                .concat(),
            );
        }
    };
    match &args[1..] {
        [e, rest @ ..] if e == "-e" => match rest.first() {
            Some(code) => dump(up_to, b"-e", code),
            None => usage_error(b"-e needs code"),
        },
        [_] => {
            let path = &raw[1];
            match read_file(path) {
                Ok(src) => dump(up_to, path, &src),
                Err(msg) => Out {
                    stderr: [b"error: cannot read ", &path[..], b": ", &msg[..], b"\n"].concat(),
                    code: EXIT_USAGE,
                    ..Out::default()
                },
            }
        }
        _ => usage_error(b"dump needs a script path or -e code"),
    }
}

/// Reads a script and decodes it like Go. A failure is described the way
/// Go's `os.ReadFile` does, with the path bytes unchanged.
fn read_file(path: &[u8]) -> Result<String, Vec<u8>> {
    use std::io::ErrorKind;
    let bytes = std::fs::read(OsStr::from_bytes(path)).map_err(|e| {
        let (op, text) = match e.kind() {
            ErrorKind::NotFound => ("open ", "no such file or directory".to_string()),
            ErrorKind::PermissionDenied => ("open ", "permission denied".to_string()),
            ErrorKind::IsADirectory => ("read ", "is a directory".to_string()),
            _ => {
                let text = e.to_string();
                let text = text.split(" (os error").next().unwrap_or(&text);
                ("open ", text.to_lowercase())
            }
        };
        [op.as_bytes(), path, b": ", text.as_bytes()].concat()
    })?;
    Ok(decode_go(&bytes))
}

/// Prints one stage of a source text (Go `dump`). `file` is the name as
/// given; diagnostics compile under its decoded form, and the location
/// lines of the entry file get the original bytes back.
fn dump(up_to: Stage, file: &[u8], src: &str) -> Out {
    let name = decode_go(file);
    let (b, errs) = Loader::default().compile(&name, src, up_to);
    if !errs.is_empty() {
        let text: String = errs.iter().map(|e| diag::format(e, &b.sources)).collect();
        return Out {
            stderr: restore_name(&text, &name, file),
            code: EXIT_COMPILE,
            ..Out::default()
        };
    }
    let stdout = match up_to {
        Stage::Ast => b.surface.as_ref().map(|n| format!("{n}\n")),
        Stage::Core => b.core.as_ref().map(|n| format!("{n}\n")),
        Stage::All => b.proto.as_ref().map(disassemble),
    };
    Out {
        stdout: stdout.unwrap_or_default().into_bytes(),
        code: EXIT_OK,
        ..Out::default()
    }
}

/// Puts the undecoded entry file name back into the `  --> NAME:` lines.
fn restore_name(text: &str, name: &str, raw: &[u8]) -> Vec<u8> {
    if name.as_bytes() == raw {
        return text.as_bytes().to_vec();
    }
    let from = format!("  --> {name}:");
    let mut out = Vec::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        match line.strip_prefix(&from) {
            Some(rest) => {
                out.extend_from_slice(b"  --> ");
                out.extend_from_slice(raw);
                out.push(b':');
                out.extend_from_slice(rest.as_bytes());
            }
            None => out.extend_from_slice(line.as_bytes()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_name_puts_raw_bytes_back_in_location_lines() {
        let text = "error[E0204]: undefined name `nope`\n  --> q\u{fffd}.fung:1:5\n  |\n1 | q\u{fffd}.fung:\n";
        let got = restore_name(text, "q\u{fffd}.fung", b"q\xfe.fung");
        let want: &[u8] =
            b"error[E0204]: undefined name `nope`\n  --> q\xfe.fung:1:5\n  |\n1 | q\xef\xbf\xbd.fung:\n";
        assert_eq!(got, want);
        assert_eq!(
            restore_name("  --> -e:1:1\n", "-e", b"-e"),
            b"  --> -e:1:1\n"
        );
    }

    #[test]
    fn non_utf8_arguments_are_decoded_and_echoed() {
        let out = run(&[b"dump".to_vec(), b"n\xff".to_vec()]);
        assert_eq!(out.code, EXIT_USAGE);
        assert!(
            out.stderr
                .starts_with(b"error: unknown dump stage `n\xff` (want")
        );
        let out = run(&[
            b"dump".to_vec(),
            b"core".to_vec(),
            b"-e".to_vec(),
            b"print(\"\xff\")".to_vec(),
        ]);
        assert_eq!(out.code, EXIT_OK);
        assert_eq!(
            out.stdout,
            "(do (call (prelude print) \"\u{fffd}\"))\n".as_bytes()
        );
    }
}
