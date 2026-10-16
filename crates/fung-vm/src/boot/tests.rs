//! Boot tests: ports of Go `internal/vm/prelude_test.go`
//! `TestProcessFunctions`, `TestFungPreludeFunctions` and
//! `TestTakeDropCheckTheirCount`, the concurrency natives, the
//! parameter tables against `spec/builtins.txt`, and the positions of
//! errors raised inside `extras/prelude.fung` (expected text from the Go
//! binary).

use std::collections::BTreeSet;
use std::io::Cursor;

use fung_front::builtins::builtins;
use fung_front::source::SourceMap;

use crate::testutil::{Opts, exec, on_big_stack, run_booted, run_file, temp_dir};
use crate::value::{NativeParam, Value, format};
use crate::vm::Runtime;

fn render_params(ps: &[NativeParam]) -> String {
    ps.iter()
        .map(|p| {
            if p.rest {
                format!("...{}", p.name)
            } else if p.default.is_some() {
                format!("{}=?", p.name)
            } else {
                p.name.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn concurrency_builtin_errors_have_a_position() {
    // A native's error points at the call's `(` like every other native
    // error in Go (`spawn(1)` → E0403 at 2:6); so does the error of the
    // fung `deref` (stage 3b, `extras/rust/agent.fung`).
    for (src, want) in [
        (
            "print(1)\nspawn(1)",
            "error[E0403]: argument `f` must be fn, got int\n  --> -e:2:6\n",
        ),
        (
            "print(1)\nderef(1)",
            "error[E0403]: argument `a` must be agent, got int\n  --> -e:2:6\n",
        ),
    ] {
        let (out, err, code) = run_booted(src);
        assert_eq!(out, "1\n");
        assert!(err.starts_with(want), "{err}");
        assert_eq!(code, 1);
    }
}

#[test]
fn fung_prelude_and_process_functions() {
    let (out, _, code) = run_booted("print(take([1, 2, 3], 2), sh(\"echo hi\")!.out)");
    assert_eq!((out.as_str(), code), ("[1, 2] hi\n\n", 0));
}

#[test]
fn agents_run() {
    // Stage 3b: agents (`extras/rust/agent.fung`) replace atoms; no
    // concurrency builtin gives E0805.
    let (out, err, code) = run_booted("a = agent(1)\nprint(swap(a, x => x + 1), deref(a), a)");
    assert_eq!((out.as_str(), err.as_str(), code), ("2 2 <agent>\n", "", 0));
    let (_, err, code) = run_booted("atom(1)");
    assert!(
        err.starts_with("error[E0204]: undefined name `atom`"),
        "{err}"
    );
    assert_eq!(code, 2);
    for (name, _) in crate::conc::natives() {
        let src = match name {
            "channel" => "channel(-1)".to_string(),
            "send" => format!("{name}(1, 2)"),
            _ => format!("{name}(1)"),
        };
        let o = exec(&src, Opts::default());
        assert_ne!(o.code, "E0805", "{src}");
    }
}

/// The fung helpers built on the concurrency natives fail at the user's
/// call (the extras frames are builtin, Go `attach`), with the extras
/// frames in the call path. (`pmap` and `fan_in` run since stage 3a: see
/// `pmap_runs_its_tasks` and `fan_in_merges_its_channels`.)
#[test]
fn fung_helpers_over_concurrency_fail_at_the_user_call() {
    let cases = [("fan_in(1)", 1, 7), ("print(1)\nfan_in(1)", 2, 7)];
    for (src, line, col) in cases {
        let o = exec(src, Opts::default());
        let e = o.error.unwrap_or_else(|| panic!("{src}: no error"));
        assert_eq!(
            (e.headline.as_str(), e.line, e.col),
            ("error[E0506]: cannot take the length of int", line, col),
            "{src}"
        );
        assert!(
            e.path.iter().any(|c| c.file == "extras/prelude.fung"),
            "{src}: {:?}",
            e.path
        );
        assert_eq!(e.path.last().map(|c| c.file.as_str()), Some("-e"), "{src}");
    }
}

#[test]
fn booted_prelude_has_every_spec_name_and_spec_parameters() {
    on_big_stack(|| {
        let rt = Runtime::new(
            Box::new(std::io::sink()),
            Box::new(std::io::sink()),
            Box::new(Cursor::new(Vec::new())),
            SourceMap::default(),
        );
        rt.boot(vec!["a".into(), "b c".into()]).expect("boot");
        rt.with_prelude(|p| {
            // The spec's names with the Rust differences applied
            // (`fung_front::builtins`: no atom natives, the agent names).
            let spec = builtins();
            let names = spec.prelude_names();
            let want: BTreeSet<&str> = names.iter().map(String::as_str).collect();
            let got: BTreeSet<&str> = p.0.keys().map(String::as_str).collect();
            assert_eq!(got, want);
            for name in &spec.prelude_native {
                let Value::Native(n) = &p.0[name] else {
                    assert_eq!(name, "args");
                    continue;
                };
                assert_eq!(n.name, name);
                let line = spec.native_signature(name).expect(name);
                assert_eq!(render_params(&n.params), line, "{name}");
            }
            assert_eq!(format(&p.0["args"]), "[\"a\", \"b c\"]");
            assert_eq!(format(&p.0["take"]), "<fn take>");
            for name in ["agent", "swap", "deref"] {
                assert_eq!(format(&p.0[name]), format!("<fn {name}>"));
            }
        });
        assert_eq!(rt.args(), ["a", "b c"]);
    });
}

#[test]
fn native_modules_match_the_spec() {
    on_big_stack(|| {
        let spec = builtins();
        for (key, m) in [
            ("_native.conc", super::super::conc::module()),
            ("_native.process", crate::native::process::module()),
            ("_native.fs", crate::native::fs::module()),
            ("_native.path", crate::native::path::module()),
            ("_native.time", crate::native::time::module()),
            ("_native.json", crate::native::json::module()),
        ] {
            assert_eq!(m.key, key);
            let names: Vec<&str> = m.names();
            let want: Vec<&str> = spec.modules[key].iter().map(String::as_str).collect();
            let mut sorted = names.clone();
            sorted.sort();
            assert_eq!(sorted, want, "{key}");
            for name in names {
                let Some(Value::Native(n)) = m.get(name) else {
                    panic!("{key}.{name} is not a native")
                };
                assert_eq!(
                    render_params(&n.params),
                    spec.module_signature(key, name).expect(name),
                    "{key}.{name}"
                );
            }
        }
    });
}

#[test]
fn script_args_reach_the_program() {
    let o = exec(
        "print(args, len(args))",
        Opts {
            args: &["x", "y"],
            ..Opts::default()
        },
    );
    assert_eq!(o.out, "[\"x\", \"y\"] 2\n");
}

// --- Go TestProcessFunctions ---

#[test]
fn process_functions() {
    let cases: &[(&str, &str)] = &[
        (
            "print(exec([\"echo\", \"hi there\"]))",
            "Ok({out: \"hi there\\n\", err: \"\", code: 0})\n",
        ),
        (
            "print(sh(\"echo a; echo b >&2; exit 3\"))",
            "Err({kind: 'exit, code: 3, out: \"a\\n\", err: \"b\\n\", message: \"`/bin/sh` exited with code 3\"})\n",
        ),
        ("print(exec([\"cat\"], stdin: \"x\")!.out)", "x\n"),
        (
            "print(exec([\"sh\", \"-c\", \"echo $FOO\"], env: {\"FOO\": \"bar\"})!.out)",
            "bar\n\n",
        ),
        ("print(exec([\"pwd\"], cwd: \"/\")!.out)", "/\n\n"),
        (
            "match exec([\"definitely-not-a-command-xyz\"]): Err({kind: k}) => print(k); _ => print(\"?\") ;;",
            "'not_found\n",
        ),
        (
            "print(env(\"FUNG_TEST_UNSET_VAR\"), type_of(env()))",
            "nil 'map\n",
        ),
        ("print(args)", "[]\n"),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", *want), "{src}");
    }
    for (src, want) in [
        ("exec([])", "E0403"),
        ("exec([1])", "E0403"),
        ("exec(\"ls\")", "E0403"),
        ("exec([\"ls\"], cwd: 1)", "E0403"),
    ] {
        assert_eq!(exec(src, Opts::default()).code, want, "{src}");
    }
}

/// Failures to start a command, as the Go binary reports them.
#[test]
fn commands_that_cannot_run_are_not_found_like_go() {
    let cases: &[(&str, &str)] = &[
        (
            "print(exec([\"definitely-not-a-command-xyz\"]))",
            "Err({kind: 'not_found, message: \"cannot run `definitely-not-a-command-xyz`: executable file not found in $PATH\"})\n",
        ),
        (
            "print(exec([\"/nonexistent/x\"]))",
            "Err({kind: 'not_found, message: \"cannot run `/nonexistent/x`: no such file or directory\"})\n",
        ),
        (
            "print(exec([\"/etc\"]))",
            "Err({kind: 'not_found, message: \"cannot run `/etc`: permission denied\"})\n",
        ),
        (
            "print(exec([\"\"]))",
            "Err({kind: 'not_found, message: \"cannot run ``: exec: no command\"})\n",
        ),
        (
            "print(exec([\"ls\"], cwd: \"/nonexistent\"))",
            "Err({kind: 'not_found, message: \"cannot run `ls`: no such file or directory\"})\n",
        ),
        (
            "print(exec([\"ls\"], cwd: \"/etc/hosts\"))",
            "Err({kind: 'not_found, message: \"cannot run `ls`: not a directory\"})\n",
        ),
        (
            "print(sh(\"kill -9 $$\"))",
            "Err({kind: 'exit, code: -1, out: \"\", err: \"\", message: \"`/bin/sh` exited with code -1\"})\n",
        ),
        (
            "print(exec([\"sh\", \"-c\", \"echo\\u{0}\"]))",
            "Err({kind: 'not_found, message: \"cannot run `sh`: invalid argument\"})\n",
        ),
        (
            "print(exec([\"sh\"], env: {\"A\": \"x\\u{0}\"}))",
            "Err({kind: 'not_found, message: \"cannot run `sh`: exec: environment variable contains NUL\"})\n",
        ),
        (
            "print(exec([\"ls\"], cwd: \"/\\u{0}\"))",
            "Err({kind: 'not_found, message: \"cannot run `ls`: exec: environment variable contains NUL\"})\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", *want), "{src}");
    }
}

// --- Go TestFungPreludeFunctions ---

#[test]
fn fung_prelude_functions() {
    let cases: &[(&str, &str)] = &[
        (
            "ch = channel(3)\nsend(ch, 1)\nsend(ch, 2)\nclose(ch)\nprint(first(ch))",
            "1\n",
        ),
        (
            "ch = channel(3)\nsend(ch, 1)\nsend(ch, 2)\nclose(ch)\nprint(last(ch))",
            "2\n",
        ),
        (
            "xs = [1, 2, 3]\nprint(take(xs, 2), take(xs, 9), take(xs, -1), drop(xs, 1), drop(xs, 9), drop(xs, -1), xs)",
            "[1, 2] [1, 2, 3] [] [2, 3] [] [1, 2, 3] [1, 2, 3]\n",
        ),
        (
            "print(unique([1, 1.0, \"a\", \"a\"]), flatten([[1], 2, []]))",
            "[1, \"a\"] [1, 2]\n",
        ),
        (
            "print(pad_left(\"7\", 3, \"0\"), pad_right(\"참\", 3, \" \"), pad_left(\"long\", 2, \" \"), pad_left(\"ab\", 5, \"xy\"))",
            "007 참   long xyxab\n",
        ),
        (
            "print(lines(\"\"), lines(\"a\"), lines(\"a\\n\"), lines(\"a\\n\\nb\"), lines(\"a\\r\\nb\\r\\n\"), lines(\"\\n\"), lines(\"참치\\n회\\n\"))",
            "[] [\"a\"] [\"a\"] [\"a\", \"\", \"b\"] [\"a\", \"b\"] [\"\"] [\"참치\", \"회\"]\n",
        ),
        (
            "print(is_ok(Ok(1)), is_ok(Err(1)), is_ok(1), is_err(Err(1)), is_err(Err(1, 2)))",
            "true false false true true\n",
        ),
        (
            "print(take(0..5, 2), drop(0..5, 4), first(1..3), last(1..3), reverse(0..3), zip(0..2, [\"a\"]))",
            "[0, 1] [4] 1 2 [2, 1, 0] [(0, \"a\")]\n",
        ),
        (
            "print(min([]), max([]), sum(0..4), min([2.5, 1]), get({a: 1}, \"a\", default: 0), range(2), range(1, 2))",
            "nil nil 6 1 1 0..2 1..2\n",
        ),
        (
            "print(len(pad_left(\"a\", 34_000_000, \"abc\")), pad_left(\"x\", 5, \"ab\"))",
            "34000000 ababx\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", *want), "{src}");
    }
    for (src, want) in [
        ("pad_left(\"a\", 3, \"\")", "E0803"),
        ("pad_left(\"a\", 200000000, \" \")", "E0806"),
        ("lines(1)", "E0403"),
        ("take(1, 2)", "E0506"),
        ("first(\"abc\")", "E0506"),
        ("get({}, 1)", "E0501"),
    ] {
        assert_eq!(exec(src, Opts::default()).code, want, "{src}");
    }
}

// --- Go TestTakeDropCheckTheirCount ---

#[test]
fn take_drop_check_their_count() {
    for (src, want) in [
        ("take([1], \"x\")", "take expects an int count, got 'str"),
        ("drop([1], \"x\")", "drop expects an int count, got 'str"),
        ("take([1], 1.5)", "take expects an int count, got 'float"),
        ("drop(0..3, nil)", "drop expects an int count, got 'nil"),
    ] {
        let e = exec(src, Opts::default()).error.expect(src);
        assert_eq!(e.code, "E0803", "{src}");
        assert!(e.headline.contains(want), "{src}: {}", e.headline);
    }
}

// --- errors inside extras/prelude.fung (full text from the Go binary) ---

#[test]
fn errors_inside_the_fung_prelude_name_its_file() {
    let (out, err, code) = run_booted("print(1)\ntake([1], \"x\")");
    assert_eq!(out, "1\n");
    assert_eq!(
        err,
        "error[E0803]: take expects an int count, got 'str
  --> -e:2:5
  |
2 | take([1], \"x\")
  |     ^
  |
 call path (innermost first):
   extras/prelude.fung:62 _clamp
   extras/prelude.fung:68 take
   -e:2       <entry>
"
    );
    assert_eq!(code, 1);

    let (_, err, _) = run_booted("until(1, x => x + \"a\", x => false)");
    assert_eq!(
        err,
        "error[E0501]: cannot add int and str
  --> -e:1:17
  |
1 | until(1, x => x + \"a\", x => false)
  |                 ^
  |
 call path (innermost first):
   -e:1       <lambda>
   extras/prelude.fung:120 until
   -e:1       <entry>
"
    );
}

/// The user program has several files (ids 0 and 1); the prelude's file
/// comes after them, so its frames and the imported file's frames each
/// name their own file.
#[test]
fn prelude_frames_and_imported_files_keep_their_names() {
    let dir = temp_dir();
    std::fs::write(dir.join("lib.fung"), "fn cut(xs) = take(xs, \"x\") + []\n").unwrap();
    std::fs::write(
        dir.join("main.fung"),
        "import \"./lib\"\nprint(1)\nlib.cut([1])\n",
    )
    .unwrap();
    let d = dir.display();
    let (out, err, code) = run_file(&dir.join("main.fung"));
    assert_eq!(out, "1\n");
    assert_eq!(
        err,
        format!(
            "error[E0803]: take expects an int count, got 'str
  --> {d}/lib.fung:1:18
  |
1 | fn cut(xs) = take(xs, \"x\") + []
  |                  ^
  |
 call path (innermost first):
   extras/prelude.fung:62 _clamp
   extras/prelude.fung:68 take
   {d}/lib.fung:1 cut
   {d}/main.fung:3 <entry>
"
        )
    );
    assert_eq!(code, 1);
}

/// A file with the execute bit that is not a program: Go's `execve`
/// fails with ENOEXEC (no `/bin/sh` fallback, unlike `execvp`).
#[test]
fn files_that_are_not_programs_give_exec_format_error() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir();
    for (name, body) in [
        ("bad", &b"\x00\x01\x02garbage"[..]),
        ("noshe", b"echo hi\n"),
    ] {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        let src = format!("print(exec([\"{}\"]))", p.display());
        let (out, _, _) = run_booted(&src);
        assert_eq!(
            out,
            format!(
                "Err({{kind: 'not_found, message: \"cannot run `{}`: exec format error\"}})\n",
                p.display()
            )
        );
    }
}

/// `fan_in` drains each channel on its own task into one channel, closed
/// when all are drained (expected output from the Go binary).
#[test]
fn fan_in_merges_its_channels() {
    let (out, err, code) = crate::testutil::run(
        "a = channel(2)\nb = channel(2)\neach([1, 2], x => send(a, x))\nclose(a)\nsend(b, 3)\nclose(b)\nprint(fan_in([a, b]) |> map(x => x) |> sort, fan_in([]) |> map(x => x))",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("[1, 2, 3] []\n", "", 0)
    );
}

/// `pmap` spawns its tasks with `_native.conc.spawn_awaited` and awaits
/// them (expected output from the Go binary).
#[test]
fn pmap_runs_its_tasks() {
    let (out, err, code) =
        crate::testutil::run("print(pmap([1, 2, 3], x => x * x, limit: 2), pmap([1], x => 1 / 0))");
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (
            "[1, 4, 9] [Err({kind: 'error, message: \"error[E0502]: division by zero\"})]\n",
            "",
            0
        )
    );
}

/// `sleep` waits for real, so `retry` with a delay works as in Go.
#[test]
fn sleep_waits_and_retry_uses_it() {
    let start = std::time::Instant::now();
    let (out, err, code) = crate::testutil::run(
        "print(sleep(30ms))\nprint(retry(() => Err(1), times: 3, delay: 10ms))",
    );
    assert_eq!((out.as_str(), err.as_str(), code), ("nil\nErr(1)\n", "", 0));
    assert!(start.elapsed() >= std::time::Duration::from_millis(50));
}
