//! `_native.fs` tests: Go `TestFsReadWriteListStat` and `TestFsListFull`,
//! the directory listings on a tree, the exact failure messages, the errno table, and whole runs
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
                let Ok(t) = e.file_type() else { continue };
                if t.is_dir() {
                    open_up(&e.path());
                } else if !t.is_symlink() {
                    // A link's mode is its target's: leave both alone.
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
/// UTF-8), `sub/deep/f`, an empty `emptyd`, an unreadable `locked`, a
/// write-only `wonly` and a read-only directory `ro`.
fn fixture(dir: &Path) {
    std::fs::create_dir_all(dir.join("sub/deep")).unwrap();
    std::fs::create_dir(dir.join("emptyd")).unwrap();
    std::fs::write(dir.join("a.txt"), "hi\n").unwrap();
    std::fs::write(dir.join("sub/deep/f"), "x\n").unwrap();
    std::fs::write(dir.join("bad.txt"), b"a\xffb\xc3").unwrap();
    std::fs::write(dir.join("locked"), "secret").unwrap();
    std::fs::set_permissions(dir.join("locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
    std::fs::write(dir.join("wonly"), "w").unwrap();
    std::fs::set_permissions(dir.join("wonly"), std::fs::Permissions::from_mode(0o200)).unwrap();
    std::fs::create_dir(dir.join("ro")).unwrap();
    std::fs::set_permissions(dir.join("ro"), std::fs::Permissions::from_mode(0o555)).unwrap();
    tree(&dir.join("t"));
}

/// The tree `list`, `glob` and `walk` run on: names that sort
/// differently by byte and by locale, hidden files and directories, an
/// empty directory, symbolic links (to a directory, to a file, to
/// nothing) and an unreadable directory `locked_d`.
fn tree(t: &Path) {
    for dir in [".hdir", "Z", "emptyd", "sub/A", "sub/deep", "locked_d"] {
        std::fs::create_dir_all(t.join(dir)).unwrap();
    }
    for file in [
        "a.md",
        "B.md",
        "10.txt",
        "9.txt",
        "a b.txt",
        "_u",
        ".hidden",
        ".hdir/x",
        "é.txt",
        "Z/z.md",
        "sub/b.md",
        "sub/.g",
        "sub/A/c.md",
        "sub/deep/f",
        "locked_d/inner",
    ] {
        std::fs::write(t.join(file), file).unwrap();
    }
    std::os::unix::fs::symlink("sub", t.join("ln_dir")).unwrap();
    std::os::unix::fs::symlink("a.md", t.join("ln_file")).unwrap();
    std::os::unix::fs::symlink("nope", t.join("dangling")).unwrap();
    std::fs::set_permissions(t.join("locked_d"), std::fs::Permissions::from_mode(0o000)).unwrap();
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
        "list",
        "glob",
        "walk",
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

/// Go `TestFsReadWriteListStat`'s `list`, `walk` and `glob`, and
/// `TestFsListFull`.
#[test]
fn list_walk_glob() {
    let d = Scratch::new("lwg");
    let a = d.at("a.txt");
    std::fs::write(&a, "hi\n").unwrap();
    std::fs::create_dir_all(d.at("sub/deep")).unwrap();
    std::fs::write(d.at("sub/deep/b.txt"), "").unwrap();
    let no = Value::Bool(false);
    assert_eq!(
        show(call("list", vec![p(&d.0), no.clone()])),
        r#"Ok(["a.txt", "sub"])"#
    );
    assert_eq!(
        show(call("list", vec![p(&d.0), Value::Bool(true)])),
        format!(r#"Ok(["{}", "{}"])"#, a.display(), d.at("sub").display())
    );
    assert_eq!(
        show(call(
            "list",
            vec![s(&format!("{}/./", d.text())), Value::Bool(true)]
        )),
        format!(r#"Ok(["{}", "{}"])"#, a.display(), d.at("sub").display())
    );
    assert_eq!(
        show(call("walk", vec![p(&d.0)])),
        format!(
            r#"Ok(["{}", "{}"])"#,
            a.display(),
            d.at("sub/deep/b.txt").display()
        )
    );
    assert_eq!(
        show(call("glob", vec![p(&d.at("*.txt"))])),
        format!(r#"Ok(["{}"])"#, a.display())
    );
    assert_eq!(
        show(call("glob", vec![p(&d.at("*/*/*"))])),
        format!(r#"Ok(["{}"])"#, d.at("sub/deep/b.txt").display())
    );
    let none = d.at("none");
    assert_eq!(
        show(call("list", vec![p(&none), no.clone()])),
        format!(
            "Err({{kind: 'not_found, message: \"cannot list `{}`: no such file or directory\"}})",
            none.display()
        )
    );
    assert_eq!(
        show(call("list", vec![p(&a), no.clone()])),
        format!(
            "Err({{kind: 'io, message: \"cannot list `{}`: not a directory\"}})",
            a.display()
        )
    );
    assert_eq!(
        show(call("walk", vec![p(&none)])),
        format!(
            "Err({{kind: 'not_found, message: \"cannot walk `{}`: no such file or directory\"}})",
            none.display()
        )
    );
    assert_eq!(
        show(call("walk", vec![p(&a)])),
        format!(r#"Ok(["{}"])"#, a.display())
    );
    assert_eq!(show(call("glob", vec![p(&none)])), "Ok([])");
    assert_eq!(
        show(call("glob", vec![s("x/[")])),
        "Err({kind: 'invalid, message: \"bad glob pattern `x/[`: syntax error in pattern\"})"
    );
    assert_eq!(show(call("list", vec![p(&d.0), Value::Int(1)])), "E0403");
    assert_eq!(show(call("list", vec![Value::Nil, no])), "E0403");
    assert_eq!(show(call("glob", vec![Value::Nil])), "E0403");
    assert_eq!(show(call("walk", vec![Value::Nil])), "E0403");
}

/// Byte order (not locale order), hidden names, links not followed by
/// `walk` and followed by `glob` and `list`, and an unreadable directory:
/// `glob` passes over it, `walk` stops there.
#[test]
fn listings_keep_gos_order() {
    let d = Scratch::new("order");
    let t = d.at("t");
    tree(&t);
    let tt = t.display().to_string();
    let quoted = |xs: &[&str], full: bool| {
        let xs: Vec<String> = xs
            .iter()
            .map(|x| {
                if full {
                    format!("\"{tt}/{x}\"")
                } else {
                    format!("\"{x}\"")
                }
            })
            .collect();
        format!("Ok([{}])", xs.join(", "))
    };
    let top = [
        ".hdir", ".hidden", "10.txt", "9.txt", "B.md", "Z", "_u", "a b.txt", "a.md", "dangling",
        "emptyd", "ln_dir", "ln_file", "locked_d", "sub", "é.txt",
    ];
    assert_eq!(
        show(call("list", vec![p(&t), Value::Bool(false)])),
        quoted(&top, false)
    );
    assert_eq!(
        show(call("list", vec![p(&t), Value::Bool(true)])),
        quoted(&top, true)
    );
    assert_eq!(
        show(call("glob", vec![s(&format!("{tt}/*"))])),
        quoted(&top, true)
    );
    assert_eq!(
        show(call("glob", vec![s(&format!("{tt}/*/*"))])),
        quoted(
            &[
                ".hdir/x",
                "Z/z.md",
                "ln_dir/.g",
                "ln_dir/A",
                "ln_dir/b.md",
                "ln_dir/deep",
                "sub/.g",
                "sub/A",
                "sub/b.md",
                "sub/deep",
            ],
            true
        )
    );
    assert_eq!(
        show(call("walk", vec![p(&t.join("sub"))])),
        quoted(&["sub/.g", "sub/A/c.md", "sub/b.md", "sub/deep/f"], true)
    );
    assert_eq!(
        show(call("walk", vec![p(&t.join("ln_dir"))])),
        quoted(&["ln_dir"], true)
    );
    assert_eq!(
        show(call("walk", vec![s(&format!("{tt}/ln_dir/"))])),
        quoted(
            &["ln_dir/.g", "ln_dir/A/c.md", "ln_dir/b.md", "ln_dir/deep/f"],
            true
        )
    );
    if running_as_root(&t.join("locked_d")) {
        return;
    }
    assert_eq!(
        show(call("walk", vec![p(&t)])),
        format!("Err({{kind: 'permission, message: \"cannot walk `{tt}`: permission denied\"}})")
    );
    assert_eq!(
        show(call(
            "list",
            vec![p(&t.join("locked_d")), Value::Bool(false)]
        )),
        format!(
            "Err({{kind: 'permission, message: \"cannot list `{tt}/locked_d`: permission denied\"}})"
        )
    );
    assert_eq!(
        show(call("glob", vec![s(&format!("{tt}/locked_d/*"))])),
        "Ok([])"
    );
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

/// The errno texts (`oserr::ERRORS`, recorded per system): the common
/// errors' words, one line of text for every errno from 0 to 160, and
/// `errno n` where the table has none.
#[test]
fn errno_table_reads_as_recorded() {
    let t = oserr::errno_text;
    assert_eq!(t(0), "errno 0");
    assert_eq!(t(1), "operation not permitted");
    assert_eq!(t(oserr::ENOENT), "no such file or directory");
    assert_eq!(t(oserr::EACCES), "permission denied");
    assert_eq!(t(oserr::EEXIST), "file exists");
    assert_eq!(t(oserr::ENOTDIR), "not a directory");
    assert_eq!(t(21), "is a directory");
    assert_eq!(t(oserr::ENOTEMPTY), "directory not empty");
    assert_eq!(t(-1), "errno -1");
    assert_eq!(t(100_000), "errno 100000");
    for n in 0..=160 {
        let text = t(n);
        assert!(
            !text.is_empty() && !text.contains('\n'),
            "errno {n}: {text:?}"
        );
    }
}

// --- file handles ---

/// Runs `body` after `import fs` with `d` bound to the scratch directory.
fn run_in(d: &Scratch, body: &str) -> testutil::Outcome {
    exec(
        &format!("import fs\nd = \"{}\"\n{body}", d.text()),
        Opts::default(),
    )
}

/// Go `TestFsHandles` and `TestReadLineOnWriteHandle`: write, close,
/// closed and wrong-direction handles, `read_line` and `read_lines` on a
/// handle, append mode, the handle's text, type and equality.
#[test]
fn handles_like_go() {
    let d = Scratch::new("handles");
    let o = run_in(
        &d,
        r#"h = fs.open("${d}/h.txt", 'write)!
print(fs.write_line(h, "one"), fs.write_line(h, "two"), fs.read("${d}/h.txt"))
print(fs.close(h), fs.close(h), fs.write_line(h, "x"), read_line(h))
r = fs.open("${d}/h.txt")!
print(fs.write_line(r, "x"), read_line(r), read_lines(r), read_line(r))
print(fs.close(r), read_line(r), read_lines(r))
a = fs.open("${d}/h.txt", 'append)!
print(fs.write_line(a, "three"), read_line(a), fs.close(a), fs.read("${d}/h.txt"))
print(type_of(a), a == a, a == r, a != h)
print(fs.with_open("${d}/h.txt", read_lines), fs.with_open("${d}/none", f => 1) |> is_err)"#,
    );
    let h = d.at("h.txt");
    let h = h.display();
    assert_eq!(
        o.out,
        format!(
            "Ok(nil) Ok(nil) Ok(\"\")
Ok(nil) Err({{kind: 'closed, message: \"file `{h}` is closed\"}}) Err({{kind: 'closed, message: \"file `{h}` is closed\"}}) Err({{kind: 'closed, message: \"file `{h}` is closed\"}})
Err({{kind: 'invalid, message: \"file `{h}` is not open for writing\"}}) Ok(\"one\") Ok([\"two\"]) Ok(nil)
Ok(nil) Err({{kind: 'closed, message: \"file `{h}` is closed\"}}) Err({{kind: 'closed, message: \"file `{h}` is closed\"}})
Ok(nil) Err({{kind: 'invalid, message: \"file `{h}` is not open for reading\"}}) Ok(nil) Ok(\"one\\ntwo\\nthree\\n\")
'file true false true
Ok([\"one\", \"two\", \"three\"]) true
"
        ),
        "{}",
        o.err
    );
    let o = run_in(&d, r#"print(fs.open("${d}/h.txt")!)"#);
    assert_eq!(o.out, format!("<file \"{h}\">\n"));
    let o = run_in(&d, r#"print(fs.open(d, 'rw))"#);
    assert_eq!(o.code, "E0403");
    assert!(
        o.err.starts_with(
            "error[E0403]: argument `mode` must be 'read | 'write | 'append, got 'rw\n"
        ),
        "{}",
        o.err
    );
}

/// A handle shows its path the way Go's `strconv.Quote` does (`\x01`,
/// not fung's `\u{1}`), and other values holding it show it the same way.
#[test]
fn handle_text_quotes_like_go() {
    let d = Scratch::new("quote");
    let o = run_in(
        &d,
        r#"h = fs.open("${d}/\u{1}é\t\"\\\${\u{7f}\u{a0}\u{e000}\u{e0001}", 'write)!
print([h], h)"#,
    );
    let q = format!(
        "{}/\\x01é\\t\\\"\\\\${{\\x7f\\u00a0\\ue000\\U000e0001",
        d.text()
    );
    assert_eq!(
        o.out,
        format!("[<file \"{q}\">] <file \"{q}\">\n"),
        "{}",
        o.err
    );
}

/// Review Focus 2: eight tasks write 200 lines each into one handle at
/// the same time; every line arrives whole (the handle's lock covers a
/// whole `write_line`, which runs without the run token). The lines are
/// longer than the 4096-byte buffer is divided by, so flushes fall inside
/// lines.
#[test]
fn concurrent_write_lines_stay_whole() {
    let d = Scratch::new("concurrent");
    let o = run_in(
        &d,
        r#"h = fs.open("${d}/out.txt", 'write)!
pad = repeat("x", 300)
ts = for i in 0..8: spawn(() => each(0..200, j => fs.write_line(h, "task ${i} line ${j} ${pad}")!)) ;;
each(ts, t => await(t))
print(fs.close(h))"#,
    );
    assert_eq!((o.out.as_str(), o.err.as_str()), ("Ok(nil)\n", ""));
    let text = std::fs::read_to_string(d.at("out.txt")).unwrap();
    let pad = "x".repeat(300);
    let mut seen: Vec<String> = text.lines().map(str::to_string).collect();
    assert_eq!(seen.len(), 1600);
    seen.sort();
    let mut want: Vec<String> = (0..8)
        .flat_map(|i| (0..200).map(move |j| (i, j)))
        .map(|(i, j)| format!("task {i} line {j} {pad}"))
        .collect();
    want.sort();
    assert!(seen == want, "some lines are torn or missing");
}

/// Review Focus 3: a handle closes its file when its last copy goes, so
/// thousands of handles dropped without `close` leak no descriptors. The
/// program runs in a child process (this test binary again) whose
/// descriptor limit is 128: keeping 5000 handles fails there, dropping
/// them one at a time does not.
#[test]
fn dropped_handles_free_their_descriptors() {
    const CHILD: &str = "FUNG_FD_LIMIT_CHILD";
    let name = concat!(module_path!(), "::dropped_handles_free_their_descriptors");
    let name = name.split_once("::").unwrap().1;
    if std::env::var(CHILD).is_err() {
        let exe = std::env::current_exe().unwrap();
        let o = std::process::Command::new("sh")
            .args(["-c", "ulimit -n 128 && exec \"$@\"", "sh"])
            .arg(exe)
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(
            o.status.success() && out.contains("1 passed"),
            "child failed:\n{out}\n{}",
            String::from_utf8_lossy(&o.stderr)
        );
        return;
    }
    let d = Scratch::new("fdleak");
    std::fs::write(d.at("a.txt"), "hi\n").unwrap();
    let o = run_in(
        &d,
        r#"p = "${d}/a.txt"
fn keep(p, n) = map(0..n, _ => fs.open(p)) |> filter(is_err) |> take(1)
print(keep(p, 5000))
each(0..5000, _ => fs.open(p)!)
each(0..5000, _ => fs.open("${d}/w.txt", 'write)!)
print(fs.open(p) |> is_ok, fs.with_open(p, read_lines))"#,
    );
    assert_eq!(
        o.out,
        format!(
            "[Err({{kind: 'io, message: \"cannot open `{}`: too many open files\"}})]\ntrue Ok([\"hi\"])\n",
            d.at("a.txt").display()
        ),
        "{}",
        o.err
    );
}

/// File input and output run without the run token: two tasks open the
/// two ends of a FIFO, and each open blocks until the other end is open,
/// in whichever order they run; then one writes and the other reads.
/// Holding the token while blocked would never end (the bound turns that
/// into a failure; no sleep orders the tasks). The writer uses 'append
/// (write only): 'write opens read-write like Go's `os.Create`, which
/// does not wait for a reader, so its data could be gone before the
/// reader opens (in Go too).
#[test]
fn file_io_runs_without_the_run_token() {
    let d = Scratch::new("fifo");
    let fifo = d.at("pipe");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(made.success());
    let src = format!(
        "import fs\np = \"{}\"\nr = spawn(() => fs.with_open(p, read_lines))\nw = spawn(fn(): h = fs.open(p, 'append)!; each(0..3, i => fs.write_line(h, \"l${{i}}\")!); fs.close(h) ;;)\nprint(await(w), await(r))",
        fifo.display()
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let o = exec(&src, Opts::default());
        let _ = tx.send((o.out, o.err));
    });
    let (out, err) = rx
        .recv_timeout(std::time::Duration::from_secs(60))
        .expect("blocked file input held the run token");
    assert_eq!(
        (out.as_str(), err.as_str()),
        ("Ok(nil) Ok([\"l0\", \"l1\", \"l2\"])\n", "")
    );
}

// --- whole runs against their expected output ---

/// Programs run in a fresh `fixture` directory bound to `d` (the programs
/// that need no fixture are in `prelude/run_cases.rs`).
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
    r#"h = fs.open("${d}/h.txt", 'write)!
print(h, type_of(h), h == h, h == fs.open("${d}/a.txt")!, [h] == [h])
print(fs.write_line(h, "one"), fs.write_line(h, "two"), fs.read("${d}/h.txt"))
print(fs.close(h), fs.close(h), fs.write_line(h, "x"), read_line(h), read_lines(h), fs.read("${d}/h.txt"))
r = fs.open("${d}/h.txt")!
print(fs.write_line(r, "x"), read_line(r), read_lines(r), read_line(r), read_lines(r))
print(fs.close(r), read_line(r), read_lines(r), fs.write_line(r, "x"))
a = fs.open("${d}/h.txt", 'append)!
print(fs.write_line(a, "three"), read_line(a), read_lines(a), fs.close(a), fs.read("${d}/h.txt"))
print(fs.open("${d}/none"), fs.open("${d}/nope/x", 'write), fs.open("${d}/nope/x", 'append))
print(fs.open("${d}/locked"), fs.open("${d}/locked", 'write), fs.open("${d}/locked", 'append))
print(fs.open("${d}/wonly"), fs.open("${d}/wonly", 'write), fs.open("${d}/wonly", 'append) |> is_ok)
print(fs.open("${d}/ro/x", 'write), fs.open(d, 'write), fs.open(d, 'append), fs.open("${d}/a.txt/x"))
print(fs.open("a\u{0}b"), fs.open("a\u{0}b", 'write), fs.open(""), fs.open("", 'append))
print(fs.with_open(d, read_lines), fs.with_open("${d}/bad.txt", read_lines), fs.with_open("${d}/a.txt", h => [read_line(h), read_line(h)]))
print(fs.with_open("${d}/sub/deep/f", read_lines, mode: 'read), fs.with_open("${d}/w.txt", h => fs.write_line(h, "w"), mode: 'write), fs.read("${d}/w.txt"))
print(fs.with_open("${d}/none", h => 1), fs.with_open("${d}/w.txt", h => read_line(h), 'append), fs.read_lines("${d}/w.txt"), fs.read_lines("${d}/none"))
print(fs.write("${d}/crlf.txt", "a\r\nb\r\n\r\n\nc\r"), fs.with_open("${d}/crlf.txt", read_lines), fs.read_lines("${d}/crlf.txt"))
fn lost(p) = fs.write_line(fs.open(p, 'write)!, "lost")
print(lost("${d}/lost.txt"), fs.read("${d}/lost.txt"))
k = fs.with_open("${d}/a.txt", h => h)
print(k, read_line(k), fs.close(k), fs.write_line(k, ""))"#,
    r#"p = "${d}/big.txt"
w = fs.open(p, 'write)!
fn size(p) = len(fs.read(p)!)
print(fs.write_line(w, repeat("a", 4094)), size(p))
print(fs.write_line(w, "b"), size(p))
print(fs.write_line(w, repeat("c", 5000)), size(p))
print(fs.write_line(w, repeat("d", 9000)), size(p))
print(fs.write_line(w, repeat("e", 10000)), size(p))
print(fs.write_line(w, ""), fs.write_line(w, "한글"), size(p))
print(fs.close(w), size(p))
r = fs.open(p)!
print(fold(read_lines(r)!, "", (acc, l) => "${acc} ${len(l)}"), read_line(r), fs.close(r))
x = fs.open(p, 'append)!
print(fs.write_line(x, repeat("f", 5000)), size(p), fs.close(x), size(p))"#,
    r#"t = "${d}/t"
print(fs.list(t))
print(fs.list(t, full: true))
print(fs.list("${t}/", true))
print(fs.list("${t}/sub/../sub/", full: true))
print(fs.list("${t}/emptyd"), fs.list("${t}/emptyd", full: true))
print(fs.list("${t}/ln_dir"), fs.list("${t}/ln_dir", full: true))
print(fs.list("${t}/a.md"))
print(fs.list("${t}/ln_file"))
print(fs.list("${t}/dangling"))
print(fs.list("${t}/none"))
print(fs.list("${t}/locked_d"))
print(fs.list(""), fs.list("a\u{0}b"))
print(fs.list("src", full: true) == fs.glob("src/*"))"#,
    r#"t = "${d}/t"
print(fs.walk(t))
print(fs.walk("${t}/sub"))
print(fs.walk("${t}/sub/"))
print(fs.walk("${t}//sub/./"))
print(fs.walk("${t}/ln_dir"))
print(fs.walk("${t}/ln_dir/"))
print(fs.walk("${t}/a.md"), fs.walk("${t}/ln_file"), fs.walk("${t}/dangling"))
print(fs.walk("${t}/emptyd"), fs.walk("${t}/Z"), fs.walk("${t}/.hdir"))
print(fs.walk("${t}/none"))
print(fs.walk("${t}/a.md/"))
print(fs.walk("${t}/locked_d"))
print(fs.walk(""), fs.walk("a\u{0}b"))
print(fs.walk("src") == fs.walk("./src"))"#,
    r#"t = "${d}/t"
print(fs.glob("${t}/*"))
print(fs.glob("${t}/.*"))
print(fs.glob("${t}/*/*"))
print(fs.glob("${t}/*.md"))
print(fs.glob("${t}/[a-z]*"))
print(fs.glob("${t}/[^a-z]*"))
print(fs.glob("${t}/[!a]*"))
print(fs.glob("${t}/s*/*/*"))
print(fs.glob("${t}/**"))
print(fs.glob("${t}/?.md"), fs.glob("${t}/?.txt"), fs.glob("${t}/??.txt"))
print(fs.glob("${t}//*.md"))
print(fs.glob("${t}/s*/"))
print(fs.glob("${t}/*/deep"))
print(fs.glob("${t}/a.md"), fs.glob("${t}/none"), fs.glob("${t}/dangling"), fs.glob("${t}/a.md/"))
print(fs.glob("${t}/\\a.md"), fs.glob("${t}/a\\ b.txt"), fs.glob("${t}/\\[a].md"))
print(fs.glob("${t}/locked_d/*"), fs.glob("${t}/locked_d"))
print(fs.glob("${t}/ln_*/*"))
print(fs.glob("${t}/*/A/*"))
print(fs.glob("${t}/["))
print(fs.glob("${t}/*/["))
print(fs.glob("${t}/*["))
print(fs.glob("${t}/[/*"))
print(fs.glob("${t}/a[/*"))
print(fs.glob("${t}/none/["))
print(fs.glob("${t}/\\"))
print(fs.glob(""), fs.glob("*.toml"), fs.glob("src/nat*/glob.rs"), fs.glob("./*.toml"))
print(fs.glob("a\u{0}*"), fs.glob("a\u{0}b"))"#,
    "print(fs.list(d, full: 1))",
    "print(fs.list(d, true, 1))",
];

/// `text` with each directory's size (`size: N, is_dir: true`) as
/// `size: $SIZE`: it depends on the file system.
fn without_dir_sizes(text: &str) -> String {
    const TAIL: &str = ", is_dir: true";
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("size: ") {
        let after = &rest[i + 6..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        out.push_str(&rest[..i + 6]);
        if digits > 0 && after[digits..].starts_with(TAIL) {
            out.push_str("$SIZE");
        } else {
            out.push_str(&after[..digits]);
        }
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

#[test]
fn directory_sizes_are_masked() {
    assert_eq!(
        without_dir_sizes("{size: 96, is_dir: true} {size: 9, is_dir: false} size: x"),
        "{size: $SIZE, is_dir: true} {size: 9, is_dir: false} size: x"
    );
}

#[test]
fn runs_match_the_expected_output() {
    use testutil::expected;
    let d = Scratch::new("runs");
    let dir = d.text();
    let mut got = Vec::new();
    let mut as_root = false;
    for (i, body) in PROGRAMS.iter().enumerate() {
        let decl = format!("d = \"{dir}\"\n");
        let _ = restore_and_remove(&d.0);
        std::fs::create_dir_all(&d.0).unwrap();
        fixture(&d.0);
        // Root reads the locked file anyway: the expected output is a
        // normal user's.
        as_root |= std::fs::read(d.at("locked")).is_ok();
        let mut o = exec(&format!("import fs\n{decl}{body}"), Opts::default());
        o.out = without_dir_sizes(&o.out);
        got.push(testutil::outcome_entry(
            &format!("program {i}"),
            &o,
            &[(dir, "$D")],
        ));
    }
    assert_eq!(got.len(), 9, "cases");
    if as_root {
        return;
    }
    expected::verify(
        "fs/runs.txt",
        "Whole runs of `PROGRAMS` (crates/fung-vm/src/native/fs_tests.rs) in a fresh fixture\n\
         directory; $D stands for it, $SIZE for a directory size. Update: FUNG_EXPECTED=bless cargo test -p fung-vm native::fs::tests::runs_match",
        64 * 1024,
        &got,
    );
}
