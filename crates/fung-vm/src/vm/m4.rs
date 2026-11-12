//! The M4 instructions: building collections, indexing, slicing, `in`, the
//! result operators `!` `?` `??`, and the pattern instructions.

use std::rc::Rc;

use fung_front::bytecode::Op;
use fung_front::diag::{Arg, Diag, error};
use fung_front::source::Pos;

use super::matcher::Binder;
use super::{Stop, Task, at, err, no_pos};
use crate::value::{
    Items, List, MapBuilder, Range, Sym, Tag, Tuple, Value, format_start, index, type_name,
};

/// How `!`, `?` and `??` see a value (02): an Ok
/// payload, an Err, nil, or something else. `!` and `?` reject nil
/// (E0604, SP13) and anything else (E0605); `??` replaces nil.
pub(crate) enum ResultKind {
    Ok(Value),
    Err,
    Nil,
    Other,
}

/// The one judgement of what a result is (02, SP12). `Ok` and `Err` hold
/// exactly one value: the compiler refuses another count (E0236) and the
/// runtime builds them only with one, so the name decides. That is an
/// invariant, not a type (VM36): a native that built `Ok` with no value
/// would panic on `t.args[0]` here (E0801).
pub(crate) fn result_kind(v: &Value) -> ResultKind {
    match v {
        Value::Tag(t) => match t.name {
            Sym::OK => ResultKind::Ok(t.args[0].clone()),
            Sym::ERR => ResultKind::Err,
            _ => ResultKind::Other,
        },
        Value::Nil => ResultKind::Nil,
        _ => ResultKind::Other,
    }
}

/// E0519 (without a position) when `v` holds, at any depth, a value that
/// is not put into text silently (SP2, D46): a result, `nil`, or a handle
/// (a function, module, task, scope, channel end, atom, file or socket).
/// `place` is where the text goes (`string interpolation`, `` `join` ``).
/// Walked with an explicit stack; a part shared through an `Rc` is
/// looked at once, so the walk is linear in the value's distinct parts.
pub(crate) fn not_text(v: &Value, place: &str) -> Option<Diag> {
    let refused = |v: &Value| -> Option<String> {
        Some(match v {
            Value::Nil => "`nil`".to_string(),
            Value::Tag(t) if t.name == Sym::OK || t.name == Sym::ERR => {
                format!("an `{}` value", t.name.name())
            }
            Value::Closure(_) | Value::Native(_) => "a function".to_string(),
            Value::Module(_)
            | Value::Task(_)
            | Value::Scope(_)
            | Value::Atom(_)
            | Value::Sender(_)
            | Value::Receiver(_)
            | Value::File(_)
            | Value::Conn(_)
            | Value::Listener(_) => format!("a {} handle", type_name(v)),
            _ => return None,
        })
    };
    // A string or a number, the common part, needs no walk.
    if !matches!(
        v,
        Value::List(_) | Value::Map(_) | Value::Tuple(_) | Value::Tag(_)
    ) {
        let what = refused(v)?;
        return Some(error("E0519", no_pos(), vec![Arg::Str(what), s(place)]));
    }
    let mut stack: Vec<(&Value, bool)> = vec![(v, false)];
    let mut seen: std::collections::HashSet<*const ()> = std::collections::HashSet::new();
    while let Some((v, inside)) = stack.pop() {
        if let Some(what) = refused(v) {
            let mut d = error("E0519", no_pos(), vec![Arg::Str(what), s(place)]);
            if inside {
                d.notes
                    .push("note: it is inside the value put into text".to_string());
            }
            return Some(d);
        }
        let ptr: *const () = match v {
            Value::List(rc) => Rc::as_ptr(rc).cast(),
            Value::Map(rc) => Rc::as_ptr(rc).cast(),
            Value::Tuple(rc) => Rc::as_ptr(rc).cast(),
            Value::Tag(rc) => Rc::as_ptr(rc).cast(),
            _ => continue,
        };
        if !seen.insert(ptr) {
            continue;
        }
        match v {
            Value::List(l) => stack.extend(l.iter().map(|x| (x, true))),
            Value::Map(m) => stack.extend(m.iter().map(|(_, x)| (x, true))),
            Value::Tuple(t) => stack.extend(t.iter().map(|x| (x, true))),
            Value::Tag(t) => stack.extend(t.args.iter().map(|x| (x, true))),
            _ => {}
        }
    }
    None
}

/// How E0605 names a value that is not a result: by its type.
pub(crate) fn not_result(v: &Value) -> String {
    format!("a value of type {}", type_name(v))
}

