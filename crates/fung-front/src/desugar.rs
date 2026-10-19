//! Rewrites the surface tree the parser builds into the smaller core
//! language the compiler understands (09. 코어 언어). Both are `Node`
//! trees; this pass only changes heads and shapes. Sugar that needs
//! temporaries (match subjects, partial application) uses hidden names such
//! as `$m0` that no identifier can spell.
//!
//! The input is borrowed and never modified. Recursion follows tree depth,
//! so callers with very deep trees need a large stack (see `parser`).

use crate::diag::{Arg, Diag, error};
use crate::source::Pos;
use crate::tree::{FnInfo, Head, Lit, Node, Param};

/// Errors are boxed to keep the recursive frames small.
type R = Result<Node, Box<Diag>>;

pub fn desugar(n: &Node) -> Result<Node, Diag> {
    let mut d = Desugarer { counter: 0 };
    d.walk(n).map_err(|e| *e)
}

struct Desugarer {
    counter: usize,
}

fn s(text: &str) -> Lit {
    Lit::Str(text.to_string())
}

fn name_leaf(pos: Pos, text: &str) -> Node {
    Node::leaf(Head::Name, pos, s(text))
}

fn lit_str(n: &Node) -> String {
    match &n.lit {
        Lit::Str(t) => t.clone(),
        _ => String::new(),
    }
}

fn fn_info(n: &Node) -> &FnInfo {
    match &n.lit {
        Lit::Fn(f) => f,
        _ => unreachable!("fn node without FnInfo"),
    }
}

fn blank_info(name: &str) -> FnInfo {
    FnInfo {
        name: name.to_string(),
        params: Vec::new(),
        ret: None,
        for_body: false,
        num_locals: 0,
        captures: Vec::new(),
    }
}

fn plain_param(name: String, pos: Pos) -> Param {
    Param {
        name,
        pos,
        types: None,
        has_default: false,
        rest: false,
        pattern: None,
    }
}

/// Wraps an expression whose value must be a bool at runtime.
fn check_bool(e: Node) -> Node {
    Node::with_lit(Head::Op, e.pos, s("bool"), vec![e])
}

impl Desugarer {
    /// Returns a fresh hidden variable name.
    fn hidden(&mut self, prefix: &str) -> String {
        let name = format!("${prefix}{}", self.counter);
        self.counter += 1;
        name
    }

    /// Lowers one node; a rewritten node inherits the source span of the node
    /// it came from (assert and dbg show source text, 08).
    fn walk(&mut self, n: &Node) -> R {
        let mut out = self.walk_node(n)?;
        if out.end == 0 {
            out.off = n.off;
            out.end = n.end;
        }
        Ok(out)
    }

