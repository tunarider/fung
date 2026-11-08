//! The prelude's string functions. Strings are bytes read as UTF-8
//! characters (02, L4); `upper` and `lower` use the full Unicode case
//! mapping.

use fung_front::diag::Arg;

use crate::native::nat::{self, def, def_step, fail, opt, param};
use crate::native::walk::{Collect, Cursor, drain_with};
use fung_front::text::{Unit, char_count, units};

use std::rc::Rc;

use crate::value::{List, Lit, Text, Value, write_for_print};
use crate::vm::pieces::{self, STEP_ITEMS, STEP_SCAN, Work};
use crate::vm::{Flow, Stop, Task};

pub(super) fn add(add: &mut impl FnMut(Value)) {
    add(def_step("split", split_fn, vec![param("s"), param("sep")]));
    add(def_step("lines", lines_fn, vec![param("s")]));
    add(def_step(
        "join",
        join_fn,
        vec![param("xs"), opt("sep", Lit::Str(""))],
    ));
    add(def("trim", trim_fn, vec![param("s"), opt("cut", Lit::Nil)]));
    add(def("upper", upper_fn, vec![param("s")]));
    add(def("lower", lower_fn, vec![param("s")]));
    add(def_step(
        "replace",
        replace_fn,
        vec![param("s"), param("old"), param("new")],
    ));
    add(def(
        "starts_with",
        starts_with_fn,
        vec![param("s"), param("prefix")],
    ));
    add(def(
        "ends_with",
        ends_with_fn,
        vec![param("s"), param("suffix")],
    ));
    add(def_step("repeat", repeat_fn, vec![param("s"), param("n")]));
    add(def("byte_len", byte_len_fn, vec![param("s")]));
    add(def("bytes", bytes_fn, vec![param("s")]));
    add(def("from_bytes", from_bytes_fn, vec![param("xs")]));
    add(def("valid_utf8", valid_utf8_fn, vec![param("s")]));
}

fn string(s: Vec<u8>) -> Value {
    Value::bytes(s)
}

/// The bytes of a string value (checked as one before).
fn bytes_of(v: &Value) -> &[u8] {
    match v {
        Value::Str(t) => t,
        _ => unreachable!("checked as a string"),
    }
}

/// A walk over a string's occurrences of a separator (not empty), a
/// piece at a time (L6, `vm/pieces.rs`): it hands on the parts between
/// them, looking at about `STEP_SCAN` bytes or `STEP_ITEMS` parts each
/// time. The search window reaches past the piece by the separator's
/// length less one, so an occurrence across the line is found once.
struct Cuts {
    s: Value,
    /// Only `s[..end]` is cut (`lines` leaves a final newline out).
    end: usize,
    finder: memchr::memmem::Finder<'static>,
    /// The part being found starts at `from`; the search goes on at `at`.
    from: usize,
    at: usize,
}

impl Cuts {
    fn new(s: Value, end: usize, sep: &[u8]) -> Cuts {
        Cuts {
            s,
            end,
            finder: memchr::memmem::Finder::new(sep).into_owned(),
            from: 0,
            at: 0,
        }
    }

    /// Hands `f` the next parts, the last with `true`; true once the last
    /// one was handed on.
    fn run(&mut self, mut f: impl FnMut(&[u8], bool)) -> bool {
        let s = &bytes_of(&self.s)[..self.end];
        let sep = self.finder.needle().len();
        let mut parts = 0;
        while parts < STEP_ITEMS {
            let stop = (self.at + STEP_SCAN).min(s.len());
            let window = (stop + sep - 1).min(s.len());
            match self.finder.find(&s[self.at..window]) {
                Some(i) => {
                    let m = self.at + i;
                    f(&s[self.from..m], false);
                    self.from = m + sep;
                    self.at = self.from;
                    parts += 1;
                }
                None if window == s.len() => {
                    f(&s[self.from..], true);
                    return true;
                }
                None => {
                    self.at = stop;
                    return false;
                }
            }
        }
        false
    }
}

