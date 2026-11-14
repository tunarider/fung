//! Which functions have no effect (D141): read from their bodies.
//!
//! The only hand-made mark is the spec's `pure` on a function written in
//! Rust (a native or a `_native.*` member); without it a native has an
//! effect. A function written in fung (the file's, a local one, a lambda
//! or anonymous `fn`, a partial application, the prelude's and the
//! standard modules' written in fung) has none when nothing its body
//! does has one: no call of a function with an effect or of a function
//! not known here, no `assert`, no `!` (it stops the program). A function
//! that calls a function it is given has no effect only when the one it
//! is given has none, so each function's summary is "pure if the
//! functions given at these parameters are" (`Purity`). A pure native
//! calls the argument of its parameter `f` or `pred` (the spec's names
//! for a function parameter). Functions that call one another start as
//! pure and are worked out together, until nothing changes.
//!
//! When unsure, a call has an effect: what can go wrong is a mistake not
//! caught (E0234 not raised), never good code refused.

use std::collections::{BTreeSet, HashMap};

use super::Resolver;
use super::call::Sig;
use super::value::{PureCall, known_type};
use crate::tree::{Head, Lit, Node};

/// Where a function's node is written: its key in the table of bodies.
pub(super) type FnKey = (u32, u32);

/// The key of the function node `n`.
pub(super) fn key(n: &Node) -> FnKey {
    (n.pos.line, n.pos.col)
}

/// A summary: `None` has an effect; `Some(ps)` has none when the
/// functions given at the parameters `ps` (indices) have none.
pub(super) type Purity = Option<BTreeSet<usize>>;

/// What a call calls.
#[derive(Clone, Debug)]
pub(super) enum Callee {
    /// A function the call is sure to be: its parameters and summary.
    Known(Sig),
    /// A parameter of the function the call is in.
    Param(usize),
    /// Anything else.
    Unknown,
}

/// What an argument is, as far as its function goes.
#[derive(Clone, Debug)]
pub(super) enum ArgVal {
    /// A function value the code shows.
    Fn(Callee),
    /// A parameter of the function the call is in.
    Param(usize),
    /// A literal that is no function.
    Data,
    /// Anything else.
    Unknown,
}

/// One thing a body does that may have an effect.
#[derive(Clone, Debug)]
pub(super) enum Effect {
    /// An effect whatever the values (`assert`, `!`).
    Always,
    /// A call, its arguments by the callee's parameter index.
    Call(Callee, Vec<(usize, ArgVal)>),
}

/// A call that may be a statement: E0234 when it has no effect.
pub(super) struct Site {
    pub(super) at: (u32, u32),
    pub(super) call: Effect,
    pub(super) shown: PureCall,
}

/// The arguments `args` (as written; `got` resolved) by parameter index
/// of `sig`: positional ones in order (those past the fixed parameters
/// go to the rest one), named ones by name. After a spread the positions
/// are unknown: those arguments are left out.
fn by_param(r: &mut Resolver<'_>, sig: &Sig, args: &[Node], got: &[Node]) -> Vec<(usize, ArgVal)> {
    let n = sig.params.len();
    let rest = n > 0 && sig.params[n - 1].rest;
    let fixed = if rest { n - 1 } else { n };
    let mut out = Vec::new();
    let mut i = 0;
    let mut spread = false;
    for (a, g) in args.iter().zip(got) {
        match a.head {
            Head::Spread => spread = true,
            Head::Named => {
                if let Some(j) = sig.params.iter().position(|p| p.name == super::lit_str(a)) {
                    out.push((j, r.arg_val(&a.kids[0], &g.kids[0])));
                }
            }
            _ if !spread => {
                let j = if i < fixed { i } else { fixed };
                if j < n {
                    out.push((j, r.arg_val(a, g)));
                }
                i += 1;
            }
            _ => {}
        }
    }
    out
}

