//! `_native.path`: pure path arithmetic with Unix rules (06). `clean`
//! gives the shortest equivalent path by lexical processing (`a/../b` is
//! `b`; Rust's `Path::components` keeps `..`), `join` appends and cleans
//! (a later absolute part does not discard the earlier ones), and a name
//! that starts with a dot has no extension.

use std::rc::Rc;
use std::sync::OnceLock;

use super::nat::{self, native, param, rest};
use crate::value::{Args, Module, Native, Value};
use crate::vm::{Stop, Task};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.path` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            ("join", native("join", join_fn, vec![rest("parts")])),
            ("base", native("base", base_fn, vec![param("p")])),
            ("dir", native("dir", dir_fn, vec![param("p")])),
            ("ext", native("ext", ext_fn, vec![param("p")])),
            ("stem", native("stem", stem_fn, vec![param("p")])),
            ("abs", native("abs", abs_fn, vec![param("p")])),
        ]
    });
    let mut m = Module::new("path", "_native.path");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

fn join_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let parts = nat::str_list("parts", &a[0])?;
    Ok(Value::bytes(join(&parts)))
}

fn base_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    Ok(Value::bytes(base(nat::need_bytes("p", &a[0])?)))
}

fn dir_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    Ok(Value::bytes(dir(nat::need_bytes("p", &a[0])?)))
}

fn ext_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    Ok(Value::bytes(ext(nat::need_bytes("p", &a[0])?).to_vec()))
}

fn stem_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let p = nat::need_bytes("p", &a[0])?;
    let b = base(p);
    let e = ext(&b).len();
    Ok(Value::bytes(b[..b.len() - e].to_vec()))
}

fn abs_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    Ok(Value::bytes(abs(nat::need_bytes("p", &a[0])?)))
}

/// A relative path joined onto the working directory (`$PWD` when it
/// names the same directory). Paths are bytes (L4).
pub(crate) fn abs(p: &[u8]) -> Vec<u8> {
    if p.starts_with(b"/") {
        return clean_bytes(p);
    }
    match cwd() {
        Some(wd) => join_bytes(&wd, p),
        None => clean_bytes(p),
    }
}

/// The working directory: `$PWD` when it names the same directory.
fn cwd() -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    let real = std::env::current_dir().ok()?;
    if let Some(pwd) = std::env::var_os("PWD")
        && pwd.as_bytes().starts_with(b"/")
        && std::fs::canonicalize(&pwd).ok() == std::fs::canonicalize(&real).ok()
    {
        return Some(pwd.as_bytes().to_vec());
    }
    Some(real.as_os_str().as_bytes().to_vec())
}

/// `join`: empty elements are dropped, the rest joined with
/// `/` and cleaned; no non-empty element gives `""`.
pub(crate) fn join(parts: &[Vec<u8>]) -> Vec<u8> {
    match parts.iter().position(|e| !e.is_empty()) {
        Some(i) => clean_bytes(&parts[i..].join(&b'/')),
        None => Vec::new(),
    }
}

/// The last element (`.` for `""`, `/` for a root).
pub fn base(p: &[u8]) -> Vec<u8> {
    if p.is_empty() {
        return b".".to_vec();
    }
    let end = p.iter().rposition(|&b| b != b'/').map_or(0, |i| i + 1);
    let p = &p[..end];
    let p = match p.iter().rposition(|&b| b == b'/') {
        Some(i) => &p[i + 1..],
        None => p,
    };
    if p.is_empty() {
        b"/".to_vec()
    } else {
        p.to_vec()
    }
}

/// `p` without its trailing `/`s (every function here ignores them, as
/// `base` does: LB19); `""` for a root.
fn trimmed(p: &[u8]) -> &[u8] {
    &p[..p.iter().rposition(|&b| b != b'/').map_or(0, |i| i + 1)]
}

/// Everything but the last element, cleaned; a trailing `/` is ignored
/// (`dir("a/b/")` is `a`, as the shell's `dirname`).
pub(crate) fn dir(p: &[u8]) -> Vec<u8> {
    let t = trimmed(p);
    if t.is_empty() {
        // "" or a root
        return clean_bytes(p);
    }
    let i = t.iter().rposition(|&b| b == b'/').map_or(0, |i| i + 1);
    clean_bytes(&t[..i])
}

/// From the last `.` of the last element (the one `base` gives, so
/// `stem + ext == base`), or `""`; dots that start the element are not an
/// extension.
pub(crate) fn ext(p: &[u8]) -> &[u8] {
    let p = trimmed(p);
    let name_at = p.iter().rposition(|&b| b == b'/').map_or(0, |i| i + 1);
    // Dots that start the name are part of it (`.hidden`, `..`).
    let dots = p[name_at..].iter().take_while(|&&b| b == b'.').count();
    let body_at = name_at + dots;
    match p[body_at..].iter().rposition(|&b| b == b'.') {
        Some(i) => &p[body_at + i..],
        None => b"",
    }
}

/// `join` of two parts over bytes (file names need not be
/// UTF-8): `name` alone when `dir` is empty, else the two cleaned.
pub fn join_bytes(dir: &[u8], name: &[u8]) -> Vec<u8> {
    if dir.is_empty() {
        return if name.is_empty() {
            Vec::new()
        } else {
            clean_bytes(name)
        };
    }
    let mut p = dir.to_vec();
    if !name.is_empty() {
        p.push(b'/');
        p.extend_from_slice(name);
    }
    clean_bytes(&p)
}

/// The shortest equivalent path by lexical processing alone.
pub(crate) fn clean_bytes(p: &[u8]) -> Vec<u8> {
    if p.is_empty() {
        return b".".to_vec();
    }
    let n = p.len();
    let rooted = p[0] == b'/';
    let mut out: Vec<u8> = Vec::with_capacity(n);
    let (mut r, mut dotdot) = (0, 0);
    if rooted {
        out.push(b'/');
        r = 1;
        dotdot = 1;
    }
    while r < n {
        // A separator, or a `.` element: both are skipped.
        if p[r] == b'/' || (p[r] == b'.' && (r + 1 == n || p[r + 1] == b'/')) {
            r += 1;
        } else if p[r] == b'.' && p[r + 1] == b'.' && (r + 2 == n || p[r + 2] == b'/') {
            r += 2;
            if out.len() > dotdot {
                let mut w = out.len() - 1;
                while w > dotdot && out[w] != b'/' {
                    w -= 1;
                }
                out.truncate(w);
            } else if !rooted {
                if !out.is_empty() {
                    out.push(b'/');
                }
                out.extend_from_slice(b"..");
                dotdot = out.len();
            }
        } else {
            if (rooted && out.len() != 1) || (!rooted && !out.is_empty()) {
                out.push(b'/');
            }
            while r < n && p[r] != b'/' {
                out.push(p[r]);
                r += 1;
            }
        }
    }
    if out.is_empty() {
        return b".".to_vec();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(p: &str) -> String {
        String::from_utf8(clean_bytes(p.as_bytes())).unwrap()
    }

    fn join(parts: &[&str]) -> String {
        let parts: Vec<Vec<u8>> = parts.iter().map(|p| p.as_bytes().to_vec()).collect();
        String::from_utf8(super::join(&parts)).unwrap()
    }

    fn base(p: &str) -> String {
        String::from_utf8(super::base(p.as_bytes())).unwrap()
    }

    fn dir(p: &str) -> String {
        String::from_utf8(super::dir(p.as_bytes())).unwrap()
    }

    fn ext(p: &str) -> &str {
        std::str::from_utf8(super::ext(p.as_bytes())).unwrap()
    }

    fn abs(p: &str) -> String {
        String::from_utf8(super::abs(p.as_bytes())).unwrap()
    }

    #[test]
    fn clean_resolves_dots_and_slashes() {
        let cases = [
            ("", "."),
            ("/", "/"),
            ("//", "/"),
            ("a//b/../c/", "a/c"),
            ("a/b/c", "a/b/c"),
            ("..", ".."),
            ("/..", "/"),
            ("../../a", "../../a"),
            ("a/../..", ".."),
            ("a/../../b", "../b"),
            ("/a/../..", "/"),
            ("./a/./b/.", "a/b"),
            (".", "."),
            ("a/", "a"),
            ("...", "..."),
            ("/a/b/../../..", "/"),
            ("한/글/../x", "한/x"),
        ];
        for (i, want) in cases {
            assert_eq!(clean(i), want, "clean({i:?})");
        }
    }

    #[test]
    fn join_base_dir_ext_stem() {
        assert_eq!(join(&[]), "");
        assert_eq!(join(&["", ""]), "");
        assert_eq!(join(&["a//b", "../c/"]), "a/c");
        assert_eq!(join(&["", "a"]), "a");
        assert_eq!(join(&["/", ""]), "/");
        assert_eq!(join(&["a", ".."]), ".");
        assert_eq!(super::base(b"a/\xff"), b"\xff");
        assert_eq!(super::ext(b"x.\xfe"), b".\xfe");
        assert_eq!(join_bytes(b"", b""), b"");
        assert_eq!(join_bytes(b"", b"./a"), b"a");
        assert_eq!(join_bytes(b"/", b"a"), b"/a");
        assert_eq!(join_bytes(b"a/", b""), b"a");
        assert_eq!(join_bytes(b"./x//", b"\xff"), b"x/\xff");
        assert_eq!(base(""), ".");
        assert_eq!(base("/"), "/");
        assert_eq!(base("///"), "/");
        assert_eq!(base("a/b/"), "b");
        assert_eq!(base("a/b//c.txt"), "c.txt");
        assert_eq!(dir(""), ".");
        assert_eq!(dir("/"), "/");
        assert_eq!(dir("a"), ".");
        assert_eq!(dir("/a"), "/");
        // a trailing `/` is ignored, as in `base` and the shell's
        // `dirname` (LB19)
        assert_eq!(dir("a/b/"), "a");
        assert_eq!(dir("a/b//"), "a");
        assert_eq!(dir("/a/"), "/");
        assert_eq!(dir("a/"), ".");
        assert_eq!(dir("///"), "/");
        assert_eq!(dir("a//b/../c/d"), "a/c");
        assert_eq!(dir(".."), ".");
        assert_eq!(ext(".hidden"), "");
        assert_eq!(ext(".a.b"), ".b");
        assert_eq!(ext(".."), "");
        assert_eq!(ext("a.b.c"), ".c");
        assert_eq!(ext("a.b/c"), "");
        assert_eq!(ext("a."), ".");
        assert_eq!(ext(""), "");
        // the extension of `base(p)`: `stem + ext == base` (LB19)
        assert_eq!(ext("dir.d/"), ".d");
        assert_eq!(ext("a/.b//"), "");
        assert_eq!(ext("/"), "");
        for p in ["dir.d/", "a/b.c//", "x", ".hidden/", "a.b/c.d.e", "/", ""] {
            let (b, e) = (base(p), ext(p));
            assert!(b.ends_with(e), "{p:?}: base {b:?}, ext {e:?}");
        }
    }

    #[test]
    fn abs_joins_the_working_directory() {
        // The working directory as `$PWD` or `getcwd` names it; the
        // symlinked case is covered by the CLI test `path_abs`.
        let wd = fung_front::loader::abs_path(".");
        assert_eq!(abs("/x/../y"), "/y");
        assert_eq!(abs(""), wd);
        assert_eq!(abs("a/./b"), clean(&format!("{wd}/a/b")));
    }
}