/// The characters of a string from `at`, a piece at a time (an empty
/// separator, `split` and `replace`).
fn chars_from(s: &[u8], at: &mut usize, mut f: impl FnMut(&[u8])) -> bool {
    for _ in 0..STEP_ITEMS {
        let Some(u) = fung_front::text::first(&s[*at..]) else {
            return true;
        };
        let n = u.byte_len();
        f(&s[*at..*at + n]);
        *at += n;
    }
    *at == s.len()
}

/// `split(s, sep)`: an empty separator splits into characters, and splitting
/// the empty string by it gives no parts.
fn split_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let [s, sep] = nat::bytes_args(&a, ["s", "sep"])?;
    if sep.is_empty() {
        struct Chars {
            s: Value,
            at: usize,
            out: List,
        }
        impl Work for Chars {
            fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
                let out = &mut self.out;
                if !chars_from(bytes_of(&self.s), &mut self.at, |c| {
                    out.push(string(c.to_vec()))
                }) {
                    return Ok(None);
                }
                let out = std::mem::replace(&mut self.out, List::new(Vec::new()));
                Ok(Some(Value::List(Rc::new(out))))
            }
        }
        return pieces::run(
            t,
            Chars {
                s: a[0].clone(),
                at: 0,
                out: List::new(Vec::new()),
            },
        );
    }
    let cuts = Cuts::new(a[0].clone(), s.len(), sep);
    pieces::run(t, Parts::new(cuts, |p| p))
}

/// The parts of a `Cuts` walk as a list, each mapped by `part`.
struct Parts {
    cuts: Cuts,
    part: fn(&[u8]) -> &[u8],
    /// Grown in place: a vector made into a list at the end would be one
    /// long step.
    out: List,
}

impl Parts {
    fn new(cuts: Cuts, part: fn(&[u8]) -> &[u8]) -> Parts {
        Parts {
            cuts,
            part,
            out: List::new(Vec::new()),
        }
    }
}

impl Work for Parts {
    fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
        let (out, part) = (&mut self.out, self.part);
        if !self.cuts.run(|p, _| out.push(string(part(p).to_vec()))) {
            return Ok(None);
        }
        let out = std::mem::replace(&mut self.out, List::new(Vec::new()));
        Ok(Some(Value::List(Rc::new(out))))
    }
}

/// `lines(s)`: `s` split on `\n`, one `\r` that ends a line dropped, and
/// no empty line after a final newline; the empty string has no lines (06,
/// the rule of `read_lines`).
fn lines_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let s = nat::need_bytes("s", &a[0])?;
    if s.is_empty() {
        return Ok(Flow::Done(nat::list(Vec::new())));
    }
    let end = s.len() - usize::from(s.ends_with(b"\n"));
    let cuts = Cuts::new(a[0].clone(), end, b"\n");
    pieces::run(t, Parts::new(cuts, |l| l.strip_suffix(b"\r").unwrap_or(l)))
}

/// `join(xs, sep = "")` renders elements as print would (06): strings
/// bare, the rest in literal form.
fn join_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    nat::check_items(&a[0], "join")?;
    let sep = nat::need_bytes("sep", &a[1])?.to_vec();
    // Every element shows as one character at least.
    if let Value::Range(r) = &a[0]
        && r.len() > i128::from(MAX_CHARS)
    {
        return Err(too_many_chars());
    }
    struct Join {
        sep: Vec<u8>,
        out: Vec<u8>,
        chars: i64,
        first: bool,
    }
    impl Collect for Join {
        fn item(&mut self, x: Value) -> Result<bool, Stop> {
            let before = self.out.len();
            if !self.first {
                self.out.extend_from_slice(&self.sep);
            }
            self.first = false;
            write_for_print(&x, &mut self.out);
            self.chars += char_count(&self.out[before..]) as i64;
            if self.chars > MAX_CHARS {
                return Err(too_many_chars());
            }
            Ok(false)
        }
        fn result(&mut self, _: &mut Task<'_>) -> Result<Flow, Stop> {
            Ok(Flow::Done(string(std::mem::take(&mut self.out))))
        }
    }
    let p = Join {
        sep,
        out: Vec::new(),
        chars: 0,
        first: true,
    };
    drain_with(t, Cursor::new(&a[0]), p)
}

