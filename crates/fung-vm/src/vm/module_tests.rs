//! Ports of Go `internal/vm/module_test.go` and the module parts of the
//! fung command: IMPORT, per-module globals, a module body running once,
//! the module value and member access.

use std::rc::Rc;

use crate::testutil::{Opts, on_big_stack, run_file, run_files, setup, temp_dir};
use crate::value::{Module, Value, format};
use crate::vm::Stop;

/// (stdout, error code) of an in-memory program, like Go `runProgram`.
fn run_program(files: &[(&str, &str)], entry: &str) -> (String, String) {
    let o = run_files(files, entry);
    (o.out, o.code)
}

#[test]
fn a_module_body_runs_once_and_keeps_its_globals() {
    // Go rejects a bare `print(...)` at a module's top level (E0306), so
    // the body prints through an assignment.
    let dir = temp_dir();
    std::fs::write(
        dir.join("lib.fung"),
        "_loaded = print(\"loaded\")\nfn twice(x) = x * 2\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.fung"),
        "import \"./lib\"\nimport \"./lib\" as again\nprint(lib.twice(21), again.twice(1), lib == again)\n",
    )
    .unwrap();
    let (out, err, code) = run_file(&dir.join("main.fung"));
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("loaded\n42 2 true\n", "", 0)
    );
}

#[test]
fn modules_keep_their_own_globals() {
    let files = [
        ("/p/a.fung", "fn helper() = \"a\"\nfn run() = helper()"),
        (
            "/p/b.fung",
            "import \"./a\"\nfn helper() = \"b\"\nfn run() = helper() + a.run()",
        ),
        (
            "/p/main.fung",
            "import \"./a\"\nimport \"./b\"\nprint(b.run(), a.run())\nf = b.run\nprint(f())",
        ),
    ];
    let (out, code) = run_program(&files, "/p/main.fung");
    assert_eq!((out.as_str(), code.as_str()), ("ba a\nba\n", ""));
}

#[test]
fn module_top_level_runs_once() {
    let files = [
        ("/p/m.fung", "x = print(\"init\")\ny = 1"),
        ("/p/a.fung", "import \"./m\"\nv = m.y"),
        (
            "/p/main.fung",
            "import \"./m\"\nimport \"./a\"\nprint(m.y, a.v)",
        ),
    ];
    let (out, _) = run_program(&files, "/p/main.fung");
    assert_eq!(out, "init\n1 1\n");
}

#[test]
fn module_value_and_errors() {
    let m = ("/p/m.fung", "fn _hidden() = 1\nfn shown() = 2\nz = 3");
    let main = (
        "/p/main.fung",
        "import \"./m\"\nprint(m, type_of(m), m.shown(), m.z, m == m)\nv = m\nprint(v._hidden)",
    );
    let (out, code) = run_program(&[m, main], "/p/main.fung");
    assert_eq!(
        (out.as_str(), code.as_str()),
        ("<module m> 'module 2 3 true\n", "E0304")
    );
    let main = ("/p/main.fung", "import \"./m\"\nv = m\nprint(v.nope)");
    let (_, code) = run_program(&[m, main], "/p/main.fung");
    assert_eq!(code, "E0303", "dynamic missing member");
    let boom = ("/p/boom.fung", "x = 1 / 0");
    let main = ("/p/main.fung", "import \"./boom\"");
    let (_, code) = run_program(&[m, boom, main], "/p/main.fung");
    assert_eq!(code, "E0502", "error during module init");
}

#[test]
fn imported_names_are_not_re_exported() {
    let files = [
        ("/p/a.fung", "x = 1"),
        ("/p/b.fung", "import \"./a\" (x)\ny = x + 1"),
        (
            "/p/main.fung",
            "import \"./b\"\nm = b\nprint(m.y)\nprint(m.x)",
        ),
    ];
    let (out, code) = run_program(&files, "/p/main.fung");
    assert_eq!((out.as_str(), code.as_str()), ("2\n", "E0303"));
}

#[test]
fn member_import_and_closure_globals() {
    let files = [
        ("/p/m.fung", "fn base() = 10\nfn add(x) = x + base()"),
        (
            "/p/main.fung",
            "import \"./m\" (add)\nfn base() = 1\nprint(add(5), map([1, 2], add), base())",
        ),
    ];
    let (out, code) = run_program(&files, "/p/main.fung");
    assert_eq!((out.as_str(), code.as_str()), ("15 [11, 12] 1\n", ""));
}