    fn walk_node(&mut self, n: &Node) -> R {
        match n.head {
            Head::Program | Head::Block => self.map_kids(Head::Do, n),
            Head::Assign => {
                let rhs = self.walk(&n.kids[1])?;
                Ok(Node::new(Head::Let, n.pos, vec![n.kids[0].clone(), rhs]))
            }
            Head::Binary => {
                let a = self.walk(&n.kids[0])?;
                let b = self.walk(&n.kids[1])?;
                let op = lit_str(n);
                Ok(match op.as_str() {
                    // a and b  ->  if a then (b must be bool) else false
                    "and" => Node::new(
                        Head::If,
                        n.pos,
                        vec![
                            a,
                            check_bool(b),
                            Node::leaf(Head::Bool, n.pos, Lit::Bool(false)),
                        ],
                    ),
                    // a or b  ->  if a then true else (b must be bool)
                    "or" => Node::new(
                        Head::If,
                        n.pos,
                        vec![
                            a,
                            Node::leaf(Head::Bool, n.pos, Lit::Bool(true)),
                            check_bool(b),
                        ],
                    ),
                    "not_in" => Node::with_lit(
                        Head::Op,
                        n.pos,
                        s("not"),
                        vec![Node::with_lit(Head::Op, n.pos, s("in"), vec![a, b])],
                    ),
                    _ => Node::with_lit(Head::Op, n.pos, n.lit.clone(), vec![a, b]),
                })
            }
            Head::Unary => {
                let a = self.walk(&n.kids[0])?;
                Ok(Node::with_lit(Head::Op, n.pos, n.lit.clone(), vec![a]))
            }
            Head::Then | Head::If => self.map_kids(Head::If, n),
            Head::Interp => self.map_kids(Head::Concat, n),
            Head::Pipe => self.pipe(n),
            Head::Fn => self.fn_node(n),
            Head::Test => {
                // A test block is a function made where it stands (08): it
                // copies the variables visible at that point and can fail on
                // its own.
                let body = self.walk(&n.kids[0])?;
                let info = blank_info(&format!("test: {}", lit_str(n)));
                let f = Node::with_lit(Head::Fn, n.pos, Lit::Fn(Box::new(info)), vec![body]);
                Ok(Node::with_lit(Head::Test, n.pos, n.lit.clone(), vec![f]))
            }
            Head::Assert => {
                let e = self.walk(&n.kids[0])?;
                Ok(Node::new(Head::Assert, n.pos, vec![e]))
            }
            Head::Call => {
                let kids: Vec<&Node> = n.kids.iter().collect();
                self.call(n.pos, &kids)
            }
            Head::For => self.for_loop(n),
            Head::Match => self.match_expr(n),
            Head::Return
            | Head::Named
            | Head::List
            | Head::Map
            | Head::Entry
            | Head::Tuple
            | Head::Tag
            | Head::Range
            | Head::Index
            | Head::Slice
            | Head::Field
            | Head::Unwrap
            | Head::Coalesce
            | Head::Arm
            | Head::Hole => self.map_kids(n.head, n),
            Head::Int
            | Head::Float
            | Head::Bool
            | Head::Name
            | Head::Nil
            | Head::String
            | Head::Symbol
            | Head::Duration
            | Head::Typealias
            | Head::Import
            | Head::Pwild
            | Head::Pbind
            | Head::Plit
            | Head::Pnil
            | Head::Psym
            | Head::Ptag
            | Head::Ptuple
            | Head::Plist
            | Head::Prest
            | Head::Pmap
            | Head::Pentry
            | Head::Palt => Ok(n.clone()),
            _ => Err(Box::new(error(
                "E0801",
                n.pos,
                vec![Arg::Str(format!(
                    "desugar: unknown node {}",
                    n.head.as_str()
                ))],
            ))),
        }
    }

    /// Builds a node with the given head and the same lit whose children are
    /// the desugared children of `n`.
    fn map_kids(&mut self, head: Head, n: &Node) -> R {
        let mut kids = Vec::with_capacity(n.kids.len());
        for k in &n.kids {
            kids.push(self.walk(k)?);
        }
        Ok(Node::with_lit(head, n.pos, n.lit.clone(), kids))
    }

    /// A call whose operands are `kids` (callee first): a hole among the
    /// arguments makes it a partial application.
    fn call(&mut self, pos: Pos, kids: &[&Node]) -> R {
        if kids[1..].iter().any(|k| k.head == Head::Hole) {
            return self.partial(pos, kids);
        }
        let mut out = Vec::with_capacity(kids.len());
        for k in kids {
            out.push(self.walk(k)?);
        }
        Ok(Node::new(Head::Call, pos, out))
    }

    /// Lowers `x |> f(a)` to `f(x, a)` and `x |> f` to `f(x)` (01).
    fn pipe(&mut self, n: &Node) -> R {
        let rhs = &n.kids[1];
        if rhs.head == Head::Call {
            // Insert the surface operand and walk the rebuilt call once: a
            // pre-walked operand would be walked again when pipes chain.
            // The operands are borrowed, so long chains are not copied.
            let mut kids: Vec<&Node> = Vec::with_capacity(rhs.kids.len() + 1);
            kids.push(&rhs.kids[0]);
            kids.push(&n.kids[0]);
            kids.extend(rhs.kids[1..].iter());
            return self.call(rhs.pos, &kids);
        }
        let x = self.walk(&n.kids[0])?;
        let f = self.walk(rhs)?;
        Ok(Node::new(Head::Call, n.pos, vec![f, x]))
    }

