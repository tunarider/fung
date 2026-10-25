//! `_native.regex` (06): regular expressions over fung strings. fung's
//! pattern syntax is the `regex` crate's, with Unicode classes; match
//! positions are character indexes (02: 문자열은 글자 단위), not byte
//! offsets.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock};

use fung_front::diag::Arg;
use regex::{Captures, Regex, RegexBuilder};

use super::nat::{self, native_step, param};
use crate::value::{MapBuilder, Module, Native, Value};
use crate::vm::{Flow, Stop, Task};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.regex` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            (
                "is_match",
                native_step("is_match", is_match_fn, vec![param("s"), param("pat")]),
            ),
            (
                "find",
                native_step("find", find_fn, vec![param("s"), param("pat")]),
            ),
            (
                "find_all",
                native_step("find_all", find_all_fn, vec![param("s"), param("pat")]),
            ),
            (
                "replace",
                native_step(
                    "replace",
                    replace_fn,
                    vec![param("s"), param("pat"), param("repl")],
                ),
            ),
            (
                "split",
                native_step("split", split_fn, vec![param("s"), param("pat")]),
            ),
        ]
    });
    let mut m = Module::new("regex", "_native.regex");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

/// A compiled pattern and its group names (`""` for an unnamed group).
struct Compiled {
    re: Regex,
    names: Vec<String>,
}

/// The most memory one compiled pattern may take (the crate's own default
/// is 10 MiB, too small for a Unicode class repeated a few hundred times:
/// `\w{400}`). Past it a pattern is E0515 "pattern too large".
const COMPILE_LIMIT: usize = 128 << 20;

/// Bounds the compiled-pattern cache; past it the cache is emptied rather
/// than evicted one by one (patterns are few and small).
const MAX_CACHED_PATTERNS: usize = 256;

/// The process-wide cache of compiled patterns. It holds plain Rust
/// values only (no fung value, so any thread may drop it), behind a mutex.
static CACHE: LazyLock<Mutex<HashMap<String, Arc<Compiled>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn cache() -> MutexGuard<'static, HashMap<String, Arc<Compiled>>> {
    // A panic while the lock was held cannot leave the map half-changed.
    CACHE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Runs `body` with the arguments, the subject, the pattern text and the
/// compiled pattern, cached. A bad pattern is a code error (E0515):
/// patterns are written in the script. Only good patterns are cached.
///
/// A cache miss compiles on a pool thread (`nat::offload_abandoning`:
/// a stop lets the compile go), as a file read does: a pattern that takes
/// seconds to compile must not stall every task. Two tasks that miss on the
/// same pattern at once both compile it (`remember` keeps the first).
///
/// Locks and threads, so that no two threads wait on each other:
/// - the cache mutex is held only inside `cache().get(..)` and `remember`,
///   short sections that wait for nothing (no park, no other lock), so a
///   thread holding it always lets it go;
/// - the compile running on a pool thread takes no lock and touches no
///   fung value: it gets an owned `String` and gives back a plain
///   `Compiled` (both `Send`, as an offload job requires);
/// - so while this task waits for the job it holds neither the cache
///   mutex nor an `Arc<Compiled>` from the cache.
fn with_pattern(
    t: &mut Task<'_>,
    a: Vec<Value>,
    body: fn(&[Value], &str, &str, &Compiled) -> Result<Value, Stop>,
) -> Result<Flow, Stop> {
    // The subject is checked first, then the pattern.
    let s = nat::need_str("s", &a[0])?;
    let pat = nat::need_str("pat", &a[1])?;
    // The guard is a temporary: it is dropped at the end of the statement.
    let hit = cache().get(pat).cloned();
    if let Some(c) = hit {
        return body(&a, s, pat, &c).map(Flow::Done);
    }
    let owned = pat.to_string();
    let finish = move |r: Result<Compiled, String>| {
        let compiled = r.map_err(|e| nat::fail("E0515", vec![Arg::Str(e)]))?;
        let (Value::Str(s), Value::Str(pat)) = (&a[0], &a[1]) else {
            unreachable!("checked before the compile")
        };
        let c = remember(&mut cache(), pat, Arc::new(compiled));
        body(&a, s, pat, &c)
    };
    // Inside a swap's function the compile runs here: the function must
    // not park, or another task could change the atom under it again and
    // again (`atom.rs`).
    if t.pure > 0 {
        return finish(compile(owned)).map(Flow::Done);
    }
    nat::offload_abandoning(t, move || compile(owned), move |_, r| finish(r))
}

/// The compiled pattern, from the cache or compiled here (tests).
#[cfg(test)]
fn compile_pattern(pat: &str) -> Arc<Compiled> {
    let hit = cache().get(pat).cloned();
    hit.unwrap_or_else(|| {
        let c = compile(pat.to_string()).unwrap_or_else(|e| panic!("{e}"));
        remember(&mut cache(), pat, Arc::new(c))
    })
}

/// Compiles `pat`, or says what is wrong with it: `` `pat`: reason ``
/// (E0515's argument). Runs on a pool thread: plain Rust values in
/// and out.
fn compile(pat: String) -> Result<Compiled, String> {
    if let Err(e) = regex_syntax::Parser::new().parse(&pat) {
        let reason = match &e {
            regex_syntax::Error::Parse(e) => e.kind().to_string(),
            regex_syntax::Error::Translate(e) => e.kind().to_string(),
            _ => "invalid pattern".to_string(),
        };
        return Err(format!("`{pat}`: {reason}"));
    }
    let re = match RegexBuilder::new(&pat).size_limit(COMPILE_LIMIT).build() {
        Ok(re) => re,
        Err(regex::Error::CompiledTooBig(_)) => return Err(format!("`{pat}`: pattern too large")),
        Err(_) => return Err(format!("`{pat}`: invalid pattern")),
    };
    #[cfg(test)]
    tests::compile_hook(&pat);
    let names = re
        .capture_names()
        .map(|n| n.unwrap_or_default().to_string())
        .collect();
    Ok(Compiled { names, re })
}

/// Adds `c` to `cache` as `pat` and returns what the cache holds for
/// `pat`: the entry already there if another task compiled it first.
/// A full cache is emptied first rather than evicted one by one.
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

/// Converts byte offsets of `s` to character indexes. Offsets
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

/// The keys of a match map and the `named` of a pattern without names,
/// made once per run thread and shared by every match (P11: a match was
/// a dozen allocations, most of them these).
struct Shared {
    text: Rc<String>,
    start: Rc<String>,
    end: Rc<String>,
    groups: Rc<String>,
    named: Rc<String>,
    no_names: Value,
}

thread_local! {
    static SHARED: Shared = {
        let key = |k: &str| Rc::new(k.to_string());
        Shared {
            text: key("text"),
            start: key("start"),
            end: key("end"),
            groups: key("groups"),
            named: key("named"),
            no_names: Value::Map(Rc::new(MapBuilder::default().finish())),
        }
    };
}

/// `{text, start, end, groups, named}` for one match: a group that did
/// not take part is nil.
fn match_value(c: &Compiled, caps: &Captures, cur: &mut RuneCursor) -> Value {
    SHARED.with(|k| {
        let whole = caps.get(0).expect("the whole match");
        let mut m = MapBuilder::with_capacity(5);
        m.set(k.text.clone(), Value::str(whole.as_str()));
        m.set(k.start.clone(), Value::Int(cur.index(whole.start())));
        m.set(k.end.clone(), Value::Int(cur.index(whole.end())));
        let mut groups = Vec::with_capacity(caps.len().saturating_sub(1));
        let mut named = None;
        for i in 1..caps.len() {
            let g = caps.get(i).map_or(Value::Nil, |g| Value::str(g.as_str()));
            if !c.names[i].is_empty() {
                named
                    .get_or_insert_with(MapBuilder::default)
                    .set(Rc::new(c.names[i].clone()), g.clone());
            }
            groups.push(g);
        }
        m.set(k.groups.clone(), nat::list(groups));
        let named = match named {
            Some(b) => Value::Map(Rc::new(b.finish())),
            None => k.no_names.clone(),
        };
        m.set(k.named.clone(), named);
        Value::Map(Rc::new(m.finish()))
    })
}

fn is_match_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    with_pattern(t, a, |_, s, _, c| Ok(Value::Bool(c.re.is_match(s))))
}

fn find_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    with_pattern(t, a, |_, s, _, c| {
        Ok(match c.re.captures(s) {
            Some(caps) => match_value(c, &caps, &mut RuneCursor::new(s)),
            None => Value::Nil,
        })
    })
}

/// Every match left to right; an empty match right after the previous
/// match is not one (06: 정규식).
fn find_all_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    with_pattern(t, a, |_, s, _, c| {
        let mut cur = RuneCursor::new(s);
        let out =
            c.re.captures_iter(s)
                .map(|caps| match_value(c, &caps, &mut cur))
                .collect();
        Ok(nat::list(out))
    })
}

/// Every match replaced by `repl`, where `$1`, `${name}` and `$$` refer to
/// groups (`$1a` is the group named "1a"; a missing group is empty).
fn replace_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    with_pattern(t, a, |a, s, _, c| {
        let repl = nat::need_str("repl", &a[2])?;
        Ok(Value::str(&c.re.replace_all(s, repl)))
    })
}

/// The pieces between matches, empty ones kept, like `split(s, sep)`
/// (06): an empty match at the very start or end of `s` cuts nothing, and
/// the empty pattern on the empty string gives no pieces.
fn split_fn(t: &mut Task<'_>, a: Vec<Value>) -> Result<Flow, Stop> {
    with_pattern(t, a, |_, s, pat, c| {
        // Nothing split by nothing is no pieces, as `split("", "")`.
        if s.is_empty() && pat.is_empty() {
            return Ok(nat::list(Vec::new()));
        }
        let mut out = Vec::new();
        let mut beg = 0;
        for m in c.re.find_iter(s) {
            if m.is_empty() && (m.start() == 0 || m.start() == s.len()) {
                continue;
            }
            out.push(Value::str(&s[beg..m.start()]));
            beg = m.end();
        }
        out.push(Value::str(&s[beg..]));
        Ok(nat::list(out))
    })
}

#[cfg(test)]
#[path = "regex_tests.rs"]
mod tests;