// Full reports, expected text from the Go binary (`fung run <abs path>`
// from the repository root; the temp dir lies outside it, so every file
// shows as its absolute path).

#[test]
fn an_error_inside_an_imported_module_shows_its_file_in_the_call_path() {
    let dir = temp_dir();
    std::fs::write(
        dir.join("boom.fung"),
        "fn helper(x) = x / 0\nfn run(x) = helper(x) + 1\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.fung"),
        "import \"./boom\"\nfn go(n) = boom.run(n) * 2\nprint(go(1))\n",
    )
    .unwrap();
    let d = dir.display();
    let (out, err, code) = run_file(&dir.join("main.fung"));
    assert_eq!(out, "");
    assert_eq!(code, 1);
    assert_eq!(
        err,
        format!(
            "error[E0502]: division by zero
  --> {d}/boom.fung:1:18
  |
1 | fn helper(x) = x / 0
  |                  ^
  |
 call path (innermost first):
   {d}/boom.fung:1 helper
   {d}/boom.fung:2 run
   {d}/main.fung:2 go
   {d}/main.fung:3 <entry>
"
        )
    );
}

#[test]
fn an_error_in_a_module_body_shows_the_module_frame() {
    let dir = temp_dir();
    std::fs::write(dir.join("init.fung"), "x = 1\ny = x / 0\n").unwrap();
    std::fs::write(dir.join("main.fung"), "import \"./init\"\nprint(init.x)\n").unwrap();
    let d = dir.display();
    let (out, err, code) = run_file(&dir.join("main.fung"));
    assert_eq!((out.as_str(), code), ("", 1));
    assert_eq!(
        err,
        format!(
            "error[E0502]: division by zero
  --> {d}/init.fung:2:7
  |
2 | y = x / 0
  |       ^
  |
 call path (innermost first):
   {d}/init.fung:2 <module init>
   {d}/main.fung:1 <entry>
"
        )
    );
}

