//! Boot tests: the process functions, the prelude functions written in
//! fung, the count checks of `take` and `drop`, the concurrency natives,
//! the parameter tables against `spec/builtins.txt`, and the positions of
//! errors raised inside `extras/prelude.fung` (the whole expected text).

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
    // error (`spawn(1, …)` → E0403 at 2:6), `deref` among them.
    for (src, want) in [
        (
            "print(1)\nspawn(1, () => 2)",
            "error[E0403]: argument `s` must be scope, got int\n  --> -e:2:6\n",
        ),
        (
            "print(1)\nderef(1)",
            "error[E0403]: argument `a` must be atom, got int\n  --> -e:2:6\n",
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
fn atoms_run() {
    // Concurrency v2 L5: atoms (prelude natives) replace the agents; no
    // concurrency builtin gives E0805.
    let (out, err, code) = run_booted("a = atom(1)\nprint(swap(a, x => x + 1), deref(a), a)");
    assert_eq!((out.as_str(), err.as_str(), code), ("2 2 <atom>\n", "", 0));
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
/// call (the extras frames are builtin), with the extras
/// frames in the call path. (`pmap` and `fan_in` run since stage 3a: see
/// `pmap_runs_its_tasks` and `fan_in_merges_its_channels`; since stage 6a
/// they live in the module `conc`, `extras/conc.fung`.)
#[test]
fn fung_helpers_over_concurrency_fail_at_the_user_call() {
    let cases = [
        // the lambda's call is a tail call: the frame it left behind
        // keeps the place, at `fan_in(` (L8)
        ("import conc\nscope(s => conc.fan_in(s, 1))", 2, 23),
        (
            "import conc\nprint(1)\nscope(s => conc.fan_in(s, 1))",
            3,
            23,
        ),
    ];
    for (src, line, col) in cases {
        let o = exec(src, Opts::default());
        let e = o.error.unwrap_or_else(|| panic!("{src}: no error"));
        assert_eq!(
            (e.headline.as_str(), e.line, e.col),
            ("error[E0506]: cannot take the length of int", line, col),
            "{src}"
        );
        assert!(
            e.path.iter().any(|c| c.file == "extras/conc.fung"),
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
            // The spec's names.
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
        });
        assert_eq!(rt.args(), ["a", "b c"]);
    });
}

#[test]
fn native_modules_match_the_spec() {
    on_big_stack(|| {
        let spec = builtins();
        let keys: Vec<&str> = super::NATIVE_MODULES.iter().map(|(k, _)| *k).collect();
        let spec_keys: Vec<&str> = spec
            .modules
            .keys()
            .map(String::as_str)
            .filter(|k| k.starts_with("_native."))
            .collect();
        assert_eq!(keys, spec_keys);
        for (key, module) in super::NATIVE_MODULES {
            let (key, m) = (*key, module());
            assert_eq!(m.key, key);
            assert_eq!(
                super::native_module(key).map(|m| m.key.clone()),
                Some(m.key.clone())
            );
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

/// Stage 6a: `fan_in`, `pmap` and `retry` are the module `conc`
/// (`extras/conc.fung`), with the parameter tables of
/// `fung_front::builtins`; the prelude no longer has them, so a program
/// that calls one without the module gets the error any unknown name
/// gets (the text for an unknown `qmap`, the name swapped).
#[test]
fn conc_module_matches_the_spec_and_the_prelude_lacks_it() {
    on_big_stack(|| {
        let spec = builtins();
        let s = crate::testutil::setup("import conc\nprint(1)", Opts::default())
            .unwrap_or_else(|(e, _)| panic!("{e}"));
        let m = s.rt.import_module("conc").expect("conc");
        let mut names: Vec<&str> = m.names();
        names.sort();
        assert_eq!(names, ["fan_in", "pmap", "retry"]);
        for name in names {
            let Some(Value::Closure(c)) = m.get(name) else {
                panic!("conc.{name} is not a fung function")
            };
            let got = c
                .proto
                .params
                .iter()
                .map(|p| {
                    if p.has_default {
                        format!("{}=?", p.name)
                    } else {
                        p.name.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            assert_eq!(
                got,
                spec.module_signature("conc", name).expect(name),
                "conc.{name}"
            );
        }
    });
    let (out, err, code) = run_booted("print(1)\npmap([1], x => x)");
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (
            "",
            "error[E0204]: undefined name `pmap`\n  --> -e:2:1\n  |\n2 | pmap([1], x => x)\n  | ^^^^ not found in this scope\n  |\n help: moved to module conc: import conc, then conc.pmap\n",
            2
        )
    );
    for name in ["fan_in", "retry"] {
        let (_, err, _) = run_booted(&format!("{name}(1)"));
        assert!(
            err.starts_with(&format!("error[E0204]: undefined name `{name}`")),
            "{err}"
        );
    }
}

/// The `http` module (stage 4d, `extras/http.fung`) is written in
/// fung: its public functions are the spec's, with the spec's parameter
/// tables.
#[test]
fn http_module_matches_the_spec() {
    on_big_stack(|| {
        let spec = builtins();
        let s = crate::testutil::setup("import http\nprint(1)", Opts::default())
            .unwrap_or_else(|(e, _)| panic!("{e}"));
        let m = s.rt.import_module("http").expect("http");
        let mut names: Vec<&str> = m.names();
        names.sort();
        let want: Vec<&str> = spec.modules["http"].iter().map(String::as_str).collect();
        assert_eq!(names, want);
        for name in names {
            let Some(Value::Closure(c)) = m.get(name) else {
                panic!("http.{name} is not a fung function")
            };
            let got = c
                .proto
                .params
                .iter()
                .map(|p| {
                    if p.rest {
                        format!("...{}", p.name)
                    } else if p.has_default {
                        format!("{}=?", p.name)
                    } else {
                        p.name.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            assert_eq!(
                got,
                spec.module_signature("http", name).expect(name),
                "http.{name}"
            );
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

// --- process functions ---

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

/// Failures to start a command and their reasons (06: 명령 실행).
#[test]
fn commands_that_cannot_run_are_not_found() {
    let cases: &[(&str, &str)] = &[
        (
            "print(exec([\"definitely-not-a-command-xyz\"]))",
            "Err({kind: 'not_found, message: \"cannot run `definitely-not-a-command-xyz`: command not found in PATH\"})\n",
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
            "Err({kind: 'not_found, message: \"cannot run ``: empty command name\"})\n",
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
            "Err({kind: 'not_found, message: \"cannot run `sh`: argument contains a NUL byte\"})\n",
        ),
        (
            "print(exec([\"sh\"], env: {\"A\": \"x\\u{0}\"}))",
            "Err({kind: 'not_found, message: \"cannot run `sh`: environment variable contains a NUL byte\"})\n",
        ),
        (
            "print(exec([\"ls\"], cwd: \"/\\u{0}\"))",
            "Err({kind: 'not_found, message: \"cannot run `ls`: working directory contains a NUL byte\"})\n",
        ),
        (
            "print(exec([\"ls\"], cwd: \"no\\u{0}\", env: {\"A\": \"\\u{0}\"}))",
            "Err({kind: 'not_found, message: \"cannot run `ls`: working directory contains a NUL byte\"})\n",
        ),
    ];
    for (src, want) in cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", *want), "{src}");
    }
}

// --- prelude functions written in fung ---

#[test]
fn fung_prelude_functions() {
    let cases: &[(&str, &str)] = &[
        (
            "(tx, ch) = channel(3)\nsend(tx, 1)\nsend(tx, 2)\nclose(tx)\nprint(first(ch))",
            "1\n",
        ),
        (
            "(tx, ch) = channel(3)\nsend(tx, 1)\nsend(tx, 2)\nclose(tx)\nprint(last(ch))",
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

/// A range wider than the int range is not empty, and `first`, `last`,
/// `take` and `drop` work on it without counting it: only `len` fails
/// (02). A result that would hold too many numbers is E0806.
#[test]
fn wide_ranges_need_no_length() {
    let w = "-9223372036854775808..=9223372036854775807";
    let cases = [
        (
            format!("print(is_empty({w}), first({w}), last({w}))"),
            "false -9223372036854775808 9223372036854775807\n",
        ),
        (
            format!(
                "print(take({w}, 2), take({w}, -1), drop(0..=9223372036854775807, 9223372036854775807))"
            ),
            "[-9223372036854775808, -9223372036854775807] [] [9223372036854775807]\n",
        ),
        (
            "print(is_empty(3..3), last(3..3), last(3..=3), last(-2..0), take(5..8, 10), drop(5..8, 1), drop(5..8, 9), drop(5..8, -1))"
                .to_string(),
            "true nil 3 -1 [5, 6, 7] [6, 7] [] [5, 6, 7]\n",
        ),
    ];
    for (src, want) in &cases {
        let o = exec(src, Opts::default());
        assert_eq!((o.code.as_str(), o.out.as_str()), ("", *want), "{src}");
    }
    for src in [format!("drop({w}, 2)"), format!("take({w}, 200000000)")] {
        assert_eq!(exec(&src, Opts::default()).code, "E0806", "{src}");
    }
}

// --- take and drop check their count ---

/// A count that is not an int is an argument error, as for a native.
#[test]
fn take_drop_check_their_count() {
    for (src, want) in [
        ("take([1], \"x\")", "argument `n` must be int, got str"),
        ("drop([1], \"x\")", "argument `n` must be int, got str"),
        ("take([1], 1.5)", "argument `n` must be int, got float"),
        ("drop(0..3, nil)", "argument `n` must be int, got nil"),
        ("take(\"ab\", nil)", "argument `n` must be int, got nil"),
    ] {
        let e = exec(src, Opts::default()).error.expect(src);
        assert_eq!(e.code, "E0403", "{src}");
        assert!(e.headline.contains(want), "{src}: {}", e.headline);
    }
}

/// The prelude functions written in fung name themselves when given
/// something they cannot go over (F-4): not the `map` they use inside.
#[test]
fn prelude_collection_errors_name_the_function() {
    for (src, want) in [
        ("take(5, 2)", "cannot take from int"),
        ("drop(nil, 1)", "cannot drop from nil"),
        ("first({})", "cannot take the first of map"),
        ("last(5)", "cannot take the last of int"),
        ("min(5)", "cannot take the min of int"),
        ("max((1, 2))", "cannot take the max of tuple"),
        ("reverse(5)", "cannot reverse int"),
        ("zip(5, [1])", "cannot zip int"),
        ("zip([1], true)", "cannot zip bool"),
    ] {
        let e = exec(src, Opts::default()).error.expect(src);
        assert_eq!(e.code, "E0506", "{src}");
        assert!(e.headline.ends_with(want), "{src}: {}", e.headline);
    }
}

// --- errors inside extras/prelude.fung (the whole text) ---

#[test]
fn errors_inside_the_fung_prelude_name_its_file() {
    let (out, err, code) = run_booted("print(1)\ntake([1], \"x\")");
    assert_eq!(out, "1\n");
    assert_eq!(
        err,
        "error[E0403]: argument `n` must be int, got str
  --> -e:2:5
  |
2 | take([1], \"x\")
  |     ^
  |
 call path (innermost first):
   extras/prelude.fung:81 _clamp
   extras/prelude.fung:90 take
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
   extras/prelude.fung:126 until
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
            "error[E0403]: argument `n` must be int, got str
  --> {d}/lib.fung:1:18
  |
1 | fn cut(xs) = take(xs, \"x\") + []
  |                  ^
  |
 call path (innermost first):
   extras/prelude.fung:81 _clamp
   extras/prelude.fung:90 take
   {d}/lib.fung:1 cut
   {d}/main.fung:3 <entry>
"
        )
    );
    assert_eq!(code, 1);
}

/// A file with the execute bit that is not a program: `execve` fails
/// with ENOEXEC (no `/bin/sh` fallback, unlike `execvp`).
#[test]
fn files_that_are_not_programs_are_refused() {
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
                "Err({{kind: 'not_found, message: \"cannot run `{}`: not an executable file (run scripts with sh)\"}})\n",
                p.display()
            )
        );
    }
}

/// `fan_in` drains each channel on its own task into one channel, closed
/// when all are drained.
#[test]
fn fan_in_merges_its_channels() {
    let (out, err, code) = crate::testutil::run(
        "import conc\n(a_tx, a) = channel(2)\n(b_tx, b) = channel(2)\neach([1, 2], x => send(a_tx, x))\nclose(a_tx)\nsend(b_tx, 3)\nclose(b_tx)\nprint(scope(s => conc.fan_in(s, [a, b]) |> map(x => x) |> sort), scope(s => conc.fan_in(s, []) |> map(x => x)))",
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("[1, 2, 3] []\n", "", 0)
    );
}

/// `pmap` runs its tasks in a scope of its own: a runtime error in `f`
/// fails `pmap`.
#[test]
fn pmap_runs_its_tasks() {
    let (out, err, code) = crate::testutil::run(
        "import conc\nprint(conc.pmap([1, 2, 3], x => x * x, limit: 2))\nconc.pmap([1], x => 1 / 0)",
    );
    assert_eq!(out, "[1, 4, 9]\n");
    assert!(err.contains("error[E0502]: division by zero"), "{err}");
    assert_eq!(code, 1);
}

/// `sleep` waits for real, so `retry` with a delay really waits.
#[test]
fn sleep_waits_and_retry_uses_it() {
    let start = std::time::Instant::now();
    let (out, err, code) = crate::testutil::run(
        "import conc\nprint(sleep(30ms))\nprint(conc.retry(() => Err(1), times: 3, delay: 10ms))",
    );
    assert_eq!((out.as_str(), err.as_str(), code), ("nil\nErr(1)\n", "", 0));
    assert!(start.elapsed() >= std::time::Duration::from_millis(50));
}