/// `trim(s, cut = nil)`: whitespace (the Unicode White_Space property),
/// or the characters of `cut`, off both ends. A byte outside UTF-8 is cut
/// only when `cut` holds it.
fn trim_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_bytes("s", &a[0])?;
    let cut: Vec<Unit> = match a[1] {
        Value::Nil => Vec::new(),
        _ => units(nat::need_bytes("cut", &a[1])?)
            .map(|(_, u)| u)
            .collect(),
    };
    let space = matches!(a[1], Value::Nil);
    let gone = |u: Unit| match u {
        Unit::Char(c) if space => c.is_whitespace(),
        u => cut.contains(&u),
    };
    let us: Vec<(usize, Unit)> = units(s).collect();
    let lo = us.iter().position(|&(_, u)| !gone(u));
    let Some(lo) = lo else {
        return Ok(string(Vec::new()));
    };
    let hi = us.iter().rposition(|&(_, u)| !gone(u)).expect("lo is kept");
    let (from, (at, last)) = (us[lo].0, us[hi]);
    Ok(string(s[from..at + last.byte_len()].to_vec()))
}

/// `upper(s)` and `lower(s)`: the full Unicode case mapping (06), so the
/// length may change (`ß` → `SS`) and a final sigma lowercases to `ς`.
/// Bytes outside UTF-8 are no letters and stay as they are (L4).
fn upper_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_text("s", &a[0])?;
    Ok(string(map_text(s, str::to_uppercase)))
}

fn lower_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_text("s", &a[0])?;
    Ok(string(map_text(s, str::to_lowercase)))
}

/// `f` over each valid stretch of `s`, the invalid bytes between kept.
fn map_text(s: &Text, f: fn(&str) -> String) -> Vec<u8> {
    if let Some(s) = s.as_str() {
        return f(s).into_bytes();
    }
    let mut out = Vec::with_capacity(s.len());
    for c in s.utf8_chunks() {
        out.extend_from_slice(f(c.valid()).as_bytes());
        out.extend_from_slice(c.invalid());
    }
    out
}

/// `replace(s, old, new)`: every occurrence; an empty `old` matches before
/// every character and at the end. A result past `MAX_CHARS` characters is
/// E0806 before anything is built (02). Built a piece at a time (L6).
fn replace_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let [s, old, new] = nat::bytes_args(&a, ["s", "old", "new"])?;
    if !replace_fits(s, old, new) {
        return Err(too_many_chars());
    }
    let new = new.to_vec();
    let out = Vec::with_capacity(s.len());
    if old.is_empty() {
        struct Between {
            s: Value,
            new: Vec<u8>,
            at: usize,
            out: Vec<u8>,
        }
        impl Work for Between {
            fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
                let (out, new) = (&mut self.out, &self.new);
                if !chars_from(bytes_of(&self.s), &mut self.at, |c| {
                    out.extend_from_slice(new);
                    out.extend_from_slice(c);
                }) {
                    return Ok(None);
                }
                self.out.extend_from_slice(&self.new);
                Ok(Some(string(std::mem::take(&mut self.out))))
            }
        }
        return pieces::run(
            t,
            Between {
                s: a[0].clone(),
                new,
                at: 0,
                out,
            },
        );
    }
    struct Replace {
        cuts: Cuts,
        new: Vec<u8>,
        out: Vec<u8>,
    }
    impl Work for Replace {
        fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            let (out, new) = (&mut self.out, &self.new);
            if !self.cuts.run(|p, last| {
                out.extend_from_slice(p);
                if !last {
                    out.extend_from_slice(new);
                }
            }) {
                return Ok(None);
            }
            Ok(Some(string(std::mem::take(&mut self.out))))
        }
    }
    let cuts = Cuts::new(a[0].clone(), s.len(), old);
    pieces::run(t, Replace { cuts, new, out })
}

