//! The `fung` command: `fung dump`, `run`,
//! `<script.fung>`, `-e`, `check`, `test`, `fmt`, `version`, `help` and
//! the REPL (08).

mod args;
mod check;
mod fmt;
mod paths;
mod repl;
mod run;
mod test;

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};

use args::{Spec, parse, unknown_option};
use fung_front::bytecode::disassemble;
use fung_front::diag::{self, EXIT_COMPILE, EXIT_OK, EXIT_USAGE};
use fung_front::loader::{Loader, Stage};
use fung_front::source::{decode_lossy, decode_source};

/// The usage text (`fung help`, and after a usage error).
pub(crate) const USAGE: &str = "usage:
  fung <script.fung> [args...]            run a script (same as: fung run <script.fung>)
  fung                                    start the REPL
  fung -e '<code>'                        run one line of code
  fung check <script.fung>                check syntax and names without running
  fung check -e '<code>'                  the same for one line of code
  fung fmt [--check] <path>...|-          format files in place (--check: list only; -: stdin)
  fung test [path]                        run tests
  fung dump ast|core|bytecode <script>    print an internal stage
  fung dump ast|core|bytecode -e '<code>' the same for one line of code
  fung version                            print the version
  fung help                               print this text
";

/// The command runs on a thread with a large stack. The front end
/// recurses on the shape of the tree: deeply nested input (up to E0115)
/// and long flat chains (`1 + 1 + ...`, `x.b.b...`) recurse once per
/// link in the later passes, about 1 KiB each in a release build (64 MiB
/// takes some 68,000 links, 4 GiB millions; L9). At run time nothing
/// follows a value's or a call's depth on the Rust stack: fung calls,
/// natives that call fung (`fold`, `map`), and showing, comparing,
/// hashing and dropping values all keep their work on the heap (R2, L9).
/// 4 GiB is only reserved address space
/// (pages are committed when touched): deep recursion runs out of memory
/// long before it runs out of stack. A host that refuses the reservation
/// (little memory and swap, strict overcommit, `ulimit -v`) gets the next
/// smaller size (`fung_vm::stack::ENTRY_STACKS`,
/// docs/notes/R3a-runtime.md 2절).
const STACKS: &[usize] = fung_vm::stack::ENTRY_STACKS;

/// What a command produced: standard output, standard error, exit code.
/// The streams are bytes because arguments that are not UTF-8 are
/// echoed unchanged. A run writes to the process streams as it goes and leaves
/// both empty.
#[derive(Default)]
pub(crate) struct Out {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    code: i32,
}

fn main() {
    // Before any other thread: every thread inherits the block, and the
    // signal thread receives SIGINT and SIGTERM (`fung_vm::signals`).
    fung_vm::signals::init();
    // Room for thousands of connections (`fung_vm::rlimit`).
    fung_vm::rlimit::raise_open_files();
    // A Rust panic inside fung code becomes an E0801 report (the VM's
    // panic fence); the default hook would print the panic message on
    // top of that report.
    std::panic::set_hook(Box::new(|_| {}));
    let raw: Vec<Vec<u8>> = std::env::args_os()
        .skip(1)
        .map(OsString::into_vec)
        .collect();
    let out = start(STACKS, raw);
    let _ = run::ProcessOut::stdout().write_all(&out.stdout);
    let _ = run::ProcessOut::stderr().write_all(&out.stderr);
    std::process::exit(out.code);
}

/// Runs the command on a thread with the first stack size in `sizes`
/// the system accepts. If it accepts none, the result is one line on
/// standard error and exit code 1, not a panic (which the empty hook
/// above would make silent).
fn start(sizes: &[usize], raw: Vec<Vec<u8>>) -> Out {
    let started =
        fung_vm::stack::spawn_sized(std::thread::Builder::new, sizes, raw, |raw| run(&raw));
    match started {
        Ok(h) => h.join().unwrap_or_else(|_| Out {
            stderr: b"error: internal error in the fung command\n".to_vec(),
            code: diag::EXIT_RUNTIME,
            ..Out::default()
        }),
        Err((e, _)) => Out {
            stderr: format!("error: cannot start the fung command thread: {e}\n").into_bytes(),
            code: diag::EXIT_RUNTIME,
            ..Out::default()
        },
    }
}

