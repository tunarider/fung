//! `_native.fs` tests: Go `TestFsReadWriteListStat` (the functions this
//! file has), the exact failure messages, the errno table, and whole runs
//! compared with the Go binary in a scratch directory.

use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use fung_front::source::SourceMap;

use super::module;
use crate::native::oserr;
use crate::testutil::{self, Buf, Opts, exec};
use crate::value::{NativeParam, Value, format};
use crate::vm::{Runtime, Stop};

/// Calls `_native.fs.<name>` with bound arguments, as Go's tests call
/// `n.Fn(ctx, args)`.
fn call(name: &str, args: Vec<Value>) -> Result<Value, Stop> {
    let m = module();
    let Some(Value::Native(n)) = m.get(name) else {
        panic!("no _native.fs.{name}");
    };
    let rt = Runtime::new(
        Box::new(Buf::default()),
        Box::new(Buf::default()),
        Box::new(Cursor::new(Vec::new())),
        SourceMap::default(),
    );
    let r = (n.func)(&mut rt.task(), args);
    drop(rt);
    r
}

/// The result as Go's `value.Format` shows it, or the error's code.
fn show(r: Result<Value, Stop>) -> String {
    match r {
        Ok(v) => format(&v),
        Err(Stop::Error(d)) => d.code.clone(),
        Err(Stop::Exit(n)) => format!("exit {n}"),
    }
}

fn s(x: &str) -> Value {
    Value::str(x)
}

fn p(x: &Path) -> Value {
    Value::str(x.to_str().expect("a UTF-8 test path"))
}

/// A scratch directory removed (permissions restored first) on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("fung-fs-{tag}-{}", std::process::id()));
        let _ = restore_and_remove(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn at(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    fn text(&self) -> &str {
        self.0.to_str().expect("a UTF-8 test path")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = restore_and_remove(&self.0);
    }
}

fn restore_and_remove(dir: &Path) -> std::io::Result<()> {
    fn open_up(p: &Path) {
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    open_up(&e.path());
                } else {
                    let _ =
                        std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(0o644));
                }
            }
        }
    }
    open_up(dir);
    std::fs::remove_dir_all(dir)
}

