//! `_native.path` (Go `internal/native/path.go`): pure path arithmetic over
//! Go's `path/filepath` with Unix rules. `clean` is `filepath.Clean`,
//! ported byte for byte (Rust's `Path::components` normalises differently).

use std::rc::Rc;
use std::sync::OnceLock;

use super::nat::{self, native, param, rest};
use crate::value::{Module, Native, Value};
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

fn join_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let parts = nat::str_list("parts", &a[0])?;
    Ok(Value::str(&join(&parts)))
}

fn base_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(Value::str(&base(nat::need_str("p", &a[0])?)))
}

fn dir_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(Value::str(&dir(nat::need_str("p", &a[0])?)))
}

fn ext_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(Value::str(ext(nat::need_str("p", &a[0])?)))
}

fn stem_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let p = nat::need_str("p", &a[0])?;
    let b = base(p);
    let e = ext(&b);
    Ok(Value::str(&b[..b.len() - e.len()]))
}

fn abs_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(Value::str(&abs(nat::need_str("p", &a[0])?)))
}

/// Go `filepath.Abs`: a relative path is joined onto the working
/// directory; when that cannot be read the path is only cleaned.
pub(crate) fn abs(p: &str) -> String {
    if p.starts_with('/') {
        return clean(p);
    }
    match std::env::current_dir() {
        Ok(wd) => {
            let wd = fung_front::source::decode_go(std::os::unix::ffi::OsStrExt::as_bytes(
                wd.as_os_str(),
            ));
            join(&[wd, p.to_string()])
        }
        Err(_) => clean(p),
    }
}

/// Go `filepath.Join`: empty elements are dropped, the rest joined with
/// `/` and cleaned; no non-empty element gives `""`.
pub(crate) fn join(parts: &[String]) -> String {
    match parts.iter().position(|e| !e.is_empty()) {
        Some(i) => clean(&parts[i..].join("/")),
        None => String::new(),
    }
}

/// Go `filepath.Base`.
pub(crate) fn base(p: &str) -> String {
    if p.is_empty() {
        return ".".into();
    }
    let p = p.trim_end_matches('/');
    let p = match p.rfind('/') {
        Some(i) => &p[i + 1..],
        None => p,
    };
    if p.is_empty() { "/".into() } else { p.into() }
}

/// Go `filepath.Dir`.
pub(crate) fn dir(p: &str) -> String {
    let i = p.rfind('/').map_or(0, |i| i + 1);
    clean(&p[..i])
}

/// Go `filepath.Ext`: from the last `.` after the last `/`, or `""`.
pub(crate) fn ext(p: &str) -> &str {
    for (i, c) in p.bytes().enumerate().rev() {
        if c == b'/' {
            break;
        }
        if c == b'.' {
            return &p[i..];
        }
    }
    ""
}

/// Go `filepath.Clean` (Unix): the shortest equivalent path by lexical
/// processing alone.
pub(crate) fn clean(path: &str) -> String {
    // Only ASCII bytes are dropped or added around whole characters.
    String::from_utf8(clean_bytes(path.as_bytes())).expect("clean keeps UTF-8")
}

/// Go `filepath.Join(dir, name)` over bytes (file names need not be
/// UTF-8): `name` alone when `dir` is empty, else the two cleaned.
pub(crate) fn join_bytes(dir: &[u8], name: &[u8]) -> Vec<u8> {
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

/// [`clean`] over bytes.
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

    #[test]
    fn clean_matches_go() {
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
    fn join_base_dir_ext_stem_match_go() {
        assert_eq!(join(&[]), "");
        assert_eq!(join(&["".into(), "".into()]), "");
        assert_eq!(join(&["a//b".into(), "../c/".into()]), "a/c");
        assert_eq!(join(&["".into(), "a".into()]), "a");
        assert_eq!(join(&["/".into(), "".into()]), "/");
        assert_eq!(join(&["a".into(), "..".into()]), ".");
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
        assert_eq!(dir("a/b/"), "a/b");
        assert_eq!(dir("a//b/../c/d"), "a/c");
        assert_eq!(dir(".."), ".");
        assert_eq!(ext(".hidden"), ".hidden");
        assert_eq!(ext("a.b.c"), ".c");
        assert_eq!(ext("a.b/c"), "");
        assert_eq!(ext("a."), ".");
        assert_eq!(ext(""), "");
        assert_eq!(ext("dir.d/"), "");
    }

    #[test]
    fn abs_joins_the_working_directory() {
        let wd = std::env::current_dir().unwrap();
        let wd = wd.to_str().unwrap();
        assert_eq!(abs("/x/../y"), "/y");
        assert_eq!(abs(""), clean(wd));
        assert_eq!(abs("a/./b"), clean(&format!("{wd}/a/b")));
    }
}