    /// Lowers `f(a, _, b)` to a block that evaluates f, a and b once into
    /// hidden variables and returns a lambda over the holes (03):
    ///
    /// `(do (let $p0 f) (let $p1 a) (let $p2 b) (fn (h0) (do (call $p0 $p1 h0 $p2))))`
    fn partial(&mut self, pos: Pos, kids: &[&Node]) -> R {
        let mut block = Vec::new();
        let mut info = blank_info("");
        let mut call = Vec::new();
        let mut holes = 0;
        for k in kids {
            match k.head {
                Head::Hole => {
                    let name = format!("h{holes}");
                    holes += 1;
                    call.push(name_leaf(k.pos, &name));
                    info.params.push(plain_param(name, k.pos));
                }
                Head::Named => {
                    let v = self.walk(&k.kids[0])?;
                    let tmp = self.hidden("p");
                    block.push(Node::new(Head::Let, k.pos, vec![name_leaf(k.pos, &tmp), v]));
                    call.push(Node::with_lit(
                        Head::Named,
                        k.pos,
                        k.lit.clone(),
                        vec![name_leaf(k.pos, &tmp)],
                    ));
                }
                _ => {
                    let v = self.walk(k)?;
                    let tmp = self.hidden("p");
                    block.push(Node::new(Head::Let, k.pos, vec![name_leaf(k.pos, &tmp), v]));
                    call.push(name_leaf(k.pos, &tmp));
                }
            }
        }
        let body = Node::new(Head::Do, pos, vec![Node::new(Head::Call, pos, call)]);
        block.push(Node::with_lit(
            Head::Fn,
            pos,
            Lit::Fn(Box::new(info)),
            vec![body],
        ));
        Ok(Node::new(Head::Do, pos, block))
    }

    /// Lowers `for pat in e: body ;;` to `map(e, fn(pat): body ;;)` with the
    /// prelude's map, whatever the user has named `map` (01, 02). The lambda
    /// is marked for_body so a return inside it is E0213 (03).
    fn for_loop(&mut self, n: &Node) -> R {
        let (pat, iter, body) = (&n.kids[0], &n.kids[1], &n.kids[2]);
        let iter_c = self.walk(iter)?;
        let mut body_c = self.walk(body)?;
        let mut info = blank_info("");
        info.for_body = true;
        if pat.head == Head::Pbind {
            info.params = vec![plain_param(lit_str(pat), pat.pos)];
        } else {
            let tmp = self.hidden("f");
            info.params = vec![plain_param(tmp.clone(), pat.pos)];
            // Lit "fresh": the pattern's names shadow instead of assigning (03).
            let destructure = Node::with_lit(
                Head::Let,
                pat.pos,
                s("fresh"),
                vec![pat.clone(), name_leaf(pat.pos, &tmp)],
            );
            body_c.kids.insert(0, destructure);
        }
        let lambda = Node::with_lit(Head::Fn, n.pos, Lit::Fn(Box::new(info)), vec![body_c]);
        Ok(Node::new(
            Head::Call,
            n.pos,
            vec![Node::leaf(Head::Prelude, n.pos, s("map")), iter_c, lambda],
        ))
    }

