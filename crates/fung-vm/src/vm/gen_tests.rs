//! Generated programs against a reference evaluator (VM41). The mutation
//! tests of the front end (TL9) break existing programs and run none; this
//! test writes new ones that compile and runs them. A fixed-seed generator
//! that knows the grammar makes small functions out of the shapes whose
//! lifetimes and jumps the compiler works hardest on: branches as
//! statements and as values, `match` with guards, alternatives and list
//! patterns, `??`, `and`/`or`/`not` conditions, lambdas that capture,
//! reassignment, `return` from inside branches, tail calls and return
//! types, parameter defaults that bind names of their own (VM48) and
//! calls that skip, give or name them. Every value that matters is printed on the way (`t(x)`), and a
//! direct evaluator of the same tree, written without any of the
//! compiler's machinery, says what the program must print and how it must
//! end. The compiler verifies every program it makes, so a verifier
//! violation fails the test too (E0801).
//!
//! A program the generator made that does not compile (an arm after one
//! that always matches, say) is skipped; most compile, and the test
//! asserts that.
//!
//! `FUNG_GEN_CASES` sets how many programs to try (default
//! [`DEFAULT_CASES`]) and `FUNG_GEN_SEED` the seed (default
//! [`DEFAULT_SEED`]):
//!
//!   FUNG_GEN_CASES=100000 FUNG_GEN_SEED=7 cargo test --release -p fung-vm generated_programs

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::rc::Rc;

use crate::testutil::{on_big_stack, run};

const DEFAULT_CASES: usize = 1500;
const DEFAULT_SEED: u64 = 0x6e0_f00d_4d41;
/// The calls a reference run may make before the program is skipped as
/// too long (functions only call the ones before them, so they all end).
const MAX_CALLS: usize = 20_000;

fn env_num(name: &str, default: u64) -> u64 {
    match std::env::var(name) {
        Ok(v) => v
            .parse()
            .unwrap_or_else(|_| panic!("{name}={v}: not a number")),
        Err(_) => default,
    }
}

/// xorshift64*: the same sequence everywhere.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Rng((z ^ (z >> 31)) | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// A number in `lo..=hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }

    /// A number in `0.0..1.0`.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next() % xs.len() as u64) as usize]
    }
}

/// What a name holds: an int, a list of ints, a lambda of no arguments.
#[derive(Clone, Copy, PartialEq)]
enum Ty {
    Int,
    List,
    Lam,
}

#[derive(Clone)]
enum Expr {
    Int(i64),
    Var(String),
    /// `t(e)`: prints the value and gives it back.
    Traced(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    /// A block `if` with both branches, as a value.
    If(Box<Expr>, Box<Block>, Box<Block>),
    Then(Box<Expr>, Box<Expr>, Box<Expr>),
    /// A match on an int, and one on a list.
    Match(Box<Expr>, Vec<Arm>),
    /// `m(a) ?? b`: `m` gives nil for an even number.
    Coalesce(Box<Expr>, Box<Expr>),
    /// A call: positional arguments, then named ones.
    Call(String, Vec<Expr>, Vec<(String, Expr)>),
    LamCall(String),
    Len(Box<Expr>),
    List(Vec<Expr>),
    Concat(Box<Expr>, Box<Expr>),
    Cmp(&'static str, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    /// `tb(k, b)`: prints `k` and gives the bool back.
    TracedBool(i64, bool),
    Bool(bool),
    /// `return e` where a block's value would be.
    Return(Box<Expr>),
}

#[derive(Clone)]
enum Pat {
    Lit(i64),
    Alt(Vec<i64>),
    Bind(String),
    Wild,
    Empty,
    /// `[h, ...rest]` or `[h, ...]`.
    Cons(String, Option<String>),
    Two(String, String),
    /// `[a] | [_, a] | [_, _, a]`.
    Last(String),
}

#[derive(Clone)]
struct Arm {
    pat: Pat,
    guard: Option<Expr>,
    body: Block,
}

#[derive(Clone)]
enum Stmt {
    Let(String, Expr),
    Lam(String, Expr),
    Expr(Expr),
    If(Expr, Vec<Stmt>),
    Return(Expr),
}

#[derive(Clone)]
struct Block {
    stmts: Vec<Stmt>,
    value: Expr,
}

struct Func {
    name: String,
    params: Vec<String>,
    /// Each parameter's default, if it has one (the last ones).
    defaults: Vec<Option<Expr>>,
    body: Block,
    typed: bool,
}

/// What a call needs to know of a function: its name, and each
/// parameter's name and whether it has a default.
type Sig = (String, Vec<(String, bool)>);

// ---------- the generator ----------

struct Gen<'a> {
    rng: &'a mut Rng,
    /// The functions made so far: a call goes to one of them.
    funcs: Vec<Sig>,
    /// The names of each open block, in the order they were made.
    scopes: Vec<Vec<(String, Ty)>>,
    in_lambda: usize,
    used: HashSet<String>,
    names: usize,
}

fn boxed(e: Expr) -> Box<Expr> {
    Box::new(e)
}

impl Gen<'_> {
    fn fresh(&mut self, prefix: &str) -> String {
        self.names += 1;
        format!("{prefix}{}", self.names)
    }

