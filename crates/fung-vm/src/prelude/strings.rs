//! The prelude's string functions. Strings are bytes read as UTF-8
//! characters (02, L4); `upper` and `lower` use the full Unicode case
//! mapping.

use fung_front::diag::Arg;

use crate::native::nat::{self, def, def_step, fail, opt, param};
use crate::native::walk::{Collect, Cursor, drain_with};
use fung_front::text::{Unit, char_count, units};

use std::rc::Rc;

use crate::value::{Args, List, Lit, Text, Value, write_for_print};
use crate::vm::pieces::{self, STEP_BYTES, STEP_ITEMS, STEP_SCAN, Work};
use crate::vm::{Flow, Stop, Task};

pub(super) fn add(add: &mut impl FnMut(Value)) {
    add(def_step("split", split_fn, vec![param("s"), param("sep")]));
    add(def_step("lines", lines_fn, vec![param("s")]));
    add(def_step(
        "join",
        join_fn,
        vec![param("xs"), opt("sep", Lit::Str(""))],
    ));
    add(def_step(
        "trim",
        trim_fn,
        vec![param("s"), opt("cut", Lit::Nil)],
    ));
    add(def_step(
        "trim_left",
        trim_left_fn,
        vec![param("s"), opt("cut", Lit::Nil)],
    ));
    add(def_step(
        "trim_right",
        trim_right_fn,
        vec![param("s"), opt("cut", Lit::Nil)],
    ));
    add(def_step(
        "index_of",
        index_of_fn,
        vec![param("s"), param("sub")],
    ));
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
    add(def_step("bytes", bytes_fn, vec![param("s")]));
    add(def_step("from_bytes", from_bytes_fn, vec![param("xs")]));
    add(def("is_utf8", is_utf8_fn, vec![param("s")]));
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

/// `split(s, sep)`: the parts between the separators, one more than
/// there are separators. An empty separator is E0403 (VA20, D18): the
/// characters are `chars`.
fn split_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let [s, sep] = nat::bytes_args(&a, ["s", "sep"])?;
    if sep.is_empty() {
        return Err(nat::not_nonempty("sep", &a[1]));
    }
    let cuts = Cuts::new(a[0].clone(), s.len(), sep);
    pieces::run(t, Parts::new(cuts, |p| p))
}

/// The characters of the string `s` as a list (`chars`), a piece at a
/// time.
pub(super) fn chars_of(t: &mut Task<'_>, s: Value) -> Result<Flow, Stop> {
    struct Chars {
        s: Value,
        at: usize,
        out: List,
    }
    impl Work for Chars {
        fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            let out = &mut self.out;
            let done = chars_from(bytes_of(&self.s), &mut self.at, |c| {
                out.push(Value::char_str(c))
            });
            nat::check_len(self.out.len() as u64)?;
            if !done {
                return Ok(None);
            }
            let out = std::mem::replace(&mut self.out, List::new(Vec::new()));
            Ok(Some(Value::List(Rc::new(out))))
        }
    }
    pieces::run(
        t,
        Chars {
            s,
            at: 0,
            out: List::new(Vec::new()),
        },
    )
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
        let done = self.cuts.run(|p, _| out.push(string(part(p).to_vec())));
        // The list only grows: past the limit at the end of a step is past
        // it for good (VA71).
        nat::check_len(self.out.len() as u64)?;
        if !done {
            return Ok(None);
        }
        let out = std::mem::replace(&mut self.out, List::new(Vec::new()));
        Ok(Some(Value::List(Rc::new(out))))
    }
}

/// `lines(s)`: `s` split on `\n`, one `\r` that ends a line dropped, and
/// no empty line after a final newline; the empty string has no lines (06,
/// the rule of `read_lines`).
fn lines_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
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
fn join_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
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
            if let Some(d) = crate::vm::not_text(&x, "`join`") {
                return Err(Stop::Error(Box::new(d)));
            }
            write_for_print(&x, &mut self.out)?;
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
/// only when `cut` holds it. From the front to the first kept character,
/// then from the back to the last (VA5), a piece at a time (VA10): the
/// time is what is cut off, and nothing is collected.
fn trim_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    trim_ends(t, a, true, true)
}