/// `raw` are the arguments as the OS gave them. They are decoded (one
/// U+FFFD for each invalid UTF-8 sequence) for matching and as source
/// code, echoed unchanged in messages, and given to the script as they
/// are (`args`, L4).
fn run(raw: &[Vec<u8>]) -> Out {
    let args: Vec<String> = raw.iter().map(|a| decode_lossy(a)).collect();
    match args.first().map(String::as_str) {
        Some("dump") => run_dump(&args[1..], &raw[1..]),
        Some("run") => run_file(&args[1..], &raw[1..]),
        Some("-e") => match raw.get(1).map(|c| code_arg(c)) {
            Some(Ok(code)) => executed(run::execute("-e", &code, raw[2..].to_vec())),
            Some(Err(out)) => out,
            None => usage_error(b"-e needs code"),
        },
        Some("version" | "--version") => no_arguments("version", &args[1..], &raw[1..], || {
            format!("fung {}\n", fung_vm::version::version()).into_bytes()
        }),
        Some("help" | "-h" | "--help") => {
            no_arguments("help", &args[1..], &raw[1..], || USAGE.as_bytes().to_vec())
        }
        Some("check") => check::run_check(&args[1..], &raw[1..]),
        Some("test") => test::run_test(&raw[1..]),
        Some("fmt") => fmt::run_fmt(&args[1..], &raw[1..]),
        Some("--") => run_file(&args, raw),
        Some(a) if is_script_path(a) => run_file(&args, raw),
        Some(a) if a.starts_with('-') && a != "-" => unknown_option(&raw[0]),
        Some(_) => Out {
            stderr: [
                b"error: unknown command `",
                &raw[0][..],
                b"`\n\n",
                USAGE.as_bytes(),
            ]
            .concat(),
            code: EXIT_USAGE,
            ..Out::default()
        },
        None => repl::start(),
    }
}

/// `version` and `help`: the shared option rules, then no arguments.
fn no_arguments(name: &str, args: &[String], raw: &[Vec<u8>], text: impl Fn() -> Vec<u8>) -> Out {
    const SPEC: Spec = Spec {
        flags: &[],
        code: false,
        to_script: false,
    };
    match parse(args, raw, &SPEC) {
        Err(out) => out,
        Ok(p) if !p.raw.is_empty() => usage_error(format!("{name} takes no arguments").as_bytes()),
        Ok(_) => Out {
            stdout: text(),
            ..Out::default()
        },
    }
}

pub(crate) fn usage_error(msg: &[u8]) -> Out {
    Out {
        stderr: [b"error: ", msg, b"\n\n", USAGE.as_bytes()].concat(),
        code: EXIT_USAGE,
        ..Out::default()
    }
}

/// Whether a first argument is a script to run rather than a command
/// name: a script carries the .fung extension or is
/// written as a path.
fn is_script_path(arg: &str) -> bool {
    !arg.is_empty()
        && !arg.starts_with('-')
        && (arg.ends_with(".fung") || arg.contains(['/', '\\']))
}

/// The outcome of a run: its output is already written.
fn executed(code: i64) -> Out {
    Out {
        code: run::status(code),
        ..Out::default()
    }
}

/// `fung run FILE ARGS...` and `fung FILE ARGS...`: options (none but
/// `--` and `-h`) come before the path; after it everything is the
/// script's `args`. `args` and `raw` are those after `run`.
fn run_file(args: &[String], raw: &[Vec<u8>]) -> Out {
    const SPEC: Spec = Spec {
        flags: &[],
        code: false,
        to_script: true,
    };
    let p = match parse(args, raw, &SPEC) {
        Ok(p) => p,
        Err(out) => return out,
    };
    let Some(path) = p.raw.first() else {
        return usage_error(b"run needs a script path");
    };
    match read_file(path) {
        Ok(src) => executed(run::execute(&p.args[0], &src, p.raw[1..].to_vec())),
        Err(fail) => read_error(path, &fail),
    }
}

/// Why a file could not be read: the reason alone (`no such file or
/// directory`, `not valid UTF-8 at line 2, column 5`, as the `fs` module
/// words it) and whether the file does not exist.
pub(crate) struct ReadFail {
    pub reason: Vec<u8>,
    pub missing: bool,
}

/// `error: cannot read `PATH`: ...` for a script that cannot be read.
/// A path that does not exist is a usage error (3: a mistyped path); a
/// file that exists but cannot be read (permission, not UTF-8) is 2, in
/// every command (D30).
pub(crate) fn read_error(path: &[u8], fail: &ReadFail) -> Out {
    Out {
        stderr: [b"error: ", &cannot_read(path, &fail.reason)[..], b"\n"].concat(),
        code: if fail.missing {
            EXIT_USAGE
        } else {
            EXIT_COMPILE
        },
        ..Out::default()
    }
}