impl Resolver<'_> {
    /// The parameter index `got` (a resolved read) is, in the function
    /// being resolved: a local slot below its parameter count.
    fn param_of(&self, got: &Node) -> Option<usize> {
        let f = self.cur();
        if self.fns.len() < 2 || got.head != Head::Local {
            return None;
        }
        let Lit::Int(i) = got.lit else {
            return None;
        };
        (usize::try_from(i).ok()? < f.nparams).then_some(i as usize)
    }

    /// What the argument `a` (as written; `got` resolved) is.
    fn arg_val(&mut self, a: &Node, got: &Node) -> ArgVal {
        if let Some((_, sig)) = self.function_value(a, Some(got)) {
            return ArgVal::Fn(Callee::Known(sig));
        }
        if let Some(i) = self.param_of(got) {
            return ArgVal::Param(i);
        }
        match known_type(self.literal_view(a)) {
            Some("fn") | None => ArgVal::Unknown,
            Some(_) => ArgVal::Data,
        }
    }

    /// Notes a call: what the body around it does, and the call site for
    /// E0234. `n` as written, `kids` resolved; `wrong`: its arguments are
    /// an error already, so it is no E0234 too.
    pub(super) fn note_call(
        &mut self,
        n: &Node,
        kids: &[Node],
        known: Option<(String, Sig)>,
        wrong: bool,
    ) {
        let (call, shown) = match known {
            Some((name, sig)) => {
                let args = by_param(self, &sig, &n.kids[1..], &kids[1..]);
                // The underline starts where the callee is written. The
                // error shows the callee as written, its help the
                // function's own name (`s = sort; s(xs)`).
                let (shown, at) = match n.kids[0].head {
                    Head::Field => (
                        format!("{}.{name}", super::lit_str(&n.kids[0].kids[0])),
                        n.kids[0].kids[0].pos,
                    ),
                    Head::Name => (super::lit_str(&n.kids[0]).to_string(), n.kids[0].pos),
                    _ => (name.clone(), n.kids[0].pos),
                };
                // a function made without a name is called by the name
                // that holds it
                let own = match n.kids[0].head {
                    Head::Field => shown.clone(),
                    _ if name == super::call::NAMELESS => shown.clone(),
                    _ => name,
                };
                // worked out with the bodies (`settle_effects`)
                let takes_fn = false;
                (
                    Effect::Call(Callee::Known(sig), args),
                    Some(PureCall {
                        shown,
                        own,
                        at,
                        takes_fn,
                    }),
                )
            }
            None => match self.param_of(&kids[0]) {
                Some(i) => (Effect::Call(Callee::Param(i), Vec::new()), None),
                None => (Effect::Call(Callee::Unknown, Vec::new()), None),
            },
        };
        self.cur_mut().effects.push(call.clone());
        // a call whose arguments are wrong is that error, not also E0234
        if let Some(shown) = shown.filter(|_| !wrong) {
            self.sites.push(Site {
                at: (n.pos.line, n.pos.col),
                call,
                shown,
            });
        }
    }

    /// Notes something with an effect whatever the values in the body
    /// being resolved (`assert`, `!`).
    pub(super) fn note_effect(&mut self) {
        self.cur_mut().effects.push(Effect::Always);
    }

    /// Works out every body's summary and marks the calls with no effect
    /// for E0234 (`pure_calls`). Every body starts as pure; a body is
    /// worked out again only when a body it reads changed (a worklist), so
    /// an effect climbs a chain of calls in one pass. A summary only
    /// grows (more parameters, then an effect), so each body changes a
    /// bounded number of times.
    pub(super) fn settle_effects(&mut self) {
        let mut table: HashMap<FnKey, Purity> = self
            .bodies
            .keys()
            .map(|k| (*k, Some(BTreeSet::new())))
            .collect();
        // who reads whose summary
        let mut readers: HashMap<FnKey, Vec<FnKey>> = HashMap::new();
        for (k, effects) in &self.bodies {
            for e in effects {
                for read in reads(e) {
                    if self.bodies.contains_key(&read) {
                        readers.entry(read).or_default().push(*k);
                    }
                }
            }
        }
        let mut todo: Vec<FnKey> = self.bodies.keys().copied().collect();
        let mut queued: std::collections::HashSet<FnKey> = todo.iter().copied().collect();
        while let Some(k) = todo.pop() {
            queued.remove(&k);
            let got = summary_of(&self.bodies[&k], &table);
            if table[&k] == got {
                continue;
            }
            table.insert(k, got);
            for r in readers.get(&k).into_iter().flatten() {
                if queued.insert(*r) {
                    todo.push(*r);
                }
            }
        }
        for mut site in std::mem::take(&mut self.sites) {
            if eval(&site.call, &table).is_some_and(|ps| ps.is_empty()) {
                // it calls a function it is given: its help points at
                // `each` (worked out from the body for the file's own)
                if let Effect::Call(Callee::Known(sig), _) = &site.call {
                    site.shown.takes_fn = summary(sig, &table).is_some_and(|ps| !ps.is_empty());
                }
                self.pure_calls.insert(site.at, site.shown);
            }
        }
        self.purity = table;
    }

    /// `sig` with its summary filled from the bodies worked out here, for
    /// another file: whether pure, and the parameters it calls.
    pub(super) fn exported(&self, sig: &Sig) -> Sig {
        let mut out = sig.clone();
        if let Some(k) = sig.body {
            match self.purity.get(&k).cloned().flatten() {
                Some(ps) => {
                    out.pure = true;
                    out.calls = ps.into_iter().collect();
                }
                None => {
                    out.pure = false;
                    out.calls = Vec::new();
                }
            }
        }
        out.body = None;
        out
    }
}