    fn vars(&self, ty: Ty) -> Vec<String> {
        self.scopes
            .iter()
            .flatten()
            .filter(|v| v.1 == ty)
            .map(|v| v.0.clone())
            .collect()
    }

    /// A read of each name of `scope` nothing read (an unread name is a
    /// compile error).
    fn reads(&self, scope: &[(String, Ty)]) -> Vec<Stmt> {
        scope
            .iter()
            .filter(|v| !self.used.contains(&v.0))
            .map(|(n, ty)| {
                let e = match ty {
                    Ty::Int => Expr::Var(n.clone()),
                    Ty::List => Expr::Len(boxed(Expr::Var(n.clone()))),
                    Ty::Lam => Expr::LamCall(n.clone()),
                };
                Stmt::Expr(Expr::Traced(boxed(e)))
            })
            .collect()
    }

    fn var(&mut self, names: &[String]) -> Expr {
        let v = self.rng.pick(names).clone();
        self.used.insert(v.clone());
        Expr::Var(v)
    }

    fn int(&mut self, d: i32) -> Expr {
        let r = self.rng.unit();
        let vs = self.vars(Ty::Int);
        if d <= 0 || r < 0.18 {
            if !vs.is_empty() && self.rng.chance(0.75) {
                return self.var(&vs);
            }
            return Expr::Int(self.rng.range(0, 4));
        }
        if r < 0.26 {
            return Expr::Traced(boxed(self.int(d - 1)));
        }
        if r < 0.40 {
            let (a, b) = (boxed(self.int(d - 1)), boxed(self.int(d - 1)));
            return if self.rng.chance(0.5) {
                Expr::Add(a, b)
            } else {
                Expr::Sub(a, b)
            };
        }
        if r < 0.50 {
            let c = self.bool(d - 1);
            let (a, b) = (
                self.block(d - 1, Ty::Int, false),
                self.block(d - 1, Ty::Int, false),
            );
            return Expr::If(boxed(c), Box::new(a), Box::new(b));
        }
        if r < 0.56 {
            let c = self.bool(d - 1);
            return Expr::Then(boxed(c), boxed(self.int(d - 1)), boxed(self.int(d - 1)));
        }
        if r < 0.68 {
            return self.match_int(d, Ty::Int);
        }
        if r < 0.75 {
            return self.match_list(d, Ty::Int);
        }
        if r < 0.82 {
            return Expr::Coalesce(boxed(self.int(d - 1)), boxed(self.int(d - 1)));
        }
        if r < 0.88 && !self.funcs.is_empty() {
            return self.call(d);
        }
        let lams = self.vars(Ty::Lam);
        if r < 0.93 && !lams.is_empty() {
            let v = self.rng.pick(&lams).clone();
            self.used.insert(v.clone());
            return Expr::LamCall(v);
        }
        if r < 0.97 {
            return Expr::Len(boxed(self.list(d - 1)));
        }
        Expr::Int(self.rng.range(0, 4))
    }

    /// A call: a parameter with a default is skipped, given by position
    /// (while none before it was skipped or named) or named.
    fn call(&mut self, d: i32) -> Expr {
        let (name, params) = self.rng.pick(&self.funcs).clone();
        let (mut positional, mut named) = (Vec::new(), Vec::new());
        let mut by_name = false;
        for (param, has_default) in params {
            let r = self.rng.unit();
            if has_default && r < 0.4 {
                by_name = true;
                continue;
            }
            by_name |= has_default && r < 0.7;
            let e = self.int(d - 1);
            if by_name {
                named.push((param, e));
            } else {
                positional.push(e);
            }
        }
        Expr::Call(name, positional, named)
    }

    fn list(&mut self, d: i32) -> Expr {
        let r = self.rng.unit();
        let vs = self.vars(Ty::List);
        if d <= 0 || r < 0.3 {
            if !vs.is_empty() && self.rng.chance(0.8) {
                return self.var(&vs);
            }
            let n = self.rng.range(0, 3);
            return Expr::List((0..n).map(|_| Expr::Int(self.rng.range(0, 3))).collect());
        }
        if r < 0.5 {
            let n = self.rng.range(0, 3);
            return Expr::List((0..n).map(|_| self.int(d - 1)).collect());
        }
        if r < 0.7 {
            return Expr::Concat(boxed(self.list(d - 1)), boxed(self.list(d - 1)));
        }
        if r < 0.8 {
            let c = self.bool(d - 1);
            let (a, b) = (
                self.block(d - 1, Ty::List, false),
                self.block(d - 1, Ty::List, false),
            );
            return Expr::If(boxed(c), Box::new(a), Box::new(b));
        }
        if r < 0.9 {
            return self.match_int(d, Ty::List);
        }
        self.match_list(d, Ty::List)
    }