/// `fung dump STAGE FILE` and `fung dump STAGE -e CODE`.
/// `raw` holds the undecoded form of each of `args`.
fn run_dump(args: &[String], raw: &[Vec<u8>]) -> Out {
    const SPEC: Spec = Spec {
        flags: &[],
        code: true,
        to_script: false,
    };
    let p = match parse(args, raw, &SPEC) {
        Ok(p) => p,
        Err(out) => return out,
    };
    let (args, raw) = (&p.args, &p.raw);
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
    let rest = &raw[1..];
    if p.code_flag {
        return match (&p.code, rest.is_empty()) {
            (None, _) => usage_error(b"-e needs code"),
            // Nothing runs: arguments after the code have no use (TL4).
            (Some(_), false) => usage_error(b"dump takes one script path or -e code, nothing more"),
            (Some(code), true) => match code_arg(code) {
                Ok(code) => dump(up_to, b"-e", &code),
                Err(out) => out,
            },
        };
    }
    match rest {
        [] => usage_error(b"dump needs a script path or -e code"),
        [path] => match read_file(path) {
            Ok(src) => dump(up_to, path, &src),
            Err(fail) => read_error(path, &fail),
        },
        _ => usage_error(b"dump takes one script path or -e code, nothing more"),
    }
}

/// Reads a script, which must be UTF-8 (L4); [`cannot_read`] makes the
/// message from a failure.
pub(crate) fn read_file(path: &[u8]) -> Result<String, ReadFail> {
    read_bytes(path).and_then(decode)
}

/// Source bytes as text, or the reason they are not (D30: not a missing
/// file).
pub(crate) fn decode(bytes: Vec<u8>) -> Result<String, ReadFail> {
    decode_source(bytes).map_err(|why| ReadFail {
        reason: why.into_bytes(),
        missing: false,
    })
}

/// The code of `-e`, which is source text like a file's: not UTF-8 is
/// refused as [`read_file`] refuses it (L4).
pub(crate) fn code_arg(raw: &[u8]) -> Result<String, Out> {
    decode(raw.to_vec()).map_err(|fail| read_error(b"-e", &fail))
}

/// Reads a file's bytes; a failure is [`read_file`]'s.
pub(crate) fn read_bytes(path: &[u8]) -> Result<Vec<u8>, ReadFail> {
    std::fs::read(OsStr::from_bytes(path)).map_err(|e| ReadFail {
        reason: fung_vm::native::os_error_text(&e).into_bytes(),
        missing: e.kind() == std::io::ErrorKind::NotFound,
    })
}

/// ``cannot read `path`: reason``, the path's bytes as given (the `fs`
/// module's form).
pub(crate) fn cannot_read(path: &[u8], reason: &[u8]) -> Vec<u8> {
    [b"cannot read `", path, b"`: ", reason].concat()
}