    /// Lowers a function: defaults and body are walked, and every
    /// destructuring parameter (03) gets a hidden name `$aN` whose value is
    /// matched at the top of the body with fresh (shadowing) bindings.
    /// Defaults run before the body, so a default that uses names of an
    /// earlier pattern parameter matches that pattern first, in its own
    /// block, binding only the names it uses (03: a default sees the
    /// parameters before it).
    fn fn_node(&mut self, n: &Node) -> R {
        let mut info = fn_info(n).clone();
        let mut kids = Vec::with_capacity(n.kids.len());
        for k in &n.kids {
            kids.push(self.walk(k)?);
        }
        let mut prologue = Vec::new();
        let mut earlier: Vec<(Node, String)> = Vec::new();
        let mut default_ix = 0;
        // Names only defaults read are bound there, not again in the body
        // (where the copy would be an unused variable, W0102).
        let mut in_body = Vec::new();
        names_in(kids.last().expect("fn has a body"), &mut in_body);
        let mut in_defaults = Vec::new();
        for d in &kids[..kids.len() - 1] {
            names_in(d, &mut in_defaults);
        }
        let only_defaults: Vec<String> = in_defaults
            .into_iter()
            .filter(|name| !in_body.contains(name))
            .collect();
        for prm in info.params.iter_mut() {
            if prm.has_default {
                let d = &mut kids[default_ix];
                default_ix += 1;
                let mut used = Vec::new();
                names_in(d, &mut used);
                let lets: Vec<Node> = earlier
                    .iter()
                    .filter_map(|(pat, tmp)| keep_names(pat, &used).map(|p| (p, tmp)))
                    .map(|(p, tmp)| {
                        Node::with_lit(Head::Let, d.pos, s("fresh"), vec![p, name_leaf(d.pos, tmp)])
                    })
                    .collect();
                if !lets.is_empty() {
                    let value = std::mem::replace(d, Node::new(Head::Nil, n.pos, Vec::new()));
                    let mut block = lets;
                    block.push(value);
                    *d = Node::new(Head::Do, block[0].pos, block);
                }
            }
            let Some(pattern) = prm.pattern.take() else {
                continue;
            };
            let tmp = self.hidden("a");
            let pos = prm.pos;
            *prm = plain_param(tmp.clone(), pos);
            earlier.push(((*pattern).clone(), tmp.clone()));
            let body_pattern = without_names(&pattern, &only_defaults);
            prologue.push(Node::with_lit(
                Head::Let,
                pos,
                s("fresh"),
                vec![body_pattern, name_leaf(pos, &tmp)],
            ));
        }
        if !prologue.is_empty() {
            let last = kids.last_mut().expect("fn has a body");
            prologue.append(&mut last.kids);
            last.kids = prologue;
        }
        Ok(Node::with_lit(
            Head::Fn,
            n.pos,
            Lit::Fn(Box::new(info)),
            kids,
        ))
    }

    /// Evaluates the subject once into a hidden variable:
    ///
    /// `(do (let $m0 e) (match $m0 (arm pat guard body)...))`
    fn match_expr(&mut self, n: &Node) -> R {
        let subject = self.walk(&n.kids[0])?;
        let tmp = self.hidden("m");
        let mut kids = vec![name_leaf(n.pos, &tmp)];
        for arm in &n.kids[1..] {
            kids.push(self.map_kids(Head::Arm, arm)?);
        }
        let m = Node::new(Head::Match, n.pos, kids);
        Ok(Node::new(
            Head::Do,
            n.pos,
            vec![
                Node::new(Head::Let, n.pos, vec![name_leaf(n.pos, &tmp), subject]),
                m,
            ],
        ))
    }
}

/// The names an expression reads (any depth; a name shadowed inside it
/// still counts, which only binds an unused copy).
fn names_in(n: &Node, out: &mut Vec<String>) {
    if n.head == Head::Name
        && let Lit::Str(name) = &n.lit
    {
        out.push(name.clone());
    }
    for k in &n.kids {
        names_in(k, out);
    }
}