/// The fixture the failure cases run in: `a.txt`, `bad.txt` (invalid
/// UTF-8), `sub/deep/f`, an empty `emptyd`, an unreadable `locked` and a
/// read-only directory `ro`.
fn fixture(dir: &Path) {
    std::fs::create_dir_all(dir.join("sub/deep")).unwrap();
    std::fs::create_dir(dir.join("emptyd")).unwrap();
    std::fs::write(dir.join("a.txt"), "hi\n").unwrap();
    std::fs::write(dir.join("sub/deep/f"), "x\n").unwrap();
    std::fs::write(dir.join("bad.txt"), b"a\xffb\xc3").unwrap();
    std::fs::write(dir.join("locked"), "secret").unwrap();
    std::fs::set_permissions(dir.join("locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
    std::fs::create_dir(dir.join("ro")).unwrap();
    std::fs::set_permissions(dir.join("ro"), std::fs::Permissions::from_mode(0o555)).unwrap();
}

/// Root reads files without permission bits: the permission cases then
/// do not fail.
fn running_as_root(locked: &Path) -> bool {
    std::fs::File::open(locked).is_ok()
}

fn render_params(ps: &[NativeParam]) -> String {
    ps.iter()
        .map(|p| {
            if p.default.is_some() {
                format!("{}=?", p.name)
            } else {
                p.name.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn params_match_the_spec() {
    let spec = fung_front::builtins::builtins();
    let m = module();
    assert_eq!(m.key, "_native.fs");
    let names = m.names();
    for name in [
        "read",
        "write",
        "append",
        "exists",
        "stat",
        "mkdir",
        "remove",
        "remove_all",
        "move",
        "copy",
        "temp_dir",
    ] {
        assert!(names.contains(&name), "{name}");
    }
    for name in names {
        let Some(Value::Native(n)) = m.get(name) else {
            panic!("_native.fs.{name} is not a native")
        };
        assert_eq!(
            render_params(&n.params),
            spec.module_signature("_native.fs", name).expect(name),
            "{name}"
        );
    }
}

/// Go `TestFsReadWriteListStat`, without `list`, `walk` and `glob`.
#[test]
fn read_write_stat_copy_move_remove() {
    let d = Scratch::new("rw");
    let a = d.at("a.txt");
    assert_eq!(show(call("write", vec![p(&a), s("hi\n")])), "Ok(nil)");
    assert_eq!(show(call("append", vec![p(&a), s("there\n")])), "Ok(nil)");
    assert_eq!(show(call("read", vec![p(&a)])), r#"Ok("hi\nthere\n")"#);
    let none = d.at("none");
    assert_eq!(
        show(call("read", vec![p(&none)])),
        format!(
            "Err({{kind: 'not_found, message: \"cannot read `{}`: no such file or directory\"}})",
            none.display()
        )
    );
    assert_eq!(show(call("mkdir", vec![p(&d.at("sub/deep"))])), "Ok(nil)");
    assert_eq!(
        show(call("write", vec![p(&d.at("sub/deep/b.txt")), s("")])),
        "Ok(nil)"
    );
    assert_eq!(
        show(call("stat", vec![p(&a)])),
        r#"Ok({name: "a.txt", size: 9, is_dir: false})"#
    );
    assert_eq!(show(call("exists", vec![p(&a)])), "true");
    assert_eq!(
        show(call("copy", vec![p(&a), p(&d.at("c.txt"))])),
        "Ok(nil)"
    );
    assert_eq!(
        show(call("move", vec![p(&d.at("c.txt")), p(&d.at("d.txt"))])),
        "Ok(nil)"
    );
    assert_eq!(
        show(call("read", vec![p(&d.at("d.txt"))])),
        r#"Ok("hi\nthere\n")"#
    );
    assert_eq!(
        show(call("remove", vec![p(&d.at("sub"))])),
        format!(
            "Err({{kind: 'exists, message: \"cannot remove `{}`: directory not empty\"}})",
            d.at("sub").display()
        )
    );
    assert_eq!(show(call("remove_all", vec![p(&d.at("sub"))])), "Ok(nil)");
    assert_eq!(show(call("exists", vec![p(&d.at("sub"))])), "false");
    let r = call("temp_dir", vec![]).unwrap();
    let text = format(&r);
    assert!(text.starts_with("Ok(\""), "{text}");
    let Value::Tag(t) = &r else { unreachable!() };
    assert_eq!(show(call("remove_all", vec![t.args[0].clone()])), "Ok(nil)");
    assert_eq!(show(call("read", vec![Value::Int(1)])), "E0403");
}

#[test]
fn failures_read_like_go() {
    let d = Scratch::new("fail");
    fixture(&d.0);
    let at = |rel: &str| d.at(rel).to_str().unwrap().to_string();
    let err = |kind: &str, msg: String| format!("Err({{kind: '{kind}, message: \"{msg}\"}})");
    let cases: Vec<(&str, Vec<Value>, String)> = vec![
        (
            "read",
            vec![s("/nonexistent-fung-test/x")],
            err(
                "not_found",
                "cannot read `/nonexistent-fung-test/x`: no such file or directory".into(),
            ),
        ),
        (
            "read",
            vec![s(d.text())],
            err("io", format!("cannot read `{}`: is a directory", d.text())),
        ),
        (
            "read",
            vec![s(&at("bad.txt"))],
            "Ok(\"a\u{FFFD}b\u{FFFD}\")".into(),
        ),
        (
            "write",
            vec![s(&at("nope/x")), s("x")],
            err(
                "not_found",
                format!("cannot write `{}`: no such file or directory", at("nope/x")),
            ),
        ),
        (
            "write",
            vec![s(d.text()), s("x")],
            err("io", format!("cannot write `{}`: is a directory", d.text())),
        ),
        (
            "append",
            vec![s(d.text()), s("x")],
            err(
                "io",
                format!("cannot append to `{}`: is a directory", d.text()),
            ),
        ),
        ("mkdir", vec![s(&at("sub"))], "Ok(nil)".into()),
        (
            "mkdir",
            vec![s(&at("a.txt"))],
            err(
                "io",
                format!("cannot create directory `{}`: not a directory", at("a.txt")),
            ),
        ),
        (
            "mkdir",
            vec![s(&at("a.txt/b"))],
            err(
                "io",
                format!(
                    "cannot create directory `{}`: not a directory",
                    at("a.txt/b")
                ),
            ),
        ),
        (
            "remove",
            vec![s(&at("none"))],
            err(
                "not_found",
                format!("cannot remove `{}`: no such file or directory", at("none")),
            ),
        ),
        (
            "remove",
            vec![s(&at("a.txt/x"))],
            err(
                "io",
                format!("cannot remove `{}`: not a directory", at("a.txt/x")),
            ),
        ),
        ("remove_all", vec![s(&at("none"))], "Ok(nil)".into()),
        ("remove_all", vec![s("")], "Ok(nil)".into()),
        (
            "remove_all",
            vec![s(&at("."))],
            err(
                "io",
                format!("cannot remove `{}`: invalid argument", at(".")),
            ),
        ),
        (
            "copy",
            vec![s(&at("none")), s(&at("c"))],
            err(
                "not_found",
                format!("cannot copy `{}`: no such file or directory", at("none")),
            ),
        ),
        (
            "copy",
            vec![s(&at("a.txt")), s(&at("nope/c"))],
            err(
                "not_found",
                format!(
                    "cannot copy to `{}`: no such file or directory",
                    at("nope/c")
                ),
            ),
        ),
        (
            "copy",
            vec![s(d.text()), s(&at("c"))],
            err("io", format!("cannot copy `{}`: is a directory", d.text())),
        ),
        (
            "move",
            vec![s(&at("none")), s(&at("c"))],
            err(
                "not_found",
                format!("cannot move `{}`: no such file or directory", at("none")),
            ),
        ),
        (
            "move",
            vec![s(&at("a.txt")), s(&at("sub"))],
            err(
                "exists",
                format!("cannot move `{}`: file exists", at("a.txt")),
            ),
        ),
        (
            "move",
            vec![s(&at("sub")), s(&at("a.txt"))],
            err(
                "io",
                format!("cannot move `{}`: not a directory", at("sub")),
            ),
        ),
        (
            "move",
            vec![s(&at("sub")), s(&at("emptyd"))],
            err(
                "exists",
                format!("cannot move `{}`: file exists", at("sub")),
            ),
        ),
        (
            "stat",
            vec![s(&at("sub/"))],
            format!(
                "Ok({{name: \"sub\", size: {}, is_dir: true}})",
                std::fs::metadata(at("sub")).unwrap().len()
            ),
        ),
        (
            "stat",
            vec![s("")],
            err(
                "not_found",
                "cannot stat ``: no such file or directory".into(),
            ),
        ),
        (
            "stat",
            vec![s(&at("a.txt/x"))],
            err(
                "io",
                format!("cannot stat `{}`: not a directory", at("a.txt/x")),
            ),
        ),
        (
            "read",
            vec![s("a\0b")],
            err("io", "cannot read `a\\u{0}b`: invalid argument".into()),
        ),
        ("exists", vec![s("")], "false".into()),
        ("exists", vec![s("a\0b")], "false".into()),
    ];
    for (name, args, want) in cases {
        let shown = format!(
            "{name}({})",
            args.iter().map(format).collect::<Vec<_>>().join(", ")
        );
        assert_eq!(show(call(name, args)), want, "{shown}");
    }
}

#[test]
fn permission_failures_read_like_go() {
    let d = Scratch::new("perm");
    fixture(&d.0);
    if running_as_root(&d.at("locked")) {
        eprintln!("skipping the permission cases: running as root");
        return;
    }
    let at = |rel: &str| d.at(rel).to_str().unwrap().to_string();
    let denied = |verb: &str, path: String| {
        format!(
            "Err({{kind: 'permission, message: \"cannot {verb} `{path}`: permission denied\"}})"
        )
    };
    assert_eq!(
        show(call("read", vec![s(&at("locked"))])),
        denied("read", at("locked"))
    );
    assert_eq!(
        show(call("write", vec![s(&at("locked")), s("x")])),
        denied("write", at("locked"))
    );
    assert_eq!(
        show(call("write", vec![s(&at("ro/x")), s("x")])),
        denied("write", at("ro/x"))
    );
    assert_eq!(
        show(call("append", vec![s(&at("locked")), s("x")])),
        denied("append to", at("locked"))
    );
    assert_eq!(
        show(call("mkdir", vec![s(&at("ro/x/y"))])),
        denied("create directory", at("ro/x/y"))
    );
    assert_eq!(
        show(call("copy", vec![s(&at("locked")), s(&at("c"))])),
        denied("copy", at("locked"))
    );
}

#[test]
fn copy_keeps_the_permission_bits() {
    let d = Scratch::new("mode");
    std::fs::write(d.at("x"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(d.at("x"), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        show(call("copy", vec![p(&d.at("x")), p(&d.at("y"))])),
        "Ok(nil)"
    );
    let mode = std::fs::metadata(d.at("y")).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
}

/// Go `os.MkdirTemp("", "fung-")`: `$TMPDIR` (or `/tmp`) joined with
/// `fung-` and a decimal number, made with mode 0700.
#[test]
fn temp_dir_is_made_under_tmpdir() {
    let base = match std::env::var("TMPDIR") {
        Ok(t) if !t.is_empty() => t,
        _ => "/tmp".to_string(),
    };
    let prefix = if base.ends_with('/') {
        format!("{base}fung-")
    } else {
        format!("{base}/fung-")
    };
    let mut seen = Vec::new();
    for _ in 0..2 {
        let Ok(Value::Tag(t)) = call("temp_dir", vec![]) else {
            panic!("temp_dir failed")
        };
        assert_eq!(t.name.name(), "Ok");
        let Value::Str(dir) = &t.args[0] else {
            panic!("not a string")
        };
        let rest = dir.strip_prefix(&prefix).unwrap_or_else(|| panic!("{dir}"));
        assert!(
            !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()),
            "{dir}"
        );
        let meta = std::fs::metadata(dir.as_str()).unwrap();
        assert!(meta.is_dir());
        assert_eq!(meta.permissions().mode() & 0o777, 0o700);
        seen.push(dir.to_string());
    }
    assert_ne!(seen[0], seen[1]);
    for dir in seen {
        std::fs::remove_dir(dir).unwrap();
    }
}

#[test]
fn wrong_argument_types_are_e0403() {
    for (name, args) in [
        ("read", vec![Value::Int(1)]),
        ("write", vec![s("x"), Value::Nil]),
        ("write", vec![Value::Nil, Value::Nil]),
        ("append", vec![s("x"), Value::Int(1)]),
        ("exists", vec![Value::Nil]),
        ("stat", vec![Value::Bool(true)]),
        ("mkdir", vec![Value::Int(1)]),
        ("remove", vec![Value::Int(1)]),
        ("remove_all", vec![Value::Int(1)]),
        ("move", vec![s("x"), Value::Int(1)]),
        ("copy", vec![Value::Int(1), s("x")]),
    ] {
        assert_eq!(show(call(name, args)), "E0403", "{name}");
    }
}

// --- the errno table ---

#[test]
fn reasons_are_gos_errno_texts() {
    use std::io::Error;
    let e = |n| oserr::reason(&Error::from_raw_os_error(n));
    assert_eq!(e(oserr::ENOENT), "no such file or directory");
    assert_eq!(e(oserr::EACCES), "permission denied");
    assert_eq!(e(oserr::EPERM), "operation not permitted");
    assert_eq!(e(oserr::EEXIST), "file exists");
    assert_eq!(e(oserr::ENOTDIR), "not a directory");
    assert_eq!(e(21), "is a directory");
    assert_eq!(e(oserr::EINVAL), "invalid argument");
    assert_eq!(e(oserr::ENOTEMPTY), "directory not empty");
    assert_eq!(e(0), "errno 0");
    assert_eq!(e(-1), "errno -1");
    assert_eq!(e(9999), "errno 9999");
    #[cfg(target_os = "macos")]
    {
        assert_eq!(e(16), "resource busy");
        assert_eq!(e(18), "cross-device link");
        assert_eq!(e(63), "file name too long");
        assert_eq!(e(62), "too many levels of symbolic links");
    }
    #[cfg(target_os = "linux")]
    {
        assert_eq!(e(16), "device or resource busy");
        assert_eq!(e(18), "invalid cross-device link");
        assert_eq!(e(36), "file name too long");
        assert_eq!(e(40), "too many levels of symbolic links");
        assert_eq!(e(41), "errno 41");
    }
    // An error the standard library makes itself has no errno.
    let own = Error::other("Something odd");
    assert_eq!(oserr::reason(&own), "something odd");
}

#[test]
fn kinds_follow_gos_error_classes() {
    use std::io::Error;
    let k = |n| oserr::kind(&Error::from_raw_os_error(n));
    assert_eq!(k(oserr::ENOENT), "not_found");
    assert_eq!(k(oserr::EEXIST), "exists");
    assert_eq!(k(oserr::ENOTEMPTY), "exists");
    assert_eq!(k(oserr::EACCES), "permission");
    assert_eq!(k(oserr::EPERM), "permission");
    assert_eq!(k(oserr::ENOTDIR), "io");
    assert_eq!(k(21), "io");
    assert_eq!(oserr::kind(&Error::other("x")), "io");
}

/// Every errno from 0 to 160 reads as `syscall.Errno(n).Error()` of the
/// Go toolchain on this machine.
#[test]
fn errno_table_matches_go() {
    let Some(text) = go_errno_texts() else { return };
    let go: Vec<&str> = text.lines().collect();
    assert_eq!(go.len(), 161, "{text}");
    for (n, want) in go.iter().enumerate() {
        assert_eq!(oserr::errno_text(n as i32), *want, "errno {n}");
    }
}

/// `go run` of a program printing `syscall.Errno(n).Error()` for n in
/// 0..=160, in a scratch directory outside the repository.
fn go_errno_texts() -> Option<String> {
    let dir = std::env::temp_dir().join(format!("fung-errno-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("main.go"),
        "package main\n\nimport (\n\t\"fmt\"\n\t\"syscall\"\n)\n\nfunc main() {\n\tfor n := 0; n <= 160; n++ {\n\t\tfmt.Println(syscall.Errno(n).Error())\n\t}\n}\n",
    )
    .unwrap();
    let out = std::process::Command::new("go")
        .args(["run", "main.go"])
        .current_dir(&dir)
        .env("GO111MODULE", "off")
        .output();
    let _ = std::fs::remove_dir_all(&dir);
    match out {
        Err(e) => {
            assert!(
                std::env::var("FUNG_REQUIRE_PARITY").as_deref() != Ok("1"),
                "FUNG_REQUIRE_PARITY=1 but `go` cannot be run: {e}"
            );
            eprintln!("skipping the errno comparison: `go` cannot be run: {e}");
            None
        }
        Ok(o) if o.status.success() => Some(String::from_utf8(o.stdout).unwrap()),
        Ok(o) => panic!("go run failed:\n{}", String::from_utf8_lossy(&o.stderr)),
    }
}

// --- whole runs against the Go binary ---

/// Programs run by both implementations in a fresh `fixture` directory
/// bound to `d`: Go imports `fs`, Rust `_native.fs` (the `fs` module
/// itself comes with the rest of `_native.fs`).
const PROGRAMS: &[&str] = &[
    r#"print(fs.read("/nonexistent-fung-test/x"))
print(fs.read(d))
print(fs.read("${d}/bad.txt"))
print(fs.read("${d}/locked"))
print(fs.write("${d}/locked", "x"))
print(fs.write("${d}/ro/x", "x"))
print(fs.write("${d}/nope/x", "x"))
print(fs.write(d, "x"))
print(fs.append("${d}/locked", "x"))
print(fs.append(d, "x"))
print(fs.mkdir("${d}/sub"))
print(fs.mkdir("${d}/a.txt"))
print(fs.mkdir("${d}/a.txt/b"))
print(fs.mkdir("${d}/ro/x/y"))
print(fs.remove("${d}/sub"))
print(fs.remove("${d}/none"))
print(fs.remove("${d}/a.txt/x"))
print(fs.remove_all("${d}/none"))
print(fs.remove_all("${d}/."))
print(fs.remove_all(""))
print(fs.remove_all("${d}/a.txt/x"))
print(fs.copy("${d}/none", "${d}/c"))
print(fs.copy("${d}/a.txt", "${d}/nope/c"))
print(fs.copy(d, "${d}/c"))
print(fs.copy("${d}/locked", "${d}/c"))
print(fs.copy("${d}/a.txt", d))
print(fs.move("${d}/none", "${d}/c"))
print(fs.move("${d}/a.txt", "${d}/nope/c"))
print(fs.move("${d}/a.txt", "${d}/sub"))
print(fs.move("${d}/none", "${d}/sub"))
print(fs.move("${d}/sub", "${d}/a.txt"))
print(fs.move("${d}/sub", "${d}/emptyd"))
print(fs.move("${d}/sub", "${d}/sub/deep/x"))
print(fs.stat("${d}/a.txt"))
print(fs.stat("${d}/sub/deep/"))
print(fs.stat("${d}/none"))
print(fs.stat(""))
print(fs.stat("${d}/a.txt/x"))
print(fs.exists(d), fs.exists("${d}/none"), fs.exists(""), fs.exists("${d}/locked"))
print(fs.read("a\u{0}b"), fs.write("a\u{0}b", ""), fs.mkdir("a\u{0}b"), fs.remove("a\u{0}b"), fs.stat("a\u{0}b"), fs.exists("a\u{0}b"))
print(fs.move("a\u{0}b", "c"), fs.copy("${d}/a.txt", "a\u{0}b"), fs.remove_all("a\u{0}b"))"#,
    r#"print(fs.write("${d}/w.txt", "hi\n"), fs.append("${d}/w.txt", "there\n"), fs.append("${d}/new.txt", "x"))
print(fs.read("${d}/w.txt"), fs.read("${d}/new.txt"))
print(fs.write("${d}/w.txt", "short"), fs.read("${d}/w.txt"))
print(fs.mkdir("${d}/m/n/o"), fs.mkdir("${d}/m/n/o/"), fs.mkdir("${d}/m/./p/.."), fs.exists("${d}/m/p"), fs.mkdir(d))
print(fs.copy("${d}/w.txt", "${d}/m/c.txt"), fs.stat("${d}/m/c.txt"))
print(fs.copy("${d}/w.txt", "${d}/m/c.txt"), fs.copy("${d}/w.txt", "${d}/w.txt"))
print(fs.move("${d}/m/c.txt", "${d}/m/n/d.txt"), fs.exists("${d}/m/c.txt"), fs.read("${d}/m/n/d.txt"))
print(fs.move("${d}/m/n/d.txt", "${d}/m/n/d.txt"), fs.move("${d}/m/n", "${d}/m/n"))
print(fs.move("${d}/emptyd", "${d}/moved"), fs.exists("${d}/moved"))
print(fs.remove("${d}/m/n/o"), fs.remove("${d}/m/n/d.txt"), fs.remove_all("${d}/m"), fs.exists("${d}/m"))
print(fs.remove_all("${d}/w.txt"), fs.exists("${d}/w.txt"))
print(fs.remove_all("${d}/sub"), fs.exists("${d}/sub"))
print(fs.stat("${d}/new.txt/"), fs.stat("${d}/new.txt"))
print(fs.write("${d}/u.txt", "한글 ok"), fs.read("${d}/u.txt"), fs.stat("${d}/u.txt"))
print(fs.stat("${d}/./u.txt"), fs.stat("${d}/ro/.."))"#,
    "print(fs.read(1))",
    "print(fs.write(d, nil))",
    "print(fs.copy(nil, 1))",
    "print(fs.stat())",
    "print(fs.exists(d, d))",
    "print(fs.mkdir(path: d))",
];

#[test]
fn runs_match_the_go_binary() {
    let Some(bin) = testutil::go_fung("fs") else {
        return;
    };
    let d = Scratch::new("go");
    let mut bad = Vec::new();
    for body in PROGRAMS {
        let decl = format!("d = \"{}\"\n", d.text());
        let _ = restore_and_remove(&d.0);
        std::fs::create_dir_all(&d.0).unwrap();
        fixture(&d.0);
        let go = testutil::run_go(&bin, &format!("import fs\n{decl}{body}"), "");
        let _ = restore_and_remove(&d.0);
        std::fs::create_dir_all(&d.0).unwrap();
        fixture(&d.0);
        let src = format!("import _native.fs as fs\n{decl}{body}");
        let o = exec(
            &src,
            Opts {
                internal: true,
                ..Opts::default()
            },
        );
        let rust = (o.out, o.err, o.exit);
        if rust != go {
            bad.push(format!("{body}\n  rust {rust:?}\n  go   {go:?}"));
        }
    }
    let _ = std::fs::remove_file(&bin);
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
