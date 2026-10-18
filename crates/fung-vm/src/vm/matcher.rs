//! Pattern matching: one place holds what every pattern means (09). The
//! front end's pattern tree is linked into `RtPattern`, whose literals are
//! values and whose tag names are symbols, so MATCH compares without
//! converting anything.

use std::rc::Rc;

use fung_front::bytecode::{PatKind, Pattern};

use super::link::{Prelude, constant};
use crate::value::{Globals, Sym, Value, equal, intern};

/// A linked pattern node.
#[derive(Debug)]
pub enum RtPattern {
    /// `_`
    Wild,
    /// A name: writes the matched value into a slot or global.
    Bind { global: bool, index: usize },
    /// A literal compared with `==`.
    Lit(Value),
    /// `Name(p…)`
    Tag { name: Sym, kids: Vec<RtPattern> },
    /// `(p, …)`
    Tuple(Vec<RtPattern>),
    /// `[p, …]`; `rest` is `None` for an exact length.
    List {
        kids: Vec<RtPattern>,
        rest: Option<Box<RtPattern>>,
    },
    /// `{k: p, …}`: other keys are ignored.
    Map {
        keys: Vec<String>,
        kids: Vec<RtPattern>,
    },
    /// `p | p`
    Alt(Vec<RtPattern>),
}

/// Links a front-end pattern.
pub(super) fn link_pattern(p: &Pattern, prelude: &Prelude) -> RtPattern {
    let kids = || p.kids.iter().map(|k| link_pattern(k, prelude)).collect();
    match p.kind {
        PatKind::Wild => RtPattern::Wild,
        PatKind::Bind => RtPattern::Bind {
            global: p.global,
            index: p.index,
        },
        PatKind::Lit => RtPattern::Lit(match &p.lit {
            Some(c) => constant(c, prelude),
            None => Value::Nil,
        }),
        PatKind::Tag => RtPattern::Tag {
            name: intern(&p.name),
            kids: kids(),
        },
        PatKind::Tuple => RtPattern::Tuple(kids()),
        PatKind::List => RtPattern::List {
            kids: kids(),
            rest: p.rest.as_ref().map(|r| Box::new(link_pattern(r, prelude))),
        },
        PatKind::Map => RtPattern::Map {
            keys: p.keys.clone(),
            kids: kids(),
        },
        PatKind::Alt => RtPattern::Alt(kids()),
    }
}

/// Where a match writes the names it binds: the frame's slots and its
/// module's globals.
pub(super) struct Binder<'a> {
    pub stack: &'a mut [Option<Value>],
    pub base: usize,
    pub globals: &'a Globals,
}

impl Binder<'_> {
    /// Tests `v` against `p` and, on success, writes every bound name. A
    /// failed match may have written some names already; the compiler
    /// never reads a name from a failed arm, so those writes are harmless.
    pub fn matches(&mut self, v: &Value, p: &RtPattern) -> bool {
        match p {
            RtPattern::Wild => true,
            RtPattern::Bind { global, index } => {
                if *global {
                    // The old value drops after the table's borrow ends.
                    let old = self.globals.borrow_mut()[*index].replace(v.clone());
                    drop(old);
                } else {
                    self.stack[self.base + index] = Some(v.clone());
                }
                true
            }
            RtPattern::Lit(lit) => equal(v, lit),
            RtPattern::Tag { name, kids } => match v {
                Value::Tag(t) if t.name == *name => self.all(&t.args, kids),
                _ => false,
            },
            RtPattern::Tuple(kids) => match v {
                Value::Tuple(t) => self.all(t, kids),
                _ => false,
            },
            RtPattern::List { kids, rest } => {
                let Value::List(l) = v else {
                    return false;
                };
                let n = kids.len();
                if l.len() < n || (rest.is_none() && l.len() != n) {
                    return false;
                }
                // Only the first n items are matched one by one; the rest
                // is a view on the same list (cheap for big lists, M10).
                for (i, k) in kids.iter().enumerate() {
                    let item = l.at(i).expect("checked against the length");
                    if !self.matches(item, k) {
                        return false;
                    }
                }
                match rest.as_deref() {
                    None | Some(RtPattern::Wild) => true,
                    Some(r) => {
                        let tail = if n == 0 {
                            v.clone()
                        } else {
                            Value::List(Rc::new(l.slice(n, l.len())))
                        };
                        self.matches(&tail, r)
                    }
                }
            }
            RtPattern::Map { keys, kids } => {
                let Value::Map(m) = v else {
                    return false;
                };
                keys.iter()
                    .zip(kids)
                    .all(|(k, p)| m.get(k).is_some_and(|item| self.matches(item, p)))
            }
            RtPattern::Alt(alts) => alts.iter().any(|alt| self.matches(v, alt)),
        }
    }

    /// Matches items against patterns pairwise; lengths must agree.
    fn all(&mut self, items: &[Value], pats: &[RtPattern]) -> bool {
        items.len() == pats.len() && items.iter().zip(pats).all(|(x, p)| self.matches(x, p))
    }
}