#[test]
fn member_errors_match_go() {
    let dir = temp_dir();
    std::fs::write(dir.join("init.fung"), "x = 1\nfn _h() = 1\n").unwrap();
    let d = dir.display();
    let cases = [
        (
            "import \"./init\"\nm = init\nprint(m.nope)\n",
            format!(
                "error[E0303]: module `init` has no member `nope`
  --> {d}/main.fung:3:8
  |
3 | print(m.nope)
  |        ^
  |
 call path (innermost first):
   {d}/main.fung:3 <entry>
"
            ),
        ),
        (
            "import \"./init\"\nm = init\nprint(m._h)\n",
            format!(
                "error[E0304]: `_h` is private to module `init`
  --> {d}/main.fung:3:8
  |
3 | print(m._h)
  |        ^
  |
 call path (innermost first):
   {d}/main.fung:3 <entry>
 help: names starting with `_` are not exported
"
            ),
        ),
        (
            "import \"./init\"\nm = init\nprint(m[1])\n",
            format!(
                "error[E0501]: cannot index module and int
  --> {d}/main.fung:3:8
  |
3 | print(m[1])
  |        ^
  |
 call path (innermost first):
   {d}/main.fung:3 <entry>
"
            ),
        ),
        (
            "import \"./init\"\nm = init\nprint(m[\"x\"], m, m < m)\n",
            format!(
                "error[E0501]: cannot compare module and module
  --> {d}/main.fung:3:20
  |
3 | print(m[\"x\"], m, m < m)
  |                    ^
  |
 call path (innermost first):
   {d}/main.fung:3 <entry>
"
            ),
        ),
    ];
    for (src, want) in cases {
        std::fs::write(dir.join("main.fung"), src).unwrap();
        let (out, err, code) = run_file(&dir.join("main.fung"));
        assert_eq!((out.as_str(), code), ("", 1), "{src:?}");
        assert_eq!(err, want, "{src:?}");
    }
}

// The runtime's module table.

/// A fake native module: what `_native.*` bridges register (Task 9).
fn fake_native() -> Rc<Module> {
    let mut m = Module::new("fake", "_native.fake");
    m.set("answer", Value::Int(42));
    m.set("_secret", Value::Int(0));
    Rc::new(m)
}

#[test]
fn a_registered_native_module_is_what_import_gives() {
    // User code cannot import `_native.*` (E0310), so the native module
    // takes the place of a file module's key.
    on_big_stack(|| {
        let s = setup(
            "import \"./lib\"\nprint(lib.answer)",
            Opts {
                file: "/p/main.fung",
                read: |_| Ok("answer = 1".to_string()),
                ..Opts::default()
            },
        )
        .unwrap_or_else(|(e, _)| panic!("{e}"));
        s.rt.register_native("/p/lib.fung", fake_native());
        s.rt.run().unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(s.out.text(), "42\n");
    });
}

#[test]
fn import_module_runs_the_body_once_and_register_does_not_reset_it() {
    on_big_stack(|| {
        let s = setup(
            "import \"./lib\"\nprint(lib.x)",
            Opts {
                file: "/p/main.fung",
                read: |_| Ok("x = print(\"body\")\ny = 2".to_string()),
                ..Opts::default()
            },
        )
        .unwrap_or_else(|(e, _)| panic!("{e}"));
        let m =
            s.rt.import_module("/p/lib.fung")
                .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(m.name, "lib");
        assert_eq!(m.key, "/p/lib.fung");
        assert_eq!(m.names(), ["x", "y"]);
        assert_eq!(format(&Value::Module(m.clone())), "<module lib>");
        // A module that already ran keeps its value (Go `Register`).
        let again = s.rt.import_module("/p/lib.fung").unwrap();
        assert!(Rc::ptr_eq(&m, &again));
        s.rt.register_main_as("/p/lib.fung");
        s.rt.run().unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(s.out.text(), "body\nnil\n");
    });
}

#[test]
fn unregistered_and_self_importing_modules_are_internal_errors() {
    on_big_stack(|| {
        let s = setup(
            "import \"./lib\"\nprint(lib.x)",
            Opts {
                file: "/p/main.fung",
                read: |_| Ok("x = 1".to_string()),
                ..Opts::default()
            },
        )
        .unwrap_or_else(|(e, _)| panic!("{e}"));
        match s.rt.import_module("nope") {
            Err(Stop::Error(d)) => assert_eq!(
                fung_front::diag::headline(&d),
                "error[E0801]: internal error: module `nope` was not registered"
            ),
            other => panic!("{other:?}"),
        }
        // A module whose body imports itself at run time: the entry
        // (which imports ./lib) registered as ./lib.
        s.rt.register_main_as("/p/lib.fung");
        match s.rt.import_module("/p/lib.fung") {
            Err(Stop::Error(d)) => assert_eq!(
                fung_front::diag::headline(&d),
                "error[E0801]: internal error: module `/p/lib.fung` imports itself at run time"
            ),
            other => panic!("{other:?}"),
        }
    });
}

/// Stage 2 does not run the native bridge modules: importing one, directly
/// or through a user module, is E0805 with the name the program wrote, at
/// that import. (Go runs them; this is the Rust port's own output.)
#[test]
fn out_of_scope_standard_modules_are_not_supported_yet() {
    for name in ["json", "regex", "http"] {
        let main = format!("import {name}\nprint(1)");
        let o = run_files(&[("/p/main.fung", main.as_str())], "/p/main.fung");
        assert_eq!((o.out.as_str(), o.code.as_str()), ("", "E0805"), "{name}");
        assert!(
            o.err.starts_with(&format!(
                "error[E0805]: not supported yet: {name}\n  --> /p/main.fung:1:1\n"
            )),
            "{name}: {}",
            o.err
        );
    }
    let files = [
        ("/p/lib.fung", "import regex\nfn f() = 1"),
        ("/p/main.fung", "import \"./lib\"\nprint(lib.f())"),
    ];
    let o = run_files(&files, "/p/main.fung");
    assert_eq!((o.out.as_str(), o.code.as_str()), ("", "E0805"));
    assert!(
        o.err
            .starts_with("error[E0805]: not supported yet: regex\n  --> /p/lib.fung:1:1\n"),
        "{}",
        o.err
    );
    // `cli` is pure fung and runs.
    let o = run_files(
        &[("/p/main.fung", "import cli\nprint(cli.parse([\"a\"]).rest)")],
        "/p/main.fung",
    );
    assert_eq!((o.out.as_str(), o.code.as_str()), ("[\"a\"]\n", ""));
    // `time` runs since stage 4b.
    let o = run_files(
        &[(
            "/p/main.fung",
            "import time\nprint(time.unix(time.from_unix(5)))",
        )],
        "/p/main.fung",
    );
    assert_eq!((o.out.as_str(), o.code.as_str()), ("5\n", ""));
}
