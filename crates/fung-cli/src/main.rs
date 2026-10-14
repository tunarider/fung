//! The `fung` command of the Rust port. Only `fung dump` exists so far;
//! it mirrors `cmd/fung` of the Go implementation byte for byte.

use std::io::Write;

use fung_front::bytecode::disassemble;
use fung_front::diag::{self, EXIT_COMPILE, EXIT_OK, EXIT_USAGE};
use fung_front::loader::{Loader, Stage};

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
/// thread with a large stack (deeply nested input must not overflow).
const STACK: usize = 512 << 20;

/// What a command produced: standard output, standard error, exit code.
#[derive(Default)]
struct Out {
    stdout: String,
    stderr: String,
    code: i32,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || run(&args))
        .expect("spawn the front-end thread")
        .join()
        .unwrap_or_else(|_| Out {
            stderr: "error: internal error in the front end\n".into(),
            code: diag::EXIT_RUNTIME,
            ..Out::default()
        });
    let _ = std::io::stdout().write_all(out.stdout.as_bytes());
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().write_all(out.stderr.as_bytes());
    std::process::exit(out.code);
}

fn run(args: &[String]) -> Out {
    match args.first().map(String::as_str) {
        Some("dump") => run_dump(&args[1..]),
        Some(other) => Out {
            stderr: format!("not implemented yet in the Rust port: {other}\n"),
            code: EXIT_USAGE,
            ..Out::default()
        },
        None => Out {
            stderr: "not implemented yet in the Rust port: the REPL\n".into(),
            code: EXIT_USAGE,
            ..Out::default()
        },
    }
}

fn usage_error(msg: &str) -> Out {
    Out {
        stderr: format!("error: {msg}\n\n{USAGE}"),
        code: EXIT_USAGE,
        ..Out::default()
    }
}

/// `fung dump STAGE FILE` and `fung dump STAGE -e CODE` (Go `runDump`).
fn run_dump(args: &[String]) -> Out {
    let Some(stage) = args.first() else {
        return usage_error("dump needs a stage: ast, core or bytecode");
    };
    let up_to = match stage.as_str() {
        "ast" => Stage::Ast,
        "core" => Stage::Core,
        "bytecode" => Stage::All,
        _ => {
            return usage_error(&format!(
                "unknown dump stage `{stage}` (want ast, core or bytecode)"
            ));
        }
    };
    match &args[1..] {
        [e, rest @ ..] if e == "-e" => match rest.first() {
            Some(code) => dump(up_to, "-e", code),
            None => usage_error("-e needs code"),
        },
        [path] => match read_file(path) {
            Ok(src) => dump(up_to, path, &src),
            Err(msg) => Out {
                stderr: format!("error: cannot read {path}: {msg}\n"),
                code: EXIT_USAGE,
                ..Out::default()
            },
        },
        _ => usage_error("dump needs a script path or -e code"),
    }
}

/// Reads a script, describing a failure the way Go's `os.ReadFile` does.
fn read_file(path: &str) -> Result<String, String> {
    use std::io::ErrorKind;
    let bytes = std::fs::read(path).map_err(|e| match e.kind() {
        ErrorKind::NotFound => format!("open {path}: no such file or directory"),
        ErrorKind::PermissionDenied => format!("open {path}: permission denied"),
        ErrorKind::IsADirectory => format!("read {path}: is a directory"),
        _ => {
            let text = e.to_string();
            let text = text.split(" (os error").next().unwrap_or(&text);
            format!("open {path}: {}", text.to_lowercase())
        }
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Prints one stage of a source text (Go `dump`).
fn dump(up_to: Stage, file: &str, src: &str) -> Out {
    let (b, errs) = Loader::default().compile(file, src, up_to);
    if !errs.is_empty() {
        return Out {
            stderr: errs.iter().map(|e| diag::format(e, &b.sources)).collect(),
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
        stdout: stdout.unwrap_or_default(),
        code: EXIT_OK,
        ..Out::default()
    }
}