/// `trim_left(s, cut = nil)`: `trim` at the front only (D73).
fn trim_left_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    trim_ends(t, a, true, false)
}

/// `trim_right(s, cut = nil)`: `trim` at the back only (D73).
fn trim_right_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    trim_ends(t, a, false, true)
}

/// `trim` at the front, the back or both.
fn trim_ends(t: &mut Task<'_>, a: Args, front: bool, back: bool) -> Result<Flow, Stop> {
    nat::need_bytes("s", &a[0])?;
    // An empty `cut` would cut nothing: E0403 (D74), as `pad`'s.
    let cut: Vec<Unit> = match a[1] {
        Value::Nil => Vec::new(),
        _ if nat::need_bytes("cut", &a[1])?.is_empty() => {
            return Err(nat::not_nonempty("cut", &a[1]));
        }
        _ => units(nat::need_bytes("cut", &a[1])?)
            .map(|(_, u)| u)
            .collect(),
    };
    struct Trim {
        s: Value,
        cut: Vec<Unit>,
        space: bool,
        /// The first kept character, once found; the search is at `at`.
        from: Option<usize>,
        at: usize,
        /// The kept part ends before `to`.
        to: usize,
        /// Whether the back is cut too.
        back: bool,
    }
    impl Trim {
        fn gone(&self, u: Unit) -> bool {
            match u {
                Unit::Char(c) if self.space => c.is_whitespace(),
                u => self.cut.contains(&u),
            }
        }
    }
    impl Work for Trim {
        fn step(&mut self, _t: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            let s = bytes_of(&self.s);
            let Some(from) = self.from else {
                let stop = (self.at + STEP_BYTES).min(s.len());
                while self.at < stop {
                    let u = fung_front::text::first(&s[self.at..]).expect("not at the end");
                    if !self.gone(u) {
                        if !self.back {
                            return Ok(Some(string(s[self.at..].to_vec())));
                        }
                        // On from the back in the same step: a short
                        // string is one step.
                        self.from = Some(self.at);
                        return self.step(_t);
                    }
                    self.at += u.byte_len();
                }
                if self.at < s.len() {
                    return Ok(None);
                }
                return Ok(Some(string(Vec::new())));
            };
            let stop = self.to.saturating_sub(STEP_BYTES).max(from);
            while self.to > stop {
                let u = fung_front::text::last(&s[from..self.to]).expect("from is kept");
                if !self.gone(u) {
                    if from == 0 && self.to == s.len() {
                        return Ok(Some(self.s.clone()));
                    }
                    return Ok(Some(string(s[from..self.to].to_vec())));
                }
                self.to -= u.byte_len();
            }
            // Cut to nothing from the back alone (`trim_right`).
            if self.to == from {
                return Ok(Some(string(Vec::new())));
            }
            Ok(None)
        }
    }
    let len = bytes_of(&a[0]).len();
    pieces::run(
        t,
        Trim {
            space: matches!(a[1], Value::Nil),
            s: a[0].clone(),
            cut,
            // the front is not cut: the kept part starts at 0
            from: (!front).then_some(0),
            at: 0,
            to: len,
            back,
        },
    )
}