/// Prints one stage of a source text. `file` is the name as
/// given; diagnostics compile under its decoded form, and the location
/// lines of the entry file get the original bytes back.
fn dump(up_to: Stage, file: &[u8], src: &str) -> Out {
    let name = decode_lossy(file);
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
pub(crate) fn restore_name(text: &str, name: &str, raw: &[u8]) -> Vec<u8> {
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
    fn non_utf8_arguments_are_echoed_and_non_utf8_code_refused() {
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
        // The code is source text: not UTF-8 is refused (L4).
        assert_eq!(out.code, EXIT_COMPILE);
        assert_eq!(
            out.stderr,
            b"error: cannot read `-e`: not valid UTF-8 at line 1, column 8\n"
        );
    }

    /// Arguments a command has no use for are a usage error everywhere
    /// (TL4), with a message that says what the command takes.
    #[test]
    fn extra_arguments_are_usage_errors() {
        let cases: &[(&[&str], &str)] = &[
            (&["version", "x"], "version takes no arguments"),
            (&["help", "x"], "help takes no arguments"),
            (&["--help", "x"], "help takes no arguments"),
            (
                &["dump", "core", "-e", "1", "x"],
                "dump takes one script path or -e code, nothing more",
            ),
            (
                &["dump", "core", "a.fung", "x"],
                "dump takes one script path or -e code, nothing more",
            ),
            (&["dump", "core"], "dump needs a script path or -e code"),
            (&["dump", "core", "-e"], "-e needs code"),
            (
                &["check", "a.fung", "x"],
                "check takes one script path or -e code, nothing more",
            ),
            (
                &["check", "-e", "1", "x"],
                "check takes one script path or -e code, nothing more",
            ),
            (&["check"], "check needs a script path or -e code"),
            (&["check", "-e"], "-e needs code"),
        ];
        for (argv, msg) in cases {
            let raw: Vec<Vec<u8>> = argv.iter().map(|a| a.as_bytes().to_vec()).collect();
            let out = run(&raw);
            assert_eq!(out.code, EXIT_USAGE, "{argv:?}");
            let err = String::from_utf8_lossy(&out.stderr);
            assert!(
                err.starts_with(&format!("error: {msg}\n\nusage:")),
                "{argv:?}: {err}"
            );
        }
        // The usage names `-e` for check and dump.
        assert!(USAGE.contains("fung check -e '<code>'"));
        assert!(USAGE.contains("fung dump ast|core|bytecode -e '<code>'"));
    }

    /// One option parser for every command (TL7): `-h` and `--help` are
    /// the usage, `--version` is `version`, an unknown option is the same
    /// words everywhere, and `--` ends the options.
    #[test]
    fn options_are_parsed_the_same_in_every_command() {
        let argv = |a: &[&str]| -> Out {
            run(&a.iter().map(|x| x.as_bytes().to_vec()).collect::<Vec<_>>())
        };
        for cmd in ["run", "check", "test", "fmt", "dump"] {
            let out = argv(&[cmd, "--help"]);
            assert_eq!((out.code, &out.stdout[..]), (EXIT_OK, USAGE.as_bytes()));
            let out = argv(&[cmd, "-h"]);
            assert_eq!((out.code, &out.stdout[..]), (EXIT_OK, USAGE.as_bytes()));
            let out = argv(&[cmd, "--foo"]);
            assert_eq!(out.code, EXIT_USAGE, "{cmd}");
            assert!(
                out.stderr
                    .starts_with(b"error: unknown option `--foo`\n\nusage:"),
                "{cmd}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let out = argv(&["dump", "core", "--foo"]);
        assert!(out.stderr.starts_with(b"error: unknown option `--foo`"));
        let out = argv(&["--version"]);
        assert_eq!(out.code, EXIT_OK);
        assert!(out.stdout.starts_with(b"fung "));
        let out = argv(&["--version", "x"]);
        assert!(out.stderr.starts_with(b"error: version takes no arguments"));
        let out = argv(&["--foo"]);
        assert_eq!(out.code, EXIT_USAGE);
        assert!(out.stderr.starts_with(b"error: unknown option `--foo`"));
        // `--` ends the options: what follows is a path, however it starts
        let out = argv(&["check", "--", "--foo"]);
        assert_eq!(out.code, EXIT_USAGE);
        assert!(
            out.stderr
                .starts_with(b"error: cannot read `--foo`: no such file"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        // the code of -e is code, even when it looks like an option
        let out = argv(&["check", "-e", "--foo"]);
        assert!(out.stderr.starts_with(b"error[E"), "{:?}", out.stderr);
        // after the script's path the options are the script's
        let out = argv(&["run", "--", "nope.fung", "--foo"]);
        assert!(out.stderr.starts_with(b"error: cannot read `nope.fung`"));
        // fmt reads standard input only alone
        for a in [
            &["fmt", "-", "-"][..],
            &["fmt", "-", "a.fung"],
            &["fmt", "a.fung", "-"],
        ] {
            let out = argv(a);
            assert_eq!(out.code, EXIT_USAGE, "{a:?}");
            assert!(
                out.stderr.starts_with(
                    b"error: fmt reads standard input only when `-` is the only path\n"
                ),
                "{a:?}"
            );
        }
    }

    /// `--` ends the options for `-e` too, `version` and `help` read
    /// options like every command, and fmt's 1 means only "needs fixing":
    /// a failed write is 2 (D30).
    #[test]
    fn double_dash_protects_e_and_version_reads_options() {
        use std::os::unix::fs::PermissionsExt;
        let argv = |a: &[&str]| -> Out {
            run(&a.iter().map(|x| x.as_bytes().to_vec()).collect::<Vec<_>>())
        };
        for cmd in [&["check"][..], &["dump", "ast"]] {
            let mut a = cmd.to_vec();
            a.extend(["--", "-e"]);
            let out = argv(&a);
            assert_eq!(out.code, EXIT_USAGE, "{a:?}");
            assert!(
                out.stderr
                    .starts_with(b"error: cannot read `-e`: no such file"),
                "{a:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        for a in [
            &["version", "--help"][..],
            &["help", "--help"],
            &["--version", "-h"],
        ] {
            let out = argv(a);
            assert_eq!(
                (out.code, &out.stdout[..]),
                (EXIT_OK, USAGE.as_bytes()),
                "{a:?}"
            );
        }
        let out = argv(&["version", "--foo"]);
        assert!(out.stderr.starts_with(b"error: unknown option `--foo`"));
        let out = argv(&["version", "--", "x"]);
        assert!(out.stderr.starts_with(b"error: version takes no arguments"));

        let dir = std::env::temp_dir().join(format!("fung-d30w-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let _guard = Cleanup(dir.clone());
        let file = dir.join("a.fung");
        std::fs::write(&file, "x=1\n").unwrap();
        // neither the file nor its directory takes a write
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let writable = std::fs::write(dir.join("probe"), "").is_ok();
        if !writable {
            let out = argv(&["fmt", file.to_str().unwrap()]);
            assert_eq!(
                out.code,
                EXIT_COMPILE,
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(out.stderr.starts_with(b"error: cannot write"));
        }
    }

    /// Restores a directory's mode and removes it, even if a test panics.
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;
            let mut stack = vec![self.0.clone()];
            while let Some(p) = stack.pop() {
                let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755));
                if let Ok(rd) = std::fs::read_dir(&p) {
                    stack.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
                }
            }
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A path that does not exist is 3; a file that exists and cannot be
    /// read (permission, not UTF-8) is 2, in every command (D30).
    #[test]
    fn unreadable_files_are_exit_2_and_missing_ones_3() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("fung-d30-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _guard = Cleanup(dir.clone());
        let bad = dir.join("bad.fung");
        std::fs::write(&bad, b"x = \"\xff\"\n").unwrap();
        let locked = dir.join("locked.fung");
        std::fs::write(&locked, "x = 1\n").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let can_read_locked = std::fs::read(&locked).is_ok();
        let p = |f: &std::path::Path| f.to_str().unwrap().to_string();
        let missing = p(&dir.join("nope.fung"));
        let argv = |a: &[&str]| -> Out {
            run(&a.iter().map(|x| x.as_bytes().to_vec()).collect::<Vec<_>>())
        };
        for cmd in [
            &["run"][..],
            &["check"],
            &["dump", "ast"],
            &["fmt"],
            &["fmt", "--check"],
        ] {
            let with = |f: &str| -> Out {
                let mut a = cmd.to_vec();
                a.push(f);
                argv(&a)
            };
            assert_eq!(with(&missing).code, EXIT_USAGE, "{cmd:?} missing");
            let out = with(&p(&bad));
            assert_eq!(out.code, EXIT_COMPILE, "{cmd:?} not UTF-8");
            assert!(
                String::from_utf8_lossy(&out.stderr)
                    .contains("not valid UTF-8 at line 1, column 6"),
                "{cmd:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            // nothing is written back
            assert_eq!(std::fs::read(&bad).unwrap(), b"x = \"\xff\"\n");
            if !can_read_locked {
                assert_eq!(with(&p(&locked)).code, EXIT_COMPILE, "{cmd:?} locked");
            }
        }
    }

    /// More than any 64-bit host maps: every system refuses it.
    const REFUSED: usize = 1 << 47;

    /// A host that refuses the big stacks runs the command on a smaller
    /// one (6a review: it used to exit 101 without a word).
    #[test]
    fn the_command_falls_back_to_a_smaller_stack() {
        let out = start(&[REFUSED, REFUSED, 64 << 20], vec![b"version".to_vec()]);
        assert_eq!(out.code, EXIT_OK);
        assert!(out.stdout.starts_with(b"fung "));
    }

    #[test]
    fn no_stack_size_fits_is_one_line_and_exit_1() {
        let out = start(&[REFUSED, REFUSED], vec![b"version".to_vec()]);
        assert_eq!(out.code, diag::EXIT_RUNTIME);
        assert!(out.stdout.is_empty());
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(
            err.starts_with("error: cannot start the fung command thread: "),
            "{err}"
        );
        assert!(
            err.ends_with('\n') && err.matches('\n').count() == 1,
            "{err}"
        );
    }
}