    fn cmp(&mut self, d: i32) -> Expr {
        let op = *self.rng.pick(&["<", "==", ">", "!=", "<=", ">="]);
        Expr::Cmp(op, boxed(self.int(d - 1)), boxed(self.int(d - 1)))
    }

    fn bool(&mut self, d: i32) -> Expr {
        let r = self.rng.unit();
        if d <= 0 || r < 0.35 {
            return self.cmp(d);
        }
        if r < 0.5 {
            return Expr::And(boxed(self.bool(d - 1)), boxed(self.bool(d - 1)));
        }
        if r < 0.65 {
            return Expr::Or(boxed(self.bool(d - 1)), boxed(self.bool(d - 1)));
        }
        if r < 0.78 {
            return Expr::Not(boxed(self.bool(d - 1)));
        }
        if r < 0.9 {
            return Expr::TracedBool(self.rng.range(10, 99), self.rng.chance(0.5));
        }
        if r < 0.95 {
            return Expr::Bool(self.rng.chance(0.5));
        }
        let n = Expr::Len(boxed(self.list(d - 1)));
        Expr::Cmp("==", boxed(n), boxed(Expr::Int(self.rng.range(0, 2))))
    }

    fn expr(&mut self, d: i32, ty: Ty) -> Expr {
        match ty {
            Ty::List => self.list(d),
            _ => self.int(d),
        }
    }

    /// An arm's body, in the block that holds the names its pattern
    /// bound: they are read first if nothing else reads them.
    fn arm(&mut self, d: i32, ty: Ty, pat: Pat, guard: Option<Expr>) -> Arm {
        let mut body = self.block(d - 1, ty, false);
        let scope = self.scopes.pop().expect("the arm's scope");
        let mut stmts = self.reads(&scope);
        stmts.append(&mut body.stmts);
        body.stmts = stmts;
        Arm { pat, guard, body }
    }

    fn bind(&mut self, ty: Ty) -> String {
        let name = self.fresh("v");
        self.scopes
            .last_mut()
            .expect("a scope")
            .push((name.clone(), ty));
        name
    }

    fn match_int(&mut self, d: i32, ty: Ty) -> Expr {
        let subject = self.int(d - 1);
        let n = self.rng.range(1, 4);
        let mut arms = Vec::new();
        // The values an earlier arm without a guard takes: an arm they
        // cover never runs (E0235, E0241; D112), so it becomes a guarded
        // name instead.
        let mut taken: HashSet<i64> = HashSet::new();
        for i in 0..n {
            let r = self.rng.unit();
            self.scopes.push(Vec::new());
            let (pat, guard);
            if i == n - 1 && self.rng.chance(0.9) {
                pat = if self.rng.chance(0.5) {
                    Pat::Bind(self.bind(Ty::Int))
                } else {
                    Pat::Wild
                };
                guard = None;
            } else {
                let p = if r < 0.35 {
                    Pat::Lit(self.rng.range(0, 4))
                } else if r < 0.6 {
                    let first = self.rng.range(0, 3);
                    Pat::Alt((0..self.rng.range(2, 3)).map(|k| first + k).collect())
                } else if r < 0.85 {
                    Pat::Bind(self.bind(Ty::Int))
                } else {
                    Pat::Wild
                };
                let values = match &p {
                    Pat::Lit(v) => vec![*v],
                    Pat::Alt(vs) => vs.clone(),
                    _ => Vec::new(),
                };
                pat = if values.iter().any(|v| taken.contains(v)) {
                    Pat::Bind(self.bind(Ty::Int))
                } else {
                    p
                };
                let open = matches!(pat, Pat::Bind(_) | Pat::Wild);
                guard = (open || self.rng.chance(0.5)).then(|| self.bool(d - 2));
                if guard.is_none() {
                    taken.extend(values);
                }
            }
            arms.push(self.arm(d, ty, pat, guard));
        }
        Expr::Match(boxed(subject), arms)
    }

