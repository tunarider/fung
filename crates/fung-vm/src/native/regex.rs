//! `_native.regex` (Go `internal/native/regex.go`, 06): regular
//! expressions over fung strings. Patterns follow Go's syntax and meaning
//! (`regex_go::compile_go`); match positions are rune indexes (02: 문자열은
//! 글자 단위), not byte offsets.
//!
//! Go's `regexp` decides more than the engine does: which matches
//! `FindAll` reports around empty matches (`allMatches`), where `Split`
//! cuts, which matches `ReplaceAllString` replaces, and how `Expand` reads
//! `$` references. Those loops are ported here step by step (open-questions
//! "Go 동등을 위한 과도기 잔가지").

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock};

use fung_front::diag::Arg;
use regex::{CaptureLocations, Regex};

use super::nat::{self, native, param};
use super::regex_go::{compile_go, go_capture_names, go_letter_or_digit};
use crate::value::{MapBuilder, Module, Native, Value};
use crate::vm::{Stop, Task, outside};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.regex` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            (
                "is_match",
                native("is_match", is_match_fn, vec![param("s"), param("pat")]),
            ),
            (
                "find",
                native("find", find_fn, vec![param("s"), param("pat")]),
            ),
            (
                "find_all",
                native("find_all", find_all_fn, vec![param("s"), param("pat")]),
            ),
            (
                "replace",
                native(
                    "replace",
                    replace_fn,
                    vec![param("s"), param("pat"), param("repl")],
                ),
            ),
            (
                "split",
                native("split", split_fn, vec![param("s"), param("pat")]),
            ),
        ]
    });
    let mut m = Module::new("regex", "_native.regex");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

/// A compiled pattern with Go's `SubexpNames()`.
struct Compiled {
    re: Regex,
    names: Vec<String>,
}

/// Bounds the compiled-pattern cache; past it the cache is emptied rather
/// than evicted one by one (patterns are few and small), as in Go.
const MAX_CACHED_PATTERNS: usize = 256;

/// The process-wide cache of compiled patterns. It holds plain Rust
/// values only (no fung value, so any thread may drop it), like Go's
/// `patternCache` behind its mutex.
static CACHE: LazyLock<Mutex<HashMap<String, Arc<Compiled>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn cache() -> MutexGuard<'static, HashMap<String, Arc<Compiled>>> {
    // A panic while the lock was held cannot leave the map half-changed.
    CACHE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The compiled pattern, cached. A bad pattern is a code error (E0515):
/// patterns are written in the script. Only good patterns are cached.
///
/// A cache miss compiles without the run token (`outside`), as a file read
/// does: a pattern that takes seconds to compile must not stall every
/// task. Go compiles under its cache mutex, so there only other regex
/// callers wait; here nobody does, and two tasks that miss on the same
/// pattern at once both compile it (`remember` keeps the first).
///
/// Locks and the token, so that no two threads wait on each other:
/// - the cache mutex is held only inside `cache().get(..)` and `remember`,
///   short sections that wait for nothing (no token, no other lock), so a
///   thread holding it always lets it go;
/// - the compile running without the token takes no lock and touches no
///   fung value: it gets an owned `String` and gives back a plain
///   `Compiled` (both `Send`, as `outside` requires);
/// - so while this task waits for the token (`outside` taking it back) it
///   holds neither the cache mutex nor an `Arc<Compiled>` from the cache.
fn compile_pattern(t: &mut Task<'_>, pat: &str) -> Result<Arc<Compiled>, Stop> {
    // The guard is a temporary: it is dropped at the end of the statement.
    let hit = cache().get(pat).cloned();
    if let Some(c) = hit {
        return Ok(c);
    }
    let owned = pat.to_string();
    let compiled =
        outside(t, move || compile(owned)).map_err(|e| nat::fail("E0515", vec![Arg::Str(e)]))?;
    Ok(remember(&mut cache(), pat, Arc::new(compiled)))
}

/// Compiles `pat` with Go's syntax and meaning. Runs without the run
/// token: plain Rust values in and out.
fn compile(pat: String) -> Result<Compiled, String> {
    let re = compile_go(&pat)?;
    #[cfg(test)]
    tests::compile_hook(&pat);
    Ok(Compiled {
        names: go_capture_names(&re),
        re,
    })
}

/// Adds `c` to `cache` as `pat` and returns what the cache holds for
/// `pat`: the entry already there if another task compiled it first.
/// A full cache is emptied first rather than evicted one by one (Go).
fn remember(
    cache: &mut HashMap<String, Arc<Compiled>>,
    pat: &str,
    c: Arc<Compiled>,
) -> Arc<Compiled> {
    if let Some(first) = cache.get(pat) {
        return first.clone();
    }
    if cache.len() >= MAX_CACHED_PATTERNS {
        cache.clear();
    }
    cache.insert(pat.to_string(), c.clone());
    c
}

/// Reads the `(s, pat)` arguments every function takes: the subject is
/// checked first, then the pattern, which is then compiled. The pattern
/// text comes back too (`split` needs it).
fn subject_and_pattern<'a>(
    t: &mut Task<'_>,
    a: &'a [Value],
) -> Result<(&'a str, &'a str, Arc<Compiled>), Stop> {
    let s = nat::need_str("s", &a[0])?;
    let pat = nat::need_str("pat", &a[1])?;
    Ok((s, pat, compile_pattern(t, pat)?))
}

/// The length of the character at `pos`, 0 at the end (Go's
/// `inputString.step` width).
fn width_at(s: &str, pos: usize) -> usize {
    s[pos..].chars().next().map_or(0, char::len_utf8)
}

/// Go `Regexp.FindAllStringSubmatchIndex(s, -1)` (`allMatches`): each
/// search starts where the last match ended, an empty match is followed by
/// a one-character step, and an empty match right where the previous match
/// ended is not reported. `each` gets every reported match's groups.
pub(crate) fn go_all(re: &Regex, s: &str, mut each: impl FnMut(&CaptureLocations)) {
    let mut locs = re.capture_locations();
    let mut pos = 0;
    let mut prev_end = None;
    while pos <= s.len() {
        let Some(m) = re.captures_read_at(&mut locs, s, pos) else {
            break;
        };
        let mut accept = true;
        if m.end() == pos {
            if Some(m.start()) == prev_end {
                accept = false;
            }
            match width_at(s, pos) {
                0 => pos = s.len() + 1,
                w => pos += w,
            }
        } else {
            pos = m.end();
        }
        prev_end = Some(m.end());
        if accept {
            each(&locs);
        }
    }
}

/// Converts byte offsets of `s` to rune indexes (Go `runeCursor`). Offsets
/// that arrive in non-decreasing order (match after match) cost only the
/// text between them, so `find_all` stays linear in the subject.
struct RuneCursor<'a> {
    s: &'a str,
    /// Bytes counted so far.
    b: usize,
    /// Runes in `s[..b]`.
    r: usize,
}

impl<'a> RuneCursor<'a> {
    fn new(s: &'a str) -> Self {
        RuneCursor { s, b: 0, r: 0 }
    }

    fn index(&mut self, byte_off: usize) -> i64 {
        if byte_off < self.b {
            (self.b, self.r) = (0, 0);
        }
        self.r += self.s[self.b..byte_off].chars().count();
        self.b = byte_off;
        self.r as i64
    }
}

fn key(k: &str) -> Rc<String> {
    Rc::new(k.to_string())
}

/// `{text, start, end, groups, named}` for one match: a group that did
/// not take part is nil, and of two groups with one name the later wins.
fn match_value(s: &str, c: &Compiled, locs: &CaptureLocations, cur: &mut RuneCursor) -> Value {
    let (start, end) = locs.get(0).unwrap_or((0, 0));
    let mut m = MapBuilder::default();
    m.set(key("text"), Value::str(&s[start..end]));
    m.set(key("start"), Value::Int(cur.index(start)));
    m.set(key("end"), Value::Int(cur.index(end)));
    let mut groups = Vec::with_capacity(locs.len().saturating_sub(1));
    let mut named = MapBuilder::default();
    for i in 1..locs.len() {
        let g = match locs.get(i) {
            Some((a, b)) => Value::str(&s[a..b]),
            None => Value::Nil,
        };
        if !c.names[i].is_empty() {
            named.set(key(&c.names[i]), g.clone());
        }
        groups.push(g);
    }
    m.set(key("groups"), nat::list(groups));
    m.set(key("named"), Value::Map(Rc::new(named.finish())));
    Value::Map(Rc::new(m.finish()))
}

fn is_match_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let (s, _, c) = subject_and_pattern(t, &a)?;
    Ok(Value::Bool(c.re.is_match(s)))
}

fn find_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let (s, _, c) = subject_and_pattern(t, &a)?;
    let mut locs = c.re.capture_locations();
    Ok(match c.re.captures_read(&mut locs, s) {
        Some(_) => match_value(s, &c, &locs, &mut RuneCursor::new(s)),
        None => Value::Nil,
    })
}

fn find_all_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let (s, _, c) = subject_and_pattern(t, &a)?;
    let mut out = Vec::new();
    let mut cur = RuneCursor::new(s);
    go_all(&c.re, s, |locs| {
        out.push(match_value(s, &c, locs, &mut cur))
    });
    Ok(nat::list(out))
}

/// Go `Regexp.ReplaceAllString` (`replaceAll`): its own loop, where an
/// empty match right after the previous match is not replaced and the
/// search always moves on at least one character.
fn replace_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let (s, _, c) = subject_and_pattern(t, &a)?;
    let repl = nat::need_str("repl", &a[2])?;
    let mut out = String::with_capacity(s.len());
    let mut locs = c.re.capture_locations();
    let (mut last_end, mut search) = (0, 0);
    while search <= s.len() {
        let Some(m) = c.re.captures_read_at(&mut locs, s, search) else {
            break;
        };
        let (start, end) = (m.start(), m.end());
        out.push_str(&s[last_end..start]);
        if end > last_end || start == 0 {
            expand(&mut out, repl, &c.names, s, &locs);
        }
        last_end = end;
        let width = width_at(s, search);
        if search + width > end {
            search += width;
        } else if search + 1 > end {
            search += 1;
        } else {
            search = end;
        }
    }
    out.push_str(&s[last_end..]);
    Ok(Value::str(&out))
}

/// Go `Regexp.Expand` for `ReplaceAllString`: `$$` is `$`, `$name` and
/// `${name}` refer to a group by number or name, and a malformed
/// reference keeps its `$` as text. An unknown or unmatched group gives
/// nothing.
fn expand(
    out: &mut String,
    mut template: &str,
    names: &[String],
    s: &str,
    locs: &CaptureLocations,
) {
    while let Some((before, after)) = template.split_once('$') {
        out.push_str(before);
        template = after;
        if let Some(rest) = template.strip_prefix('$') {
            out.push('$');
            template = rest;
            continue;
        }
        let Some((name, num, rest)) = extract(template) else {
            out.push('$');
            continue;
        };
        template = rest;
        let group = match num {
            Some(n) => locs.get(n),
            None => (0..names.len())
                .filter(|&i| names[i] == name)
                .find_map(|i| locs.get(i)),
        };
        if let Some((a, b)) = group {
            out.push_str(&s[a..b]);
        }
    }
    out.push_str(template);
}

/// Go `extract`: the name of a leading `name` or `{name}` (letters,
/// digits and `_`), the group number when the name is one (no leading
/// zero, below Go's 1e8 cut-off), and the rest of the template.
fn extract(t: &str) -> Option<(&str, Option<usize>, &str)> {
    let (brace, body) = match t.strip_prefix('{') {
        Some(b) => (true, b),
        None => (false, t),
    };
    let len = body
        .char_indices()
        .find(|&(_, c)| !go_letter_or_digit(c) && c != '_')
        .map_or(body.len(), |(i, _)| i);
    if len == 0 {
        return None;
    }
    let name = &body[..len];
    let rest = if brace {
        body[len..].strip_prefix('}')?
    } else {
        &body[len..]
    };
    let mut num: Option<usize> = Some(0);
    for d in name.bytes() {
        num = match num {
            Some(n) if d.is_ascii_digit() && n < 100_000_000 => {
                Some(n * 10 + usize::from(d - b'0'))
            }
            _ => None,
        };
        if num.is_none() {
            break;
        }
    }
    if name.starts_with('0') && name.len() > 1 {
        num = None;
    }
    Some((name, num, rest))
}

/// Go `Regexp.Split(s, -1)`: an empty pattern on an empty subject gives
/// no pieces, an empty match at the start cuts nothing, and the text after
/// the last match is kept unless that match started at the very end.
fn split_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let (s, pat, c) = subject_and_pattern(t, &a)?;
    if !pat.is_empty() && s.is_empty() {
        return Ok(nat::list(vec![Value::str("")]));
    }
    let mut out = Vec::new();
    let (mut beg, mut end) = (0, 0);
    go_all(&c.re, s, |locs| {
        let (start, stop) = locs.get(0).unwrap_or((0, 0));
        end = start;
        if stop != 0 {
            out.push(Value::str(&s[beg..end]));
        }
        beg = stop;
    });
    if end != s.len() {
        out.push(Value::str(&s[beg..]));
    }
    Ok(nat::list(out))
}

#[cfg(test)]
#[path = "regex_tests.rs"]
mod tests;