/// `index_of(s, sub)`: the character index of the first occurrence of
/// `sub` in `s` (as `s[i]` and `len` count, 02), or `nil` (D73). It finds
/// what `sub in s` finds (D107): bytes, so an occurrence that starts on a
/// byte inside a character is found at that character's index. An empty
/// `sub` is everywhere: E0403, as `split`'s `sep`. A piece at a time
/// (L6): `STEP_SCAN` bytes are searched each time, and the characters
/// before the search position counted a block at a time (VA70); the
/// search goes on from where it stopped, whatever character that byte is
/// in (VA62).
fn index_of_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    nat::need_bytes("s", &a[0])?;
    if nat::need_bytes("sub", &a[1])?.is_empty() {
        return Err(nat::not_nonempty("sub", &a[1]));
    }
    struct Find {
        s: Value,
        finder: memchr::memmem::Finder<'static>,
        /// Searched up to here (any byte).
        from: usize,
        /// Counted up to `at` (a character boundary, at most `from`),
        /// `chars` characters.
        at: usize,
        chars: usize,
    }
    /// Counts the characters from `at` up to the start of the one that
    /// holds byte `to`. Valid UTF-8 (`text`) is counted by `str`'s own
    /// counter, a word at a time, without decoding (VA68: a search of
    /// non-ASCII text that found nothing spent 20 times as long counting
    /// as searching).
    fn count_to(s: &[u8], text: Option<&str>, at: &mut usize, chars: &mut usize, to: usize) {
        let b = char_start(s, *at, to);
        *chars += match text {
            Some(t) => t[*at..b].chars().count(),
            None => fung_front::text::char_count(&s[*at..b]),
        };
        *at = b;
    }
    impl Work for Find {
        fn step(&mut self, _t: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            let s = bytes_of(&self.s);
            let text = match &self.s {
                Value::Str(t) => t.as_str(),
                _ => None,
            };
            let n = self.finder.needle().len();
            let stop = (self.from + STEP_SCAN).min(s.len());
            let window = (stop + n - 1).min(s.len());
            let found = self
                .finder
                .find(&s[self.from..window])
                .map(|i| self.from + i);
            match found {
                // at the index of the character the match starts in
                Some(m) => {
                    count_to(s, text, &mut self.at, &mut self.chars, m);
                    Ok(Some(Value::Int(self.chars as i64)))
                }
                None if window == s.len() => Ok(Some(Value::Nil)),
                None => {
                    count_to(s, text, &mut self.at, &mut self.chars, stop);
                    self.from = stop;
                    Ok(None)
                }
            }
        }
    }
    let sub = nat::need_bytes("sub", &a[1])?;
    pieces::run(
        t,
        Find {
            s: a[0].clone(),
            finder: memchr::memmem::Finder::new(sub).into_owned(),
            from: 0,
            at: 0,
            chars: 0,
        },
    )
}

/// Where the character that holds byte `to` of `s` starts (`to` itself
/// at the end): no earlier than `at`, a character boundary. Every byte
/// that is no continuation byte starts a character (a valid one, or one
/// invalid byte), and a valid character has at most three continuation
/// bytes; a continuation byte with no lead within three bytes before it
/// is a character of its own.
fn char_start(s: &[u8], at: usize, to: usize) -> usize {
    if to >= s.len() {
        return s.len();
    }
    for q in (at.max(to.saturating_sub(3))..=to).rev() {
        if (s[q] as i8) >= -0x40 {
            let len = fung_front::text::first(&s[q..])
                .expect("not at the end")
                .byte_len();
            return if q + len > to { q } else { to };
        }
    }
    to
}

/// `upper(s)` and `lower(s)`: the full Unicode case mapping (06), so the
/// length may change (`ß` → `SS`) and a final sigma lowercases to `ς`.
/// Bytes outside UTF-8 are no letters and stay as they are (L4).
fn upper_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let s = nat::need_text("s", &a[0])?;
    Ok(string(map_text(s, str::to_uppercase)))
}

fn lower_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
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

/// `replace(s, old, new)`: every occurrence. An empty `old` is E0403
/// (D74: it would match between every two characters), as `split`'s
/// empty separator. A result past `MAX_CHARS` characters is E0806
/// before anything is built (02). Built a piece at a time (L6).
fn replace_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let [s, old, new] = nat::bytes_args(&a, ["s", "old", "new"])?;
    if old.is_empty() {
        return Err(nat::not_nonempty("old", &a[1]));
    }
    if !replace_fits(s, old, new) {
        return Err(too_many_chars());
    }
    let new = new.to_vec();
    let out = Vec::with_capacity(s.len());
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
    let most = len / old_len;
    if len + most * (new_len - old_len).max(0) <= limit {
        return true;
    }
    let n = memchr::memmem::find_iter(s, old).count() as i128;
    if len + n * (new_len - old_len) <= limit {
        return true;
    }
    let chars = |x: &[u8]| char_count(x) as i128;
    chars(s) + n * (chars(new) - chars(old)) <= limit
}

/// `starts_with(s, prefix)`. An empty `prefix` is E0403, as every text
/// to search for (D118).
fn starts_with_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let [s, prefix] = nat::bytes_args(&a, ["s", "prefix"])?;
    if prefix.is_empty() {
        return Err(nat::not_nonempty("prefix", &a[1]));
    }
    Ok(Value::Bool(s.starts_with(prefix)))
}