    fn match_list(&mut self, d: i32, ty: Ty) -> Expr {
        let subject = self.list(d - 1);
        let n = self.rng.range(1, 4);
        let mut arms = Vec::new();
        // The shapes of the earlier arms without a guard: an arm one of
        // them covers never runs (E0235, E0241; D112), so it becomes a
        // guarded name instead. `[h, ...]` covers every non-empty shape,
        // `[a, b]` and `[a] | [_, a] | …` each other.
        let (mut empty, mut cons, mut some) = (false, false, false);
        for i in 0..n {
            self.scopes.push(Vec::new());
            let mut guard = None;
            let pat;
            if i == n - 1 {
                pat = if self.rng.chance(0.5) {
                    Pat::Bind(self.bind(Ty::List))
                } else {
                    Pat::Wild
                };
            } else {
                let r = self.rng.unit();
                // decided before any name is bound
                let covered = if r < 0.2 {
                    empty
                } else if r < 0.5 {
                    cons
                } else if r < 0.9 {
                    cons || some
                } else {
                    false
                };
                pat = if covered || r >= 0.9 {
                    Pat::Bind(self.bind(Ty::List))
                } else if r < 0.2 {
                    Pat::Empty
                } else if r < 0.5 {
                    let head = self.bind(Ty::Int);
                    let rest = self.rng.chance(0.6).then(|| self.bind(Ty::List));
                    Pat::Cons(head, rest)
                } else if r < 0.7 {
                    Pat::Two(self.bind(Ty::Int), self.bind(Ty::Int))
                } else {
                    Pat::Last(self.bind(Ty::Int))
                };
                if matches!(pat, Pat::Bind(_)) || self.rng.chance(0.5) {
                    guard = Some(self.bool(d - 2));
                }
                if guard.is_none() {
                    match pat {
                        Pat::Empty => empty = true,
                        Pat::Cons(..) => cons = true,
                        Pat::Two(..) | Pat::Last(_) => some = true,
                        _ => {}
                    }
                }
            }
            arms.push(self.arm(d, ty, pat, guard));
        }
        Expr::Match(boxed(subject), arms)
    }

    fn traced(&mut self, d: i32) -> Stmt {
        Stmt::Expr(Expr::Traced(boxed(self.int(d - 1))))
    }

    fn stmts(&mut self, d: i32, n: i64) -> Vec<Stmt> {
        let mut out = Vec::new();
        for _ in 0..n {
            let r = self.rng.unit();
            let values: Vec<(String, Ty)> = self
                .scopes
                .last()
                .expect("a scope")
                .iter()
                .filter(|v| v.1 != Ty::Lam)
                .cloned()
                .collect();
            if r < 0.3 {
                let ty = *self.rng.pick(&[Ty::Int, Ty::Int, Ty::List]);
                let e = self.expr(d, ty);
                out.push(Stmt::Let(self.bind(ty), e));
            } else if r < 0.45 && !values.is_empty() {
                // The name is assigned again, in its own block.
                let (name, ty) = self.rng.pick(&values).clone();
                out.push(Stmt::Let(name, self.expr(d, ty)));
            } else if r < 0.58 {
                self.in_lambda += 1;
                let e = self.int(d - 1);
                self.in_lambda -= 1;
                let name = self.fresh("g");
                self.scopes
                    .last_mut()
                    .expect("a scope")
                    .push((name.clone(), Ty::Lam));
                out.push(Stmt::Lam(name, e));
            } else if !(0.72..0.9).contains(&r) {
                out.push(self.traced(d));
            } else {
                let c = self.bool(d - 1);
                self.scopes.push(Vec::new());
                let n = self.rng.range(0, 2);
                let mut inner = self.stmts(d - 1, n);
                let last = if self.in_lambda == 0 && self.rng.chance(0.35) {
                    Stmt::Return(self.int(d - 1))
                } else {
                    self.traced(d)
                };
                let scope = self.scopes.pop().expect("the branch's scope");
                inner.extend(self.reads(&scope));
                inner.push(last);
                out.push(Stmt::If(c, inner));
            }
        }
        out
    }

    fn block(&mut self, d: i32, ty: Ty, tail_call: bool) -> Block {
        self.scopes.push(Vec::new());
        let n = if d > 0 {
            *self.rng.pick(&[0, 0, 0, 1, 1, 2, 3])
        } else {
            0
        };
        let mut stmts = self.stmts(d - 1, n);
        let value = if self.in_lambda == 0 && self.rng.chance(0.08) {
            Expr::Return(boxed(self.int(d - 1)))
        } else if tail_call && !self.funcs.is_empty() && self.rng.chance(0.5) {
            self.call(d)
        } else {
            self.expr(d, ty)
        };
        let scope = self.scopes.pop().expect("the block's scope");
        stmts.extend(self.reads(&scope));
        Block { stmts, value }
    }
}

fn sig(f: &Func) -> Sig {
    let params = f.params.iter().zip(&f.defaults);
    let params = params.map(|(p, d)| (p.clone(), d.is_some())).collect();
    (f.name.clone(), params)
}

/// A top-level call of one of the functions: positional arguments, then
/// named ones.
type TopCall = (usize, Vec<i64>, Vec<(String, i64)>);