/// A copy of a pattern where the names `drop` says are `_` (a rest keeps
/// its `...` without a name). Reports whether any name was kept.
fn strip_names(n: &Node, drop: &dyn Fn(&str) -> bool, kept: &mut bool) -> Node {
    let mut out = n.clone();
    if matches!(n.head, Head::Pbind | Head::Prest)
        && let Lit::Str(name) = &n.lit
    {
        if !drop(name) {
            *kept = true;
        } else if n.head == Head::Pbind {
            return Node::new(Head::Pwild, n.pos, Vec::new());
        } else {
            out.lit = Lit::None;
        }
    }
    out.kids = n.kids.iter().map(|k| strip_names(k, drop, kept)).collect();
    out
}

/// A copy of a parameter pattern that binds only the names in `used`, or
/// `None` when it binds none of them.
fn keep_names(pat: &Node, used: &[String]) -> Option<Node> {
    let mut kept = false;
    let copy = strip_names(pat, &|name| !used.iter().any(|u| u == name), &mut kept);
    kept.then_some(copy)
}

/// A copy of a parameter pattern without the names in `gone`.
fn without_names(pat: &Node, gone: &[String]) -> Node {
    strip_names(pat, &|name| gone.iter().any(|g| g == name), &mut false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::lex, parser::parse, source::SourceMap};

    fn tree_of(src: &str) -> Node {
        let mut sm = SourceMap::new();
        let f = sm.add("t.fung", src);
        let (toks, _) = lex(f, src).unwrap();
        desugar(&parse(&toks).unwrap()).unwrap()
    }

    fn core(src: &str) -> String {
        tree_of(src).to_string()
    }

    fn check(cases: &[(&str, &str)]) {
        for (src, want) in cases {
            assert_eq!(core(src), *want, "{src:?}");
        }
    }

    #[test]
    fn pipes_and_interpolation() {
        let s = core("xs = [1, 2] |> map(x => x)");
        assert!(s.contains("(call map (list 1 2) (fn (x)"), "{s}");
        let s = core("s = \"hi ${1 + 1}\"");
        assert!(s.contains("(concat \"hi \" (op + 1 1))"), "{s}");
    }

    #[test]
    fn desugar_arithmetic_to_ops() {
        check(&[
            ("1 + 2 * 3", "(do (op + 1 (op * 2 3)))"),
            ("-x", "(do (op - x))"),
            ("not x", "(do (op not x))"),
            ("print(1)", "(do (call print 1))"),
            ("1\n2", "(do 1 2)"),
            ("", "(do)"),
            ("a == b", "(do (op == a b))"),
        ]);
    }

    #[test]
    fn desugar_short_circuit_to_if() {
        check(&[
            ("a and b", "(do (if a (op bool b) false))"),
            ("a or b", "(do (if a true (op bool b)))"),
            (
                "a and b or c",
                "(do (if (if a (op bool b) false) true (op bool c)))",
            ),
        ]);
    }

    #[test]
    fn desugar_keeps_positions() {
        let c = tree_of("1 + 2");
        assert_eq!(c.kids[0].pos.col, 3, "the operator");
    }

    #[test]
    fn desugar_statements_and_blocks() {
        check(&[
            ("x = 1", "(do (let x 1))"),
            (
                "if c: x = 1 else: 2 ;;",
                "(do (if c (do (let x 1)) (do 2)))",
            ),
            ("if c: 1 ;;", "(do (if c (do 1) (nil)))"),
            ("c then 1 else 2", "(do (if c 1 2))"),
            ("\"a${x}b\"", "(do (concat \"a\" x \"b\"))"),
            ("nil", "(do (nil))"),
            ("'ok", "(do 'ok)"),
            ("1h", "(do 1h)"),
            ("\"plain\"", "(do \"plain\")"),
            (
                "c then a and b else d",
                "(do (if c (if a (op bool b) false) d))",
            ),
        ]);
    }

    #[test]
    fn desugar_functions() {
        check(&[
            (
                "fn add(a, b) = a + b",
                "(do (fn add(a, b) (do (op + a b))))",
            ),
            ("f = x => x", "(do (let f (fn (x) (do x))))"),
            (
                "fn g(a = 1 + 1) = return a",
                "(do (fn g(a=?) (op + 1 1) (do (return a))))",
            ),
            ("f(1, k: 2 * 3)", "(do (call f 1 (named k (op * 2 3))))"),
        ]);
    }

    #[test]
    fn desugar_m4() {
        check(&[
            ("xs |> map(f) |> sum", "(do (call sum (call map xs f)))"),
            ("x |> f", "(do (call f x))"),
            ("x in xs", "(do (op in x xs))"),
            ("x not in xs", "(do (op not (op in x xs)))"),
            (
                "(q, r) = f()",
                "(do (let (ptuple (pbind q) (pbind r)) (call f)))",
            ),
            (
                "for x in xs: g(x) ;;",
                "(do (call (prelude map) xs (fn (x) (do (call g x)))))",
            ),
            (
                "for (k, v) in m: k ;;",
                "(do (call (prelude map) m (fn ($f0) (do (let fresh (ptuple (pbind k) (pbind v)) $f0) k))))",
            ),
            (
                "match v: 0 => a; _ => b ;;",
                "(do (do (let $m0 v) (match $m0 (arm (plit 0) (nil) (do a)) (arm (pwild) (nil) (do b)))))",
            ),
            (
                "f(a, _)",
                "(do (do (let $p0 f) (let $p1 a) (fn (h0) (do (call $p0 $p1 h0)))))",
            ),
            (
                "g(_, k(1), _)",
                "(do (do (let $p0 g) (let $p1 (call k 1)) (fn (h0, h1) (do (call $p0 h0 $p1 h1)))))",
            ),
            ("xs[0] + m.k", "(do (op + (index xs 0) (field k m)))"),
            ("r!", "(do (unwrap ! r))"),
        ]);
    }

    #[test]
    fn for_body_lambda_is_marked() {
        let c = tree_of("for x in xs: x ;;");
        let fn_node = &c.kids[0].kids[2];
        match &fn_node.lit {
            Lit::Fn(info) => assert!(info.for_body),
            _ => panic!("for body lambda = {fn_node}"),
        }
    }

    #[test]
    fn chained_pipe_with_lambdas_walks_each_operand_once() {
        assert_eq!(
            core("xs |> filter(x => x) |> map(x => x)"),
            "(do (call map (call filter xs (fn (x) (do x))) (fn (x) (do x))))"
        );
        assert_eq!(
            core("xs |> (x => x) |> f"),
            "(do (call f (call (fn (x) (do x)) xs)))"
        );
    }

    #[test]
    fn test_block_becomes_a_function() {
        assert_eq!(
            core("test \"a\":\n  assert f() == 1\n;;"),
            "(do (test \"a\" (fn test: a() (do (assert (op == (call f) 1))))))"
        );
    }

    #[test]
    fn parameter_patterns_destructure_at_entry() {
        assert_eq!(
            core("((k, v)) => k"),
            "(do (fn ($a0) (do (let fresh (ptuple (pbind k) (pbind v)) $a0) k)))"
        );
        assert_eq!(
            core("fn f(a, (b, c), d = 1) = a"),
            "(do (fn f(a, $a0, d=?) 1 (do (let fresh (ptuple (pbind b) (pbind c)) $a0) a)))"
        );
    }

    #[test]
    fn input_is_not_modified_and_deep_chains_work() {
        std::thread::Builder::new()
            .stack_size(512 << 20)
            .spawn(|| {
                let src = format!("x = a{}", "[0]".repeat(9_000));
                let mut sm = SourceMap::new();
                let f = sm.add("t.fung", &src);
                let (toks, _) = lex(f, &src).unwrap();
                let surface = parse(&toks).unwrap();
                let before = surface.to_string();
                let c = desugar(&surface).unwrap();
                assert_eq!(surface.to_string(), before);
                assert!(c.to_string().starts_with("(do (let x (index"));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