/// Whether `replace(s, old, new)` stays within `MAX_CHARS`. The size of
/// the result grows with the number of matches, so this counts them only
/// when the most there could be might pass the limit, and counts
/// characters only when the bytes do.
fn replace_fits(s: &[u8], old: &[u8], new: &[u8]) -> bool {
    let limit = MAX_CHARS as i128;
    let (len, old_len, new_len) = (s.len() as i128, old.len() as i128, new.len() as i128);
    let most = if old.is_empty() {
        len + 1
    } else {
        len / old_len
    };
    if len + most * (new_len - old_len).max(0) <= limit {
        return true;
    }
    let n = if old.is_empty() {
        char_count(s) as i128 + 1
    } else {
        memchr::memmem::find_iter(s, old).count() as i128
    };
    if len + n * (new_len - old_len) <= limit {
        return true;
    }
    let chars = |x: &[u8]| char_count(x) as i128;
    chars(s) + n * (chars(new) - chars(old)) <= limit
}

fn starts_with_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [s, prefix] = nat::bytes_args(&a, ["s", "prefix"])?;
    Ok(Value::Bool(s.starts_with(prefix)))
}

fn ends_with_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let [s, suffix] = nat::bytes_args(&a, ["s", "suffix"])?;
    Ok(Value::Bool(s.ends_with(suffix)))
}

/// Bounds the strings `repeat` (and fung's `pad_*`) may build (E0806).
pub(crate) const MAX_CHARS: i64 = 100_000_000;

/// E0806 for a string past [`MAX_CHARS`].
pub(crate) fn too_many_chars() -> Stop {
    fail("E0806", vec![nat::s("characters"), Arg::Int(MAX_CHARS)])
}

/// `repeat(s, n)`; a negative `n` is 0. A result past `MAX_CHARS`
/// characters is E0806 (02). Written a piece at a time (L6).
fn repeat_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    let s = nat::need_bytes("s", &a[0])?;
    let n = nat::need_int("n", &a[1])?.max(0);
    if (char_count(s) as i64)
        .checked_mul(n)
        .is_none_or(|c| c > MAX_CHARS)
    {
        return Err(too_many_chars());
    }
    struct Repeat {
        s: Vec<u8>,
        left: usize,
        out: Vec<u8>,
    }
    impl Work for Repeat {
        fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            let per = (STEP_SCAN / self.s.len().max(1)).max(1);
            let k = per.min(self.left);
            for _ in 0..k {
                self.out.extend_from_slice(&self.s);
            }
            self.left -= k;
            if self.left > 0 {
                return Ok(None);
            }
            Ok(Some(string(std::mem::take(&mut self.out))))
        }
    }
    if s.is_empty() || n == 0 {
        return Ok(Flow::Done(string(Vec::new())));
    }
    let n = n as usize;
    pieces::run(
        t,
        Repeat {
            out: Vec::with_capacity(s.len() * n),
            s: s.to_vec(),
            left: n,
        },
    )
}

/// `byte_len(s)`: the number of bytes in `s` (L4).
fn byte_len_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(Value::Int(nat::need_bytes("s", &a[0])?.len() as i64))
}

/// `bytes(s)`: the bytes of `s` as ints from 0 to 255.
fn bytes_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_bytes("s", &a[0])?;
    Ok(nat::list(
        s.iter().map(|&b| Value::Int(i64::from(b))).collect(),
    ))
}

/// `from_bytes(xs)`: the string of a list of bytes (ints from 0 to 255),
/// UTF-8 or not: `bytes` backwards.
fn from_bytes_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    const WANT: &str = "a list of bytes (ints from 0 to 255)";
    let Value::List(items) = &a[0] else {
        return Err(nat::wrong_arg("xs", WANT, &a[0]));
    };
    let mut out = Vec::with_capacity(items.len());
    for v in items.iter() {
        match v {
            Value::Int(n @ 0..=255) => out.push(*n as u8),
            Value::Int(n) => {
                return Err(fail(
                    "E0403",
                    vec![
                        nat::s("xs"),
                        nat::s(WANT),
                        Arg::Str(format!("a list holding {n}")),
                    ],
                ));
            }
            v => {
                return Err(fail(
                    "E0403",
                    vec![
                        nat::s("xs"),
                        nat::s(WANT),
                        Arg::Str(format!("a list holding {}", crate::value::type_name(v))),
                    ],
                ));
            }
        }
    }
    Ok(string(out))
}

/// `valid_utf8(s)`: whether `s` is valid UTF-8 text, as the operations
/// that need text (JSON, patterns, URLs) require.
fn valid_utf8_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(Value::Bool(nat::need_text("s", &a[0])?.is_utf8()))
}