fn program(rng: &mut Rng) -> (Vec<Func>, Vec<TopCall>) {
    let mut funcs: Vec<Func> = Vec::new();
    for i in 0..rng.range(1, 4) {
        let mut g = Gen {
            funcs: funcs.iter().map(sig).collect(),
            rng,
            scopes: Vec::new(),
            in_lambda: 0,
            used: HashSet::new(),
            names: 0,
        };
        // The parameters, the last ones with defaults: a default sees the
        // parameters before it and binds names of its own in its `match`
        // arms and `if` blocks (VM48). It cannot `return` (E0220).
        let required = g.rng.range(1, 3);
        let optional = *g.rng.pick(&[0, 0, 1, 1, 2]);
        let (mut params, mut defaults) = (Vec::new(), Vec::new());
        g.scopes.push(Vec::new());
        for k in 0..required + optional {
            let default = (k >= required).then(|| {
                g.in_lambda += 1;
                let e = g.int(2);
                g.in_lambda -= 1;
                e
            });
            let p = g.fresh("p");
            g.scopes
                .last_mut()
                .expect("the parameters")
                .push((p.clone(), Ty::Int));
            params.push(p);
            defaults.push(default);
        }
        let depth = g.rng.range(2, 4) as i32;
        let mut body = g.block(depth, Ty::Int, true);
        let scope = g.scopes.pop().expect("the parameters");
        let mut stmts = g.reads(&scope);
        stmts.append(&mut body.stmts);
        body.stmts = stmts;
        let typed = g.rng.chance(0.4);
        funcs.push(Func {
            name: format!("f{i}"),
            params,
            defaults,
            body,
            typed,
        });
    }
    let mut calls = Vec::new();
    for (i, f) in funcs.iter().enumerate() {
        for _ in 0..3 {
            let (mut positional, mut named) = (Vec::new(), Vec::new());
            let mut by_name = false;
            for (p, d) in f.params.iter().zip(&f.defaults) {
                let r = rng.unit();
                if d.is_some() && r < 0.4 {
                    by_name = true;
                    continue;
                }
                by_name |= d.is_some() && r < 0.7;
                let v = rng.range(0, 4);
                if by_name {
                    named.push((p.clone(), v));
                } else {
                    positional.push(v);
                }
            }
            calls.push((i, positional, named));
        }
    }
    (funcs, calls)
}

// ---------- the printer ----------

fn list_of<T>(xs: &[T], f: impl Fn(&T) -> String) -> String {
    xs.iter().map(f).collect::<Vec<_>>().join(", ")
}

fn show_expr(e: &Expr) -> String {
    match e {
        Expr::Int(n) => n.to_string(),
        Expr::Var(v) => v.clone(),
        Expr::Traced(e) => format!("t({})", show_expr(e)),
        Expr::Add(a, b) => format!("({} + {})", show_expr(a), show_expr(b)),
        Expr::Sub(a, b) => format!("({} - {})", show_expr(a), show_expr(b)),
        Expr::If(c, a, b) => format!(
            "if {}:\n{}\nelse:\n{}\n;;",
            show_expr(c),
            show_block(a),
            show_block(b)
        ),
        Expr::Then(c, a, b) => format!(
            "({} then {} else {})",
            show_expr(c),
            show_expr(a),
            show_expr(b)
        ),
        Expr::Match(subject, arms) => {
            let mut s = format!("match {}:\n", show_expr(subject));
            for arm in arms {
                s += &show_pat(&arm.pat);
                if let Some(g) = &arm.guard {
                    write!(s, " when {}", show_expr(g)).unwrap();
                }
                writeln!(s, " =>\n{}", show_block(&arm.body)).unwrap();
            }
            s + ";;"
        }
        Expr::Coalesce(a, b) => format!("(m({}) ?? {})", show_expr(a), show_expr(b)),
        Expr::Call(f, args, named) => {
            let mut all: Vec<String> = args.iter().map(show_expr).collect();
            all.extend(named.iter().map(|(k, e)| format!("{k}: {}", show_expr(e))));
            format!("{f}({})", all.join(", "))
        }
        Expr::LamCall(f) => format!("{f}()"),
        Expr::Len(e) => format!("len({})", show_expr(e)),
        Expr::List(xs) => format!("[{}]", list_of(xs, show_expr)),
        Expr::Concat(a, b) => format!("({} + {})", show_expr(a), show_expr(b)),
        Expr::Cmp(op, a, b) => format!("({} {op} {})", show_expr(a), show_expr(b)),
        Expr::And(a, b) => format!("({} and {})", show_expr(a), show_expr(b)),
        Expr::Or(a, b) => format!("({} or {})", show_expr(a), show_expr(b)),
        Expr::Not(e) => format!("(not {})", show_expr(e)),
        Expr::TracedBool(k, b) => format!("tb({k}, {b})"),
        Expr::Bool(b) => b.to_string(),
        Expr::Return(e) => format!("return {}", show_expr(e)),
    }
}

