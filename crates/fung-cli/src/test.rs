//! `fung test [path]` (08): every `.fung` file
//! below the path (default `.`) is compiled with its test blocks and, if
//! it has any, runs as an entry file on a runtime of its own: the top
//! level first, then each test block in order. A file without tests is
//! neither run nor listed.
//!
//! The end of each file, in order: the program's tasks are cancelled and
//! waited for (unwinding through their `with` releases), and the runtime
//! is dropped.
//! So nothing a file starts reaches the next one.
//!
//! Exit codes: 2 if any file failed to compile, else 1 if any test
//! failed, else 0. An `exit(n)` inside a test file is a failure and skips
//! the rest of that file; its code is not the command's. So does an exit
//! a spawned task asks for, or a deadlock; when
//! that skip follows a block that passed, it is a failure of its own
//! (`<rest of file>`), as one before the first block fails the top level.

use std::collections::HashSet;
use std::io::Write;

use fung_front::bytecode::{Op, Proto};
use fung_front::diag::{self, Diag, EXIT_COMPILE, EXIT_OK, EXIT_RUNTIME, EXIT_USAGE};
use fung_front::loader::{Loader, Stage};
use fung_front::source::{SourceMap, decode_lossy};
use fung_vm::vm::{Runtime, Stop};

use crate::run::ProcessOut;
use crate::{Out, USAGE, paths, read_file, restore_name};

/// What one file added to the summary.
#[derive(Default)]
struct Tally {
    passed: usize,
    failed: usize,
    compile_failed: bool,
}

/// `args` are the arguments after `test`, as bytes. The output goes to
/// the process streams as the files run (the files' own output among
/// it); the returned `Out` carries only a usage error and the exit code.
pub fn run_test(raw: &[Vec<u8>]) -> Out {
    let root: &[u8] = match raw {
        [] => b".",
        [p] => p,
        _ => {
            return Out {
                stderr: [b"error: test takes at most one path\n\n", USAGE.as_bytes()].concat(),
                code: EXIT_USAGE,
                ..Out::default()
            };
        }
    };
    let files = match paths::test_files(root) {
        Ok(f) => f,
        Err(e) => {
            return Out {
                stderr: [b"error: ", &e[..], b"\n"].concat(),
                code: EXIT_USAGE,
                ..Out::default()
            };
        }
    };
    let mut total = Tally::default();
    // Errors already printed: a broken library is reported once.
    let mut seen = HashSet::new();
    // The first SIGINT or SIGTERM is a stop request to the file running
    // then; no file runs after it, and the run ends with 128 + n.
    let first = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(0));
    for path in &files {
        let t = test_file(path, &mut seen, &first);
        total.passed += t.passed;
        total.failed += t.failed;
        total.compile_failed |= t.compile_failed;
        if first.load(std::sync::atomic::Ordering::SeqCst) != 0 {
            break;
        }
    }
    fung_vm::signals::set(None);
    let sig = first.load(std::sync::atomic::Ordering::SeqCst);
    let _ = writeln!(
        ProcessOut::stdout(),
        "{} passed, {} failed",
        total.passed,
        total.failed
    );
    let code = if sig != 0 {
        128 + sig
    } else if total.compile_failed {
        EXIT_COMPILE
    } else if total.failed > 0 {
        EXIT_RUNTIME
    } else {
        EXIT_OK
    };
    Out {
        code,
        ..Out::default()
    }
}