/// The bodies whose summaries one effect reads: a known callee's, and
/// those of the functions given to it.
fn reads(e: &Effect) -> Vec<FnKey> {
    let Effect::Call(callee, args) = e else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Callee::Known(sig) = callee {
        out.extend(sig.body);
    }
    for (_, a) in args {
        if let ArgVal::Fn(Callee::Known(sig)) = a {
            out.extend(sig.body);
        }
    }
    out
}

/// A body's summary from what it does.
fn summary_of(effects: &[Effect], table: &HashMap<FnKey, Purity>) -> Purity {
    let mut ps = BTreeSet::new();
    for e in effects {
        ps.extend(eval(e, table)?);
    }
    Some(ps)
}

/// A function's own summary: its body's, or what its signature says.
fn summary(sig: &Sig, table: &HashMap<FnKey, Purity>) -> Purity {
    if let Some(k) = sig.body
        && let Some(p) = table.get(&k)
    {
        return p.clone();
    }
    sig.pure.then(|| sig.calls.iter().copied().collect())
}

/// What one effect needs of the parameters of the function it is in.
fn eval(e: &Effect, table: &HashMap<FnKey, Purity>) -> Purity {
    let Effect::Call(callee, args) = e else {
        return None;
    };
    match callee {
        Callee::Unknown => None,
        Callee::Param(i) => Some(BTreeSet::from([*i])),
        Callee::Known(sig) => {
            let mut ps = BTreeSet::new();
            for j in summary(sig, table)? {
                let (_, a) = args.iter().find(|(k, _)| *k == j)?;
                match a {
                    ArgVal::Data => {}
                    ArgVal::Param(i) => {
                        ps.insert(*i);
                    }
                    ArgVal::Fn(Callee::Known(s)) => {
                        // called with values it does not know: it must be
                        // pure whatever it is given
                        if !summary(s, table)?.is_empty() {
                            return None;
                        }
                    }
                    ArgVal::Fn(Callee::Param(i)) => {
                        ps.insert(*i);
                    }
                    ArgVal::Fn(Callee::Unknown) | ArgVal::Unknown => return None,
                }
            }
            Some(ps)
        }
    }
}

/// The parameters a function from the spec marked `pure` calls: those
/// named `f` or `pred`.
pub(super) fn spec_calls(sig: &Sig) -> Vec<usize> {
    sig.params
        .iter()
        .enumerate()
        .filter(|(_, p)| p.name == "f" || p.name == "pred")
        .map(|(i, _)| i)
        .collect()
}