fn show_pat(p: &Pat) -> String {
    match p {
        Pat::Lit(n) => n.to_string(),
        Pat::Alt(ns) => ns
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(" | "),
        Pat::Bind(n) => n.clone(),
        Pat::Wild => "_".into(),
        Pat::Empty => "[]".into(),
        Pat::Cons(h, rest) => format!("[{h}, ...{}]", rest.as_deref().unwrap_or("")),
        Pat::Two(a, b) => format!("[{a}, {b}]"),
        Pat::Last(a) => format!("[{a}] | [_, {a}] | [_, _, {a}]"),
    }
}

fn show_stmt(s: &Stmt) -> String {
    match s {
        Stmt::Let(n, e) => format!("{n} = {}", show_expr(e)),
        Stmt::Lam(n, e) => format!("{n} = () => {}", show_expr(e)),
        Stmt::Expr(e) => show_expr(e),
        Stmt::If(c, body) => format!(
            "if {}:\n{}\n;;",
            show_expr(c),
            body.iter().map(show_stmt).collect::<Vec<_>>().join("\n")
        ),
        Stmt::Return(e) => format!("return {}", show_expr(e)),
    }
}

fn show_block(b: &Block) -> String {
    let mut lines: Vec<String> = b.stmts.iter().map(show_stmt).collect();
    lines.push(show_expr(&b.value));
    lines.join("\n")
}

fn source(funcs: &[Func], calls: &[TopCall]) -> String {
    let mut s = String::from(
        "fn t(x):\n  print(x)\n  x\n;;\nfn tb(k, b):\n  print(k)\n  b\n;;\n\
         fn m(x) = x % 2 == 0 then nil else x\n",
    );
    for f in funcs {
        let ret = if f.typed { " -> int" } else { "" };
        let params = f.params.iter().zip(&f.defaults).map(|(p, d)| match d {
            Some(d) => format!("{p} = ({})", show_expr(d)),
            None => p.clone(),
        });
        writeln!(
            s,
            "fn {}({}){ret}:\n{}\n;;",
            f.name,
            params.collect::<Vec<_>>().join(", "),
            show_block(&f.body)
        )
        .unwrap();
    }
    for (i, args, named) in calls {
        let mut all: Vec<String> = args.iter().map(i64::to_string).collect();
        all.extend(named.iter().map(|(k, v)| format!("{k}: {v}")));
        writeln!(s, "print({}({}))", funcs[*i].name, all.join(", ")).unwrap();
    }
    s
}

// ---------- the reference evaluator ----------

#[derive(Clone)]
enum Val {
    Int(i64),
    List(Vec<i64>),
    Bool(bool),
    /// A lambda and the names it copied when it was made.
    Lam(Rc<Expr>, Rc<Env>),
}

type Env = HashMap<String, Val>;

/// How an evaluation stops early.
enum Stop {
    Return(Val),
    /// A runtime error, by its code.
    Error(&'static str),
    TooLong,
}

struct Eval<'a> {
    funcs: &'a [Func],
    out: Vec<String>,
    calls: usize,
}

fn show_val(v: &Val) -> String {
    match v {
        Val::Int(n) => n.to_string(),
        Val::Bool(b) => b.to_string(),
        Val::List(xs) => format!("[{}]", list_of(xs, i64::to_string)),
        Val::Lam(..) => unreachable!("a lambda is never printed"),
    }
}