/// Compiles one file with its tests and runs it.
fn test_file(
    path: &[u8],
    seen: &mut HashSet<String>,
    first: &std::sync::Arc<std::sync::atomic::AtomicI32>,
) -> Tally {
    let mut t = Tally::default();
    let stdout = &mut ProcessOut::stdout();
    let stderr = &mut ProcessOut::stderr();
    let src = match read_file(path) {
        Ok(s) => s,
        Err(msg) => {
            // a file that cannot be read fails like one that cannot load
            let _ = stdout.write_all(&[b"FAIL ", path, b": <load>\n"].concat());
            let _ = stderr
                .write_all(&[b"error: ", &crate::cannot_read(path, &msg)[..], b"\n"].concat());
            t.compile_failed = true;
            t.failed += 1;
            return t;
        }
    };
    let name = decode_lossy(path);
    let loader = Loader {
        tests: true,
        ..Loader::default()
    };
    let (b, errs) = loader.compile(&name, &src, Stage::All);
    let shown = |errs: &[Diag], sources: &SourceMap| -> Vec<u8> {
        let text: String = errs.iter().map(|e| diag::format(e, sources)).collect();
        restore_name(&text, &name, path)
    };
    if !errs.is_empty() {
        // A file that cannot load is a failure in the summary; an error
        // shared with other files (a broken import) is printed once. The
        // key names the file by its name: ids are local to each compile.
        let _ = stdout.write_all(&[b"FAIL ", path, b": <load>\n"].concat());
        let fresh: Vec<Diag> = errs
            .into_iter()
            .filter(|e| {
                let file = if e.pos.is_zero() {
                    ""
                } else {
                    b.sources.name(e.pos.file)
                };
                seen.insert(format!("{file}:{}:{}:{}", e.pos.line, e.pos.col, e.code))
            })
            .collect();
        let _ = stderr.write_all(&shown(&fresh, &b.sources));
        t.compile_failed = true;
        t.failed += 1;
        return t;
    }
    let entry = b.proto.expect("a compiled entry");
    if !has_tests(&entry) {
        return t; // a plain script: not run, not listed (08)
    }
    let _ = stderr.write_all(&shown(&b.warnings, &b.sources));
    let rt = Runtime::new(
        Box::new(ProcessOut::stdout()),
        Box::new(ProcessOut::stderr()),
        Box::new(std::io::BufReader::new(std::io::empty())),
        b.sources,
    );
    if let Err(errs) = rt.boot(Vec::new()) {
        let _ = stdout.write_all(&[b"FAIL ", path, b": <load>\n"].concat());
        let _ = stderr.write_all(&rt.with_sources(|s| shown(&errs, s)));
        t.compile_failed = true;
        t.failed += 1;
        return t;
    }
    rt.load(&entry, &b.modules);
    fung_vm::signals::set(Some(fung_vm::signals::stop_requests(
        rt.stop_handle(),
        first.clone(),
    )));
    // A signal that cut the file short is named as one, not as the
    // exit(128 + n) or the cancellation it made.
    let signalled = || match first.load(std::sync::atomic::Ordering::SeqCst) {
        0 => None,
        sig => Some(format!(
            "  {} stopped the run; the rest of the file was skipped\n",
            signal_name(sig)
        )),
    };
    // The FAIL lines of a test block (or the top level) that stopped.
    let failure = |test: &str, stop: &Stop| {
        let mut line = [b"FAIL ", path, b": ", test.as_bytes(), b"\n"].concat();
        if let Some(note) = signalled() {
            line.extend_from_slice(note.as_bytes());
            return line;
        }
        match stop {
            Stop::Exit(n) => line.extend_from_slice(
                format!(
                    "  exit({n}) called inside a test file; the rest of the file was skipped\n"
                )
                .as_bytes(),
            ),
            Stop::Error(d) => {
                let text = rt.with_sources(|s| shown(std::slice::from_ref(&**d), s));
                line.extend_from_slice(&indent(&text));
            }
        }
        line
    };
    // An exit asked for by a task the block did not wait out, or a
    // deadlock (which asks for exit(1)), cancels the file's program like
    // a direct exit(n): the rest of the file is skipped (08), since the
    // blocks after it would run on a cancelled program.
    let skipped = |rt: &Runtime| {
        let note = if let Some(note) = signalled() {
            rt.exit_code()?;
            note
        } else if rt.deadlocked() {
            "  a deadlock was reported; the rest of the file was skipped\n".to_string()
        } else {
            let n = rt.exit_code()?;
            format!("  exit({n}) called inside a test file; the rest of the file was skipped\n")
        };
        Some(note)
    };
    match rt.run() {
        Err(stop) => {
            let _ = stdout.write_all(&failure("<top level>", &stop));
            t.failed += 1;
        }
        Ok(()) => {
            let tests = rt.tests();
            if let (false, Some(note)) = (tests.is_empty(), skipped(&rt)) {
                let _ = stdout
                    .write_all(&[b"FAIL ", path, b": <top level>\n", note.as_bytes()].concat());
                t.failed += 1;
            } else {
                for (i, test) in tests.iter().enumerate() {
                    // A stop request in one block does not reach the next.
                    rt.reset_stop();
                    let passed = match rt.call_test(i) {
                        Ok(()) => {
                            let _ = stdout.write_all(
                                &[b"ok   ", path, b": ", test.as_bytes(), b"\n"].concat(),
                            );
                            t.passed += 1;
                            true
                        }
                        Err(stop) => {
                            let _ = stdout.write_all(&failure(test, &stop));
                            t.failed += 1;
                            if matches!(stop, Stop::Exit(_)) {
                                break;
                            }
                            false
                        }
                    };
                    // Only when blocks remain: after the last block there
                    // is nothing to skip. After a block that passed, the
                    // skip is a failure of its own, as at the top level:
                    // a run never ends silently (08).
                    if i + 1 < tests.len()
                        && let Some(note) = skipped(&rt)
                    {
                        if passed {
                            let _ =
                                stdout.write_all(&[b"FAIL ", path, b": <rest of file>\n"].concat());
                            t.failed += 1;
                        }
                        let _ = stdout.write_all(note.as_bytes());
                        break;
                    }
                }
            }
        }
    }
    // The file is done: cancel what its code left running and wait for
    // it (unwinding through its releases). A task failure nobody handled
    // (a cancelled `supervise`'s queue) fails the file (05).
    rt.cancel_all();
    rt.wait();
    if rt.failed() {
        let _ = stdout.write_all(
            &[
                b"FAIL ",
                path,
                b": <tasks>\n  a task failure was not handled (see standard error)\n",
            ]
            .concat(),
        );
        t.failed += 1;
    }
    t
}