/// Renders an Err for E0603 (02): a payload that is a map with a string
/// `message` shows that; a payload of another shape shows its literal form.
fn err_message(e: &Value) -> String {
    let Value::Tag(t) = e else {
        return format_start(e, SHOWN_BYTES);
    };
    let v = &t.args[0];
    if let Value::Map(m) = v
        && let Some(Value::Str(s)) = m.get("message")
    {
        return fung_front::text::lossy(&s[..s.len().min(SHOWN_BYTES)]).into_owned();
    }
    format_start(v, SHOWN_BYTES)
}

fn s(x: &str) -> Arg {
    Arg::Str(x.to_string())
}

/// How much of a value a pattern or `!` error shows in its one-line
/// headline (08): a value can print megabytes.
const SHOWN_CHARS: usize = 200;

/// How much of a value's text is made to show `SHOWN_CHARS` characters
/// of it: four bytes a character, and some more so a cut text is seen to
/// go on (VA28: the rest is never built).
const SHOWN_BYTES: usize = SHOWN_CHARS * 4 + 16;

/// `text` cut to `SHOWN_CHARS` characters, with `…` when it was cut.
fn shown(text: String) -> Arg {
    Arg::Str(cut(text))
}

/// `text` cut to `SHOWN_CHARS` characters, with `…` when it was cut.
fn cut(mut text: String) -> String {
    if let Some((end, _)) = text.char_indices().nth(SHOWN_CHARS) {
        text.truncate(end);
        text.push('…');
    }
    text
}

/// A value in an error's message: its literal form, cut as `shown` cuts,
/// with only the part shown ever built (VA28).
pub(crate) fn shown_value(v: &Value) -> String {
    cut(format_start(v, SHOWN_BYTES))
}

/// Whether a discarded `v` may hold an Err: the cheap test the run loop
/// makes on every discarded value, before the cold path looks closer.
pub(crate) fn may_hold_err(v: &Value) -> bool {
    matches!(
        v,
        Value::List(_) | Value::Map(_) | Value::Tuple(_) | Value::Tag(_)
    )
}

/// E0606 (without a position) when a discarded place drops an Err (02:
/// 버려지는 Err): a statement's value (POP), what `each`'s `f` returned,
/// a discarded callback's result. It shows the Err as E0603 does. An Err
/// at any depth of the value's lists, maps, tuples and tags counts (D54),
/// and the note says where it is. The test is the probe the lists' and
/// maps' nodes remember (`Value::holds_err`), so a value without an Err
/// costs a look at its parts once; the path is then found by one walk.
pub(crate) fn discarded_err(v: &Value) -> Option<Diag> {
    if !v.holds_err() {
        return None;
    }
    let (e, path) = err_path(v);
    let mut d = error("E0606", no_pos(), vec![shown(err_message(e))]);
    if !path.is_empty() {
        d.notes.push(format!("note: {}", path.join(", then ")));
    }
    Some(d)
}

/// The first Err in `v` (which holds one) and the steps to it, in words.
/// One walk in order with an explicit stack, each part visited once (a
/// part shared through an `Rc` too): linear in the value's distinct parts
/// however deep they nest.
fn err_path(v: &Value) -> (&Value, Vec<String>) {
    /// How a part was reached from its parent.
    enum Step {
        List(usize, usize),
        Map(Rc<crate::value::Text>),
        Tuple(usize, usize),
        Tag(Sym, usize, usize),
    }
    // Every part reached: its parent's index here and the step from it.
    let mut parts: Vec<(usize, Option<Step>)> = vec![(0, None)];
    let mut stack: Vec<(&Value, usize)> = vec![(v, 0)];
    let mut seen: std::collections::HashSet<*const ()> = std::collections::HashSet::new();
    let found = loop {
        let Some((x, at)) = stack.pop() else {
            unreachable!("the value holds an Err");
        };
        if is_err(x) {
            break (x, at);
        }
        let ptr: *const () = match x {
            Value::List(rc) => Rc::as_ptr(rc).cast(),
            Value::Map(rc) => Rc::as_ptr(rc).cast(),
            Value::Tuple(rc) => Rc::as_ptr(rc).cast(),
            Value::Tag(rc) => Rc::as_ptr(rc).cast(),
            _ => continue,
        };
        if !seen.insert(ptr) {
            continue;
        }
        let mut kids: Vec<(&Value, Step)> = Vec::new();
        match x {
            Value::List(xs) => {
                let n = xs.len();
                kids.extend(xs.iter().enumerate().map(|(i, y)| (y, Step::List(i, n))));
            }
            Value::Map(m) => kids.extend(m.iter().map(|(k, y)| (y, Step::Map(Rc::new(k.clone()))))),
            Value::Tuple(t) => {
                let n = t.len();
                kids.extend(t.iter().enumerate().map(|(i, y)| (y, Step::Tuple(i, n))));
            }
            Value::Tag(t) => {
                let n = t.args.len();
                kids.extend(
                    t.args
                        .iter()
                        .enumerate()
                        .map(|(i, y)| (y, Step::Tag(t.name, i, n))),
                );
            }
            _ => {}
        }
        // The first part is taken first.
        for (y, step) in kids.into_iter().rev() {
            parts.push((at, Some(step)));
            stack.push((y, parts.len() - 1));
        }
    };
    let (e, mut at) = found;
    let mut steps = Vec::new();
    while let (parent, Some(step)) = &parts[at] {
        steps.push(step);
        at = *parent;
    }
    steps.reverse();
    let path = steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            // The first step names the discarded value.
            let whose = if i == 0 { "the discarded " } else { "" };
            match step {
                Step::List(j, n) if i == 0 => {
                    format!("element {} of the discarded list (of {n})", j + 1)
                }
                Step::List(j, n) => format!("element {} (of {n})", j + 1),
                Step::Map(k) if i == 0 => format!(
                    "key {} of the discarded map",
                    crate::value::format(&Value::Str(k.clone()))
                ),
                Step::Map(k) => format!("key {}", crate::value::format(&Value::Str(k.clone()))),
                Step::Tuple(j, n) if i == 0 => {
                    format!("item {} of the discarded tuple (of {n})", j + 1)
                }
                Step::Tuple(j, n) => format!("item {} (of {n})", j + 1),
                Step::Tag(name, _, 1) => format!("the value of {whose}`{}(…)`", name.name()),
                Step::Tag(name, j, _) => {
                    format!("argument {} of {whose}`{}(…)`", j + 1, name.name())
                }
            }
        })
        .collect();
    (e, path)
}