impl Eval<'_> {
    fn int(&mut self, e: &Expr, env: &mut Env) -> Result<i64, Stop> {
        match self.expr(e, env)? {
            Val::Int(n) => Ok(n),
            _ => unreachable!("the generator made an int here"),
        }
    }

    fn list(&mut self, e: &Expr, env: &mut Env) -> Result<Vec<i64>, Stop> {
        match self.expr(e, env)? {
            Val::List(xs) => Ok(xs),
            _ => unreachable!("the generator made a list here"),
        }
    }

    fn bool(&mut self, e: &Expr, env: &mut Env) -> Result<bool, Stop> {
        match self.expr(e, env)? {
            Val::Bool(b) => Ok(b),
            _ => unreachable!("the generator made a bool here"),
        }
    }

    /// Whether `pat` takes `v`, binding its names; a guard is tried for
    /// each alternative that fits.
    fn fits(&mut self, arm: &Arm, v: &Val, env: &mut Env) -> Result<bool, Stop> {
        let guard = |me: &mut Self, env: &mut Env| match &arm.guard {
            Some(g) => me.bool(g, env),
            None => Ok(true),
        };
        match (&arm.pat, v) {
            (Pat::Lit(n), Val::Int(x)) if n != x => return Ok(false),
            (Pat::Alt(ns), Val::Int(x)) if !ns.contains(x) => return Ok(false),
            (Pat::Empty, Val::List(xs)) if !xs.is_empty() => return Ok(false),
            (Pat::Cons(h, rest), Val::List(xs)) => {
                let Some(first) = xs.first() else {
                    return Ok(false);
                };
                env.insert(h.clone(), Val::Int(*first));
                if let Some(r) = rest {
                    env.insert(r.clone(), Val::List(xs[1..].to_vec()));
                }
            }
            (Pat::Two(a, b), Val::List(xs)) => {
                if xs.len() != 2 {
                    return Ok(false);
                }
                env.insert(a.clone(), Val::Int(xs[0]));
                env.insert(b.clone(), Val::Int(xs[1]));
            }
            (Pat::Last(a), Val::List(xs)) => {
                if !(1..=3).contains(&xs.len()) {
                    return Ok(false);
                }
                env.insert(a.clone(), Val::Int(xs[xs.len() - 1]));
            }
            (Pat::Bind(n), v) => {
                env.insert(n.clone(), v.clone());
            }
            _ => {}
        }
        guard(self, env)
    }

    fn expr(&mut self, e: &Expr, env: &mut Env) -> Result<Val, Stop> {
        Ok(match e {
            Expr::Int(n) => Val::Int(*n),
            Expr::Var(v) => env[v].clone(),
            Expr::Traced(e) => {
                let v = self.expr(e, env)?;
                self.out.push(show_val(&v));
                v
            }
            Expr::Add(a, b) => Val::Int(self.int(a, env)? + self.int(b, env)?),
            Expr::Sub(a, b) => Val::Int(self.int(a, env)? - self.int(b, env)?),
            Expr::If(c, a, b) => {
                if self.bool(c, env)? {
                    self.block(a, env)?
                } else {
                    self.block(b, env)?
                }
            }
            Expr::Then(c, a, b) => {
                if self.bool(c, env)? {
                    self.expr(a, env)?
                } else {
                    self.expr(b, env)?
                }
            }
            Expr::Match(subject, arms) => {
                let v = self.expr(subject, env)?;
                for arm in arms {
                    if self.fits(arm, &v, env)? {
                        return self.block(&arm.body, env);
                    }
                }
                return Err(Stop::Error("E0601"));
            }
            Expr::Coalesce(a, b) => {
                let v = self.int(a, env)?;
                if v.rem_euclid(2) == 0 {
                    self.expr(b, env)?
                } else {
                    Val::Int(v)
                }
            }
            Expr::Call(f, args, named) => {
                let mut vals = Vec::new();
                for a in args {
                    vals.push(self.int(a, env)?);
                }
                let mut by_name = Vec::new();
                for (k, e) in named {
                    by_name.push((k.clone(), self.int(e, env)?));
                }
                let i = self.funcs.iter().position(|x| &x.name == f).expect("fn");
                Val::Int(self.call(i, &vals, &by_name)?)
            }
            Expr::LamCall(f) => {
                let Val::Lam(body, captured) = env[f].clone() else {
                    unreachable!("the generator called a lambda here");
                };
                self.expr(&body, &mut (*captured).clone())?
            }
            Expr::Len(e) => Val::Int(self.list(e, env)?.len() as i64),
            Expr::List(xs) => {
                let mut vals = Vec::new();
                for x in xs {
                    vals.push(self.int(x, env)?);
                }
                Val::List(vals)
            }
            Expr::Concat(a, b) => {
                let mut xs = self.list(a, env)?;
                xs.extend(self.list(b, env)?);
                Val::List(xs)
            }
            Expr::Cmp(op, a, b) => {
                let (a, b) = (self.int(a, env)?, self.int(b, env)?);
                Val::Bool(match *op {
                    "<" => a < b,
                    "==" => a == b,
                    ">" => a > b,
                    "!=" => a != b,
                    "<=" => a <= b,
                    _ => a >= b,
                })
            }
            Expr::And(a, b) => Val::Bool(self.bool(a, env)? && self.bool(b, env)?),
            Expr::Or(a, b) => Val::Bool(self.bool(a, env)? || self.bool(b, env)?),
            Expr::Not(e) => Val::Bool(!self.bool(e, env)?),
            Expr::TracedBool(k, b) => {
                self.out.push(k.to_string());
                Val::Bool(*b)
            }
            Expr::Bool(b) => Val::Bool(*b),
            Expr::Return(e) => return Err(Stop::Return(self.expr(e, env)?)),
        })
    }

    fn stmt(&mut self, s: &Stmt, env: &mut Env) -> Result<(), Stop> {
        match s {
            Stmt::Let(n, e) => {
                let v = self.expr(e, env)?;
                env.insert(n.clone(), v);
            }
            // A function value copies the names it sees when it is made.
            Stmt::Lam(n, e) => {
                let made = Val::Lam(Rc::new(e.clone()), Rc::new(env.clone()));
                env.insert(n.clone(), made);
            }
            Stmt::Expr(e) => {
                self.expr(e, env)?;
            }
            Stmt::If(c, body) => {
                if self.bool(c, env)? {
                    for s in body {
                        self.stmt(s, env)?;
                    }
                }
            }
            Stmt::Return(e) => return Err(Stop::Return(self.expr(e, env)?)),
        }
        Ok(())
    }

    fn block(&mut self, b: &Block, env: &mut Env) -> Result<Val, Stop> {
        for s in &b.stmts {
            self.stmt(s, env)?;
        }
        self.expr(&b.value, env)
    }

    /// Calls function `i`: the arguments given by position and by name
    /// are bound first, then each missing one's default runs, in order.
    fn call(&mut self, i: usize, args: &[i64], named: &[(String, i64)]) -> Result<i64, Stop> {
        self.calls += 1;
        if self.calls > MAX_CALLS {
            return Err(Stop::TooLong);
        }
        let f = &self.funcs[i];
        let mut env: Env = f
            .params
            .iter()
            .cloned()
            .zip(args.iter().map(|&a| Val::Int(a)))
            .collect();
        for (k, v) in named {
            env.insert(k.clone(), Val::Int(*v));
        }
        for (p, d) in f.params.iter().zip(&f.defaults) {
            if let Some(d) = d
                && !env.contains_key(p)
            {
                let v = self.expr(d, &mut env)?;
                env.insert(p.clone(), v);
            }
        }
        match self.block(&f.body, &mut env) {
            Ok(Val::Int(n)) | Err(Stop::Return(Val::Int(n))) => Ok(n),
            Ok(_) | Err(Stop::Return(_)) => unreachable!("a function gives an int"),
            Err(stop) => Err(stop),
        }
    }
}

