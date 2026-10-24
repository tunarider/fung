//! The M4 instructions: building collections, indexing, slicing, `in`, the
//! result operators `!` `?` `??`, and the pattern instructions.

use std::rc::Rc;

use fung_front::bytecode::Op;
use fung_front::diag::Arg;
use fung_front::source::Pos;

use super::matcher::Binder;
use super::{Stop, Task, at, err};
use crate::value::{List, MapBuilder, Range, Tag, Value, format, index, type_name};

/// How `!`, `?` and `??` see a value (02): an Ok payload, an Err, nil, or
/// something else. `!` rejects nil (E0604), `?` lets it through, `??`
/// replaces it; anything else is E0605 for `!` and `?`.
enum ResultKind {
    Ok(Value),
    Err,
    Nil,
    Other,
}

fn result_kind(v: &Value) -> ResultKind {
    match v {
        Value::Tag(t) => match t.name.name() {
            "Ok" if t.args.len() == 1 => ResultKind::Ok(t.args[0].clone()),
            "Ok" => ResultKind::Ok(Value::tuple(t.args.clone())),
            "Err" => ResultKind::Err,
            _ => ResultKind::Other,
        },
        Value::Nil => ResultKind::Nil,
        _ => ResultKind::Other,
    }
}

/// Renders an Err for E0603 (02): a single payload that is a map with a
/// string `message` shows that; a payload of another shape shows its
/// literal form; an Err with zero or several payloads shows the whole tag.
fn err_message(e: &Value) -> String {
    let Value::Tag(t) = e else {
        return format(e);
    };
    if t.args.len() != 1 {
        return format(e);
    }
    let v = &t.args[0];
    if let Value::Map(m) = v
        && let Some(Value::Str(s)) = m.get("message")
    {
        return s.to_string();
    }
    format(v)
}

fn s(x: &str) -> Arg {
    Arg::Str(x.to_string())
}

/// How much of a value a pattern or `!` error shows in its one-line
/// headline (08): a value can print megabytes.
const SHOWN_CHARS: usize = 200;

/// `text` cut to `SHOWN_CHARS` characters, with `…` when it was cut.
fn shown(mut text: String) -> Arg {
    if let Some((end, _)) = text.char_indices().nth(SHOWN_CHARS) {
        text.truncate(end);
        text.push('…');
    }
    Arg::Str(text)
}

/// E0606's argument when `v` is an Err that a discarded place drops
/// (02: 버려지는 Err): a statement's value (POP) or what `each`'s `f`
/// returned. It shows the Err as E0603 does.
/// Whether `v` is an `Err`: the cheap test the run loop makes on every
/// discarded value, before the cold path builds E0606.
pub(crate) fn is_err(v: &Value) -> bool {
    matches!(v, Value::Tag(t) if t.name.name() == "Err")
}

pub(crate) fn discarded_err(v: &Value) -> Option<Arg> {
    match result_kind(v) {
        ResultKind::Err => Some(shown(err_message(v))),
        _ => None,
    }
}

/// What a callback whose result is discarded came to (02: `with`'s
/// release, `supervise`'s on_fail): an Err it returned becomes its
/// runtime error E0606.
pub(crate) fn discarded(r: Result<Value, Stop>) -> Result<Value, Stop> {
    match r {
        Ok(v) => match discarded_err(&v) {
            Some(a) => Err(crate::native::nat::fail("E0606", vec![a])),
            None => Ok(v),
        },
        r => r,
    }
}

impl Task<'_> {
    /// Pops the top `n` operands in push order.
    fn pop_n(&mut self, n: usize) -> Vec<Value> {
        let start = self.stack.len() - n;
        self.stack
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
                let items = self.pop_n(arg);
                self.push(Value::tuple(items));
            }
            Op::Map => {
                let mut m = MapBuilder::default();
                let mut pairs = self.pop_n(2 * arg).into_iter();
                while let (Some(k), Some(v)) = (pairs.next(), pairs.next()) {
                    let Value::Str(k) = k else {
                        panic!("MAP with a key that is not a string");
                    };
                    m.set(k, v);
                }
                self.push(Value::Map(Rc::new(m.finish())));
            }
            Op::Tag => {
                let args = self.pop_n(arg);
                let Value::Sym(name) = self.pop() else {
                    panic!("TAG with a name that is not a symbol");
                };
                self.push(Value::Tag(Rc::new(Tag { name, args })));
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
                self.push(Value::Range(Rc::new(Range {
                    start: *s,
                    end: *e,
                    inclusive: arg == 1,
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
                let name = &self.frames[fi].proto.consts[arg];
                let r = index::index(&v, name).map_err(|d| at(d, pos))?;
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
                        return Err(err("E0605", pos, vec![s("!"), Arg::Str(type_name(&v))]));
                    }
                }
            }
            Op::UnwrapTest => match result_kind(self.top()) {
                ResultKind::Ok(payload) => self.replace_top(payload),
                // The Err stays on the stack for RETURN.
                ResultKind::Err => self.frames[fi].ip = arg,
                // nil is a value, not a failure: `?` only looks at Err (02).
                ResultKind::Nil => {}
                ResultKind::Other => {
                    let tn = type_name(self.top());
                    return Err(err("E0605", pos, vec![s("?"), Arg::Str(tn)]));
                }
            },
            Op::Coalesce => match result_kind(self.top()) {
                ResultKind::Ok(payload) => self.replace_top(payload),
                ResultKind::Err | ResultKind::Nil => {
                    self.pop();
                    self.frames[fi].ip = arg;
                }
                ResultKind::Other => {}
            },
            Op::Match => {
                let v = self.pop();
                let ok = self.match_pattern(fi, &v, arg);
                self.push(Value::Bool(ok));
            }
            Op::MatchOrFail => {
                let v = self.pop();
                if !self.match_pattern(fi, &v, arg) {
                    return Err(err("E0602", pos, vec![shown(format(&v))]));
                }
            }
            Op::NoMatch => {
                let v = self.pop();
                return Err(err("E0601", pos, vec![shown(format(&v))]));
            }
            _ => unreachable!("exec_m4 called with {}", op.name()),
        }
        Ok(())
    }

    fn replace_top(&mut self, v: Value) {
        let top = self.stack.last_mut().expect("VM stack underflow");
        *top = Some(v);
    }

    /// Matches `v` against pattern `pi` of frame `fi`, binding into the
    /// frame's slots and, only if the whole pattern matched, its globals.
    fn match_pattern(&mut self, fi: usize, v: &Value, pi: usize) -> bool {
        let f = &self.frames[fi];
        let mut b = Binder {
            stack: &mut self.stack,
            base: f.base,
            globals: &f.globals,
            pending: Vec::new(),
        };
        let ok = b.matches(v, &f.proto.patterns[pi]);
        if ok {
            b.commit();
        }
        ok
    }
}