/// `ends_with(s, suffix)`, as `starts_with`.
fn ends_with_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    let [s, suffix] = nat::bytes_args(&a, ["s", "suffix"])?;
    if suffix.is_empty() {
        return Err(nat::not_nonempty("suffix", &a[1]));
    }
    Ok(Value::Bool(s.ends_with(suffix)))
}

/// Bounds the strings `repeat` (and fung's `pad_*`) may build (E0806).
pub(crate) const MAX_CHARS: i64 = 100_000_000;

/// E0806 for a string past [`MAX_CHARS`].
pub(crate) fn too_many_chars() -> Stop {
    fail("E0806", vec![nat::s("characters"), Arg::Int(MAX_CHARS)])
}

/// `repeat(s, n)`; a negative `n` is E0403 (SP15). A result past `MAX_CHARS`
/// characters is E0806 (02). Written a piece at a time (L6).
fn repeat_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    let s = nat::need_bytes("s", &a[0])?;
    let n = nat::need_count("n", &a[1])? as i64;
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
fn byte_len_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    Ok(Value::Int(nat::need_bytes("s", &a[0])?.len() as i64))
}

/// `bytes(s)`: the bytes of `s` as ints from 0 to 255, a piece at a
/// time (VA10).
fn bytes_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    // A string of the most characters has up to four times the bytes (D63).
    nat::check_len(nat::need_bytes("s", &a[0])?.len() as u64)?;
    struct Bytes {
        s: Value,
        at: usize,
        out: List,
    }
    impl Work for Bytes {
        fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            let s = bytes_of(&self.s);
            let end = (self.at + STEP_BYTES).min(s.len());
            for &b in &s[self.at..end] {
                self.out.push(Value::Int(i64::from(b)));
            }
            self.at = end;
            if end < s.len() {
                return Ok(None);
            }
            let out = std::mem::replace(&mut self.out, List::new(Vec::new()));
            Ok(Some(Value::List(Rc::new(out))))
        }
    }
    pieces::run(
        t,
        Bytes {
            s: a[0].clone(),
            at: 0,
            out: List::new(Vec::new()),
        },
    )
}

/// `from_bytes(xs)`: the string of a list of bytes (ints from 0 to 255),
/// UTF-8 or not: `bytes` backwards. Read a piece at a time (VA10).
fn from_bytes_fn(t: &mut Task<'_>, a: Args) -> Result<Flow, Stop> {
    const WANT: &str = "a list of bytes (ints from 0 to 255)";
    let Value::List(items) = &a[0] else {
        return Err(nat::wrong_arg("xs", WANT, &a[0]));
    };
    struct FromBytes {
        items: Rc<List>,
        out: Vec<u8>,
    }
    impl Work for FromBytes {
        fn step(&mut self, _: &mut Task<'_>) -> Result<Option<Value>, Stop> {
            let end = (self.out.len() + STEP_BYTES).min(self.items.len());
            while self.out.len() < end {
                let run = self.items.run_from(self.out.len());
                let take = run.len().min(end - self.out.len());
                for v in &run[..take] {
                    let held = match v {
                        Value::Int(n @ 0..=255) => {
                            self.out.push(*n as u8);
                            continue;
                        }
                        Value::Int(n) => n.to_string(),
                        v => crate::value::type_name(v),
                    };
                    return Err(fail(
                        "E0403",
                        vec![
                            nat::s("xs"),
                            nat::s(WANT),
                            Arg::Str(format!("a list holding {held}")),
                        ],
                    ));
                }
            }
            if self.out.len() < self.items.len() {
                return Ok(None);
            }
            Ok(Some(string(std::mem::take(&mut self.out))))
        }
    }
    pieces::run(
        t,
        FromBytes {
            out: Vec::with_capacity(items.len()),
            items: items.clone(),
        },
    )
}

/// `is_utf8(s)`: whether `s` is valid UTF-8 text, as the operations
/// that need text (JSON, patterns, URLs) require.
fn is_utf8_fn(_: &mut Task<'_>, a: Args) -> Result<Value, Stop> {
    Ok(Value::Bool(nat::need_text("s", &a[0])?.is_utf8()))
}