/// What the program must print and the error it must end with, or
/// `None` when it runs too long to be worth it.
fn reference(funcs: &[Func], calls: &[TopCall]) -> Option<(Vec<String>, Option<&'static str>)> {
    let mut ev = Eval {
        funcs,
        out: Vec::new(),
        calls: 0,
    };
    for (i, args, named) in calls {
        match ev.call(*i, args, named) {
            Ok(n) => ev.out.push(n.to_string()),
            Err(Stop::Error(code)) => return Some((ev.out, Some(code))),
            Err(Stop::TooLong) => return None,
            Err(Stop::Return(_)) => unreachable!("a call takes the return"),
        }
    }
    Some((ev.out, None))
}

/// The code of the first `error[E….]` in a report.
fn error_code(err: &str) -> Option<&str> {
    let at = err.find("error[")? + "error[".len();
    err[at..].split(']').next()
}

#[test]
fn generated_programs_print_what_the_reference_evaluator_says() {
    let cases = env_num("FUNG_GEN_CASES", DEFAULT_CASES as u64) as usize;
    let seed = env_num("FUNG_GEN_SEED", DEFAULT_SEED);
    let (ran, skipped) = on_big_stack(move || {
        let mut rng = Rng::new(seed);
        let (mut ran, mut skipped) = (0, 0);
        for case in 0..cases {
            let (funcs, calls) = program(&mut rng);
            let src = source(&funcs, &calls);
            let fail = |what: String| -> ! {
                panic!(
                    "generated program {case} (FUNG_GEN_SEED={seed}): {what}\n--- program\n{src}"
                )
            };
            let Some((want_out, want_err)) = reference(&funcs, &calls) else {
                skipped += 1;
                continue;
            };
            let (out, err, exit) = run(&src);
            let got_err = error_code(&err);
            if exit == 2 {
                // The generator's own mistake (a dead arm), unless the
                // compiler says it is the compiler's.
                if got_err == Some("E0801") {
                    fail(format!("the compiler failed\n{err}"));
                }
                skipped += 1;
                continue;
            }
            let got_out: Vec<&str> = out.lines().collect();
            if got_out != want_out || got_err != want_err {
                let at = got_out
                    .iter()
                    .zip(&want_out)
                    .position(|(g, w)| g != w)
                    .unwrap_or(got_out.len().min(want_out.len()));
                fail(format!(
                    "printed line {at} is {:?}, the reference says {:?}; \
                     ended with {got_err:?}, the reference says {want_err:?}\n{err}",
                    got_out.get(at),
                    want_out.get(at)
                ));
            }
            ran += 1;
        }
        (ran, skipped)
    });
    assert!(
        ran * 10 >= cases * 7,
        "only {ran} of {cases} generated programs ran ({skipped} skipped): the generator drifted"
    );
}