/// A stop signal's name.
fn signal_name(sig: i32) -> String {
    match sig {
        2 => "SIGINT".into(),
        15 => "SIGTERM".into(),
        n => format!("signal {n}"),
    }
}

/// Whether the entry body registers any test (tests are
/// top-level only, so the entry's own code is enough).
fn has_tests(p: &Proto) -> bool {
    p.code.iter().any(|i| i.op() == Op::Test)
}

/// Prefixes every line with two spaces.
fn indent(s: &[u8]) -> Vec<u8> {
    let s = s.strip_suffix(b"\n").unwrap_or(s);
    let mut out = Vec::with_capacity(s.len() + 16);
    for line in s.split(|&c| c == b'\n') {
        out.extend_from_slice(b"  ");
        out.extend_from_slice(line);
        out.push(b'\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indent_prefixes_every_line() {
        assert_eq!(indent(b"a\nb\n"), b"  a\n  b\n");
        assert_eq!(indent(b"a\n\nb"), b"  a\n  \n  b\n");
        assert_eq!(indent(b""), b"  \n");
    }

    #[test]
    fn arguments() {
        let out = run_test(&[b"a".to_vec(), b"b".to_vec()]);
        assert_eq!(out.code, EXIT_USAGE);
        assert!(
            out.stderr
                .starts_with(b"error: test takes at most one path\n\nusage:")
        );
        let out = run_test(&[b"/nonexistent/fung-test-path".to_vec()]);
        assert_eq!(out.code, EXIT_USAGE);
        assert_eq!(
            out.stderr,
            b"error: cannot read `/nonexistent/fung-test-path`: no such file or directory\n"
        );
    }
}