fn is_err(v: &Value) -> bool {
    matches!(result_kind(v), ResultKind::Err)
}

/// What a callback whose result is discarded came to (02: `with`'s
/// release, `supervise`'s on_fail): an Err it returned becomes its
/// runtime error E0606.
pub(crate) fn discarded(r: Result<Value, Stop>) -> Result<Value, Stop> {
    match r {
        Ok(v) => match discarded_err(&v) {
            Some(d) => Err(Stop::Error(Box::new(d))),
            None => Ok(v),
        },
        r => r,
    }
}

impl Task<'_> {
    /// MATCH at `ip`: the value on top against the pattern its second
    /// operand names; on failure frame `fi` goes to `to` (VM29). Out of the run loop, which a bigger body
    /// slows down for every instruction.
    #[inline(never)]
    pub(super) fn match_op(&mut self, fi: usize, ip: usize, to: usize) {
        let pi = self.fiber.frames[fi].closure.code[ip].b();
        let v = self.pop();
        if !self.match_pattern(fi, &v, pi) {
            self.fiber.frames[fi].ip = to;
        }
    }

    /// TAG and MAP at `ip`: `n` values on the stack, and the name or the
    /// keys in the constant its second operand names (VM30).
    #[inline(never)]
    pub(super) fn make_tagged(&mut self, fi: usize, ip: usize, op: Op, n: usize) {
        let b = self.fiber.frames[fi].closure.code[ip].b();
        if op == Op::Tag {
            let args = self.pop_items(n);
            let Value::Sym(name) = self.fiber.frames[fi].proto().consts[b] else {
                panic!("TAG with a name that is not a symbol");
            };
            debug_assert!(
                !matches!(name, Sym::OK | Sym::ERR) || args.len() == 1,
                "the compiler gives Ok and Err exactly one value (E0236)"
            );
            self.push(Value::Tag(Rc::new(Tag { name, args })));
            return;
        }
        let vals = self.pop_n(n);
        let Value::List(keys) = &self.fiber.frames[fi].proto().consts[b] else {
            panic!("MAP with keys that are not a list");
        };
        let mut m = MapBuilder::default();
        for (k, v) in keys.iter().zip(vals) {
            let Value::Str(k) = k else {
                panic!("MAP with a key that is not a string");
            };
            m.set(k.clone(), v);
        }
        self.push(Value::Map(Rc::new(m.finish())));
    }

    /// Pops the top `n` operands in push order.
    fn pop_n(&mut self, n: usize) -> Vec<Value> {
        let start = self.fiber.stack.len() - n;
        self.fiber
            .stack
            .drain(start..)
            .map(|v| v.expect("VM read an unassigned slot"))
            .collect()
    }

    /// Pops the top `n` operands in push order as a tag's or a tuple's
    /// values: one or two need no buffer of their own (VM8).
    fn pop_items(&mut self, n: usize) -> Items {
        let start = self.fiber.stack.len() - n;
        self.fiber
            .stack
            .drain(start..)
            .map(|v| v.expect("VM read an unassigned slot"))
            .collect()
    }

    /// Runs one M4 instruction of frame `fi`.
    pub(super) fn exec_m4(&mut self, fi: usize, op: Op, arg: usize, pos: Pos) -> Result<(), Stop> {
        match op {
            Op::List => {
                let items = self.pop_n(arg);
                self.push(Value::List(Rc::new(List::new(items))));
            }
            Op::Tuple => {
                let items = self.pop_items(arg);
                self.push(Value::Tuple(Rc::new(Tuple::of(items))));
            }
            Op::Range => {
                let end = self.pop();
                let start = self.pop();
                let (Value::Int(s), Value::Int(e)) = (&start, &end) else {
                    return Err(err(
                        "E0501",
                        pos,
                        vec![
                            Arg::Str("make a range from".into()),
                            Arg::Str(type_name(&start)),
                            Arg::Str(type_name(&end)),
                        ],
                    ));
                };
                let inclusive = arg == 1;
                if !Range::in_order(*s, *e, inclusive) {
                    return Err(err("E0522", pos, vec![Arg::Int(*s), Arg::Int(*e)]));
                }
                self.push(Value::Range(Rc::new(Range {
                    start: *s,
                    end: *e,
                    inclusive,
                })));
            }
            Op::Index => {
                let i = self.pop();
                let v = self.pop();
                let r = index::index(&v, &i).map_err(|d| at(d, pos))?;
                self.push(r);
            }
            Op::Slice => {
                let end = if arg & 2 != 0 { Some(self.pop()) } else { None };
                let start = if arg & 1 != 0 { Some(self.pop()) } else { None };
                let v = self.pop();
                let r = index::slice(&v, start.as_ref(), end.as_ref(), arg & 4 != 0)
                    .map_err(|d| at(d, pos))?;
                self.push(r);
            }
            Op::Field => {
                let v = self.pop();
                // `index` answers for a module too: its member, E0303
                // for no such member, E0304 for a private one.
                let name = &self.fiber.frames[fi].proto().consts[arg];
                let r =
                    index::index(&v, name).map_err(|d| at(index::field_help(d, &v, name), pos))?;
                self.push(r);
            }
            Op::In => {
                let container = self.pop();
                let x = self.pop();
                let found = index::contains(&container, &x).map_err(|d| at(d, pos))?;
                self.push(Value::Bool(found));
            }
            Op::UnwrapAbort => {
                let v = self.pop();
                match result_kind(&v) {
                    ResultKind::Ok(payload) => self.push(payload),
                    ResultKind::Err => {
                        return Err(err("E0603", pos, vec![shown(err_message(&v))]));
                    }
                    ResultKind::Nil => return Err(err("E0604", pos, vec![s("!")])),
                    ResultKind::Other => {
                        return Err(err("E0605", pos, vec![s("!"), Arg::Str(not_result(&v))]));
                    }
                }
            }
            Op::UnwrapTest => match result_kind(self.top()) {
                ResultKind::Ok(payload) => self.replace_top(payload),
                // The Err stays on the stack for RETURN.
                ResultKind::Err => self.fiber.frames[fi].ip = arg,
                ResultKind::Nil => return Err(err("E0604", pos, vec![s("?")])),
                ResultKind::Other => {
                    let tn = not_result(self.top());
                    return Err(err("E0605", pos, vec![s("?"), Arg::Str(tn)]));
                }
            },
            // The value goes on to `arg`; nil or an Err gives way to the
            // right side, which follows (VM26).
            Op::Coalesce => match result_kind(self.top()) {
                ResultKind::Ok(payload) => {
                    self.replace_top(payload);
                    self.fiber.frames[fi].ip = arg;
                }
                ResultKind::Err | ResultKind::Nil => {
                    self.pop();
                }
                ResultKind::Other => self.fiber.frames[fi].ip = arg,
            },
            Op::MatchOrFail => {
                let v = self.pop();
                if !self.match_pattern(fi, &v, arg) {
                    return Err(err("E0602", pos, vec![Arg::Str(shown_value(&v))]));
                }
            }
            Op::NoMatch => {
                let v = self.pop();
                return Err(err("E0601", pos, vec![Arg::Str(shown_value(&v))]));
            }
            _ => unreachable!("exec_m4 called with {}", op.name()),
        }
        Ok(())
    }

    fn replace_top(&mut self, v: Value) {
        let top = self.fiber.stack.last_mut().expect("VM stack underflow");
        *top = Some(v);
    }

    /// Matches `v` against pattern `pi` of frame `fi`, binding into the
    /// frame's slots and, only if the whole pattern matched, its globals.
    pub(super) fn match_pattern(&mut self, fi: usize, v: &Value, pi: usize) -> bool {
        let f = &self.fiber.frames[fi];
        let mut b = Binder {
            stack: &mut self.fiber.stack,
            base: f.base(),
            globals: f.globals(),
            pending: Vec::new(),
        };
        let ok = b.matches(v, &f.proto().patterns[pi]);
        if ok {
            b.commit();
        }
        ok
    }
}
