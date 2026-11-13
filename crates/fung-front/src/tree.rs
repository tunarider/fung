//! The uniform tree every front-end stage works on. The parser produces
//! surface forms, desugar rewrites them into core forms, and the compiler
//! reads core forms. One node shape covers all of them: a head word, a
//! position, an optional literal payload, and children.

mod fn_info;

use std::fmt::{self, Write};

pub use fn_info::{Capture, CaptureKind, FnInfo, Import, Param, TypeAlias, TypeTerm};

use crate::consts::{format_dur, format_float, quote_bytes, quote_str};
use crate::source::Pos;

/// The first character of a name desugar makes up (`$f0`, `$h1`): no
/// source name has it, so it never meets one. The stages after desugar
/// ask `is_hidden`, not the character (FE62).
pub const HIDDEN: char = '$';

/// Whether `name` is one desugar made up.
pub fn is_hidden(name: &str) -> bool {
    name.starts_with(HIDDEN)
}

/// The literal of a `(let …)` desugar makes whose pattern binds new names
/// in the block (a `for` or parameter pattern), where an assignment would
/// rebind: `(let fresh pat value)` (FE62).
pub const FRESH: &str = "fresh";

/// The operator desugar wraps a condition in: `(op bool e)` checks that
/// `e` is a bool (FE62).
pub const BOOL_CHECK: &str = "bool";

/// Every node head the front end uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Head {
    Arm,
    Assert,
    Assign,
    /// A tag written without arguments, `Red` (parser only): desugar
    /// makes it the symbol `'Red`, or the tag call on a pipe's right
    /// (FE19).
    BareTag,
    Binary,
    Block,
    Bool,
    Call,
    Captured,
    Coalesce,
    Concat,
    Do,
    Duration,
    Entry,
    Field,
    Float,
    Fn,
    For,
    Global,
    Hole,
    If,
    Import,
    Index,
    Int,
    Interp,
    Let,
    List,
    Local,
    Map,
    Match,
    Member,
    /// A slice's missing end, `xs[1..]` (VA3): not the value `nil`.
    Missing,
    Name,
    Named,
    Nil,
    Op,
    Palt,
    Pbind,
    Pentry,
    Pipe,
    Plist,
    Plit,
    Pmap,
    Pnil,
    Prelude,
    Prest,
    Program,
    Psym,
    Ptag,
    Ptuple,
    Pwild,
    Range,
    Return,
    SelfFn,
    Slice,
    /// `...xs` as a call's last positional argument (D45): its items are
    /// the arguments.
    Spread,
    String,
    Symbol,
    Tag,
    Test,
    Then,
    Tuple,
    Typealias,
    Unary,
    Unwrap,
}

impl Head {
    /// The head word exactly as `fung dump` prints it.
    pub fn as_str(self) -> &'static str {
        match self {
            Head::Arm => "arm",
            Head::BareTag => "bare_tag",
            Head::Assert => "assert",
            Head::Assign => "assign",
            Head::Binary => "binary",
            Head::Block => "block",
            Head::Bool => "bool",
            Head::Call => "call",
            Head::Captured => "captured",
            Head::Coalesce => "coalesce",
            Head::Concat => "concat",
            Head::Do => "do",
            Head::Duration => "duration",
            Head::Entry => "entry",
            Head::Field => "field",
            Head::Float => "float",
            Head::Fn => "fn",
            Head::For => "for",
            Head::Global => "global",
            Head::Hole => "hole",
            Head::If => "if",
            Head::Import => "import",
            Head::Index => "index",
            Head::Int => "int",
            Head::Interp => "interp",
            Head::Let => "let",
            Head::List => "list",
            Head::Local => "local",
            Head::Map => "map",
            Head::Match => "match",
            Head::Member => "member",
            Head::Missing => "missing",
            Head::Name => "name",
            Head::Named => "named",
            Head::Nil => "nil",
            Head::Op => "op",
            Head::Palt => "palt",
            Head::Pbind => "pbind",
            Head::Pentry => "pentry",
            Head::Pipe => "pipe",
            Head::Plist => "plist",
            Head::Plit => "plit",
            Head::Pmap => "pmap",
            Head::Pnil => "pnil",
            Head::Prelude => "prelude",
            Head::Prest => "prest",
            Head::Program => "program",
            Head::Psym => "psym",
            Head::Ptag => "ptag",
            Head::Ptuple => "ptuple",
            Head::Pwild => "pwild",
            Head::Range => "range",
            Head::Return => "return",
            Head::SelfFn => "self",
            Head::Slice => "slice",
            Head::Spread => "spread",
            Head::String => "string",
            Head::Symbol => "symbol",
            Head::Tag => "tag",
            Head::Test => "test",
            Head::Then => "then",
            Head::Tuple => "tuple",
            Head::Typealias => "typealias",
            Head::Unary => "unary",
            Head::Unwrap => "unwrap",
        }
    }

    /// Bare leaves print as their value alone; other leaves print as
    /// `(head lit)`.
    pub fn is_bare_leaf(self) -> bool {
        matches!(
            self,
            Head::Int
                | Head::Float
                | Head::Bool
                | Head::BareTag
                | Head::Name
                | Head::String
                | Head::Symbol
                | Head::Duration
        )
    }
}

/// A node's literal payload.
#[derive(Clone, Debug)]
pub enum Lit {
    None,
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    /// A string literal that is not valid UTF-8 (`"\xff"`, L4).
    Bytes(Vec<u8>),
    /// Nanoseconds.
    Dur(i64),
    Fn(Box<FnInfo>),
    Alias(Box<TypeAlias>),
    Import(Box<Import>),
}

#[derive(Clone, Debug)]
pub struct Node {
    pub head: Head,
    pub pos: Pos,
    pub lit: Lit,
    pub kids: Vec<Node>,
    /// Byte span in the source, filled by the parser for expressions and
    /// statements. `end == 0` means unknown.
    pub off: usize,
    pub end: usize,
    /// The expression was written in parentheses, `(e)` (FE27): the
    /// parser marks it so a pipe calls `x |> (f(1))` as a value instead
    /// of filling the call. The mark does not show in `fung dump`.
    pub paren: bool,
    /// The node's number in its resolved tree (`number`), by which the
    /// compiler's analyses record what they learn about it; 0 before
    /// that, which no analysis records anything for. A copy keeps the
    /// number of what it copies.
    pub id: u32,
}

impl Node {
    /// Where the node's source text starts, as far as the tree knows: the
    /// first position in it (a call's own position is its `(`, an
    /// operator's the operator, so the start is a position inside). Text
    /// before the first node, such as an opening parenthesis, is not
    /// counted.
    pub fn start_pos(&self) -> Pos {
        let mut first = self.pos;
        let mut todo: Vec<&Node> = vec![self];
        while let Some(n) = todo.pop() {
            let earlier = (n.pos.line, n.pos.col) < (first.line, first.col);
            if !n.pos.is_zero() && (first.is_zero() || earlier) {
                first = n.pos;
            }
            todo.extend(&n.kids);
        }
        first
    }

    /// Calls `f` with every name a pattern binds, in order and with where
    /// it is bound: `(pbind x)`, a named `...rest`, and a plain name as an
    /// assignment's target. `..._` and `_` bind nothing (FE11). The
    /// parser, desugar and resolve all count names with this (FE22).
    pub fn bound_names(&self, f: &mut dyn FnMut(&str, Pos)) {
        if matches!(self.head, Head::Name | Head::Pbind | Head::Prest)
            && let Lit::Str(name) = &self.lit
            && !name.is_empty()
            && name != "_"
        {
            f(name, self.pos);
        }
        for k in &self.kids {
            k.bound_names(f);
        }
    }

    pub fn new(head: Head, pos: Pos, kids: Vec<Node>) -> Node {
        Node::with_lit(head, pos, Lit::None, kids)
    }

    pub fn leaf(head: Head, pos: Pos, lit: Lit) -> Node {
        Node::with_lit(head, pos, lit, Vec::new())
    }

    pub fn with_lit(head: Head, pos: Pos, lit: Lit, kids: Vec<Node>) -> Node {
        Node {
            head,
            pos,
            lit,
            kids,
            off: 0,
            end: 0,
            paren: false,
            id: 0,
        }
    }

    /// Numbers every node of the tree from 1, parents before children
    /// (resolve does, for the compiler). Iterative: trees can be deep.
    pub fn number(&mut self) {
        let mut next = 1u32;
        let mut todo: Vec<&mut Node> = vec![self];
        while let Some(n) = todo.pop() {
            n.id = next;
            next = next.checked_add(1).expect("fewer than 2^32 nodes");
            todo.extend(n.kids.iter_mut().rev());
        }
    }

    /// The node's source text, or "" when its span is unknown.
    pub fn text<'a>(&self, src: &'a str) -> &'a str {
        if self.end == 0 || self.end > src.len() || self.off > self.end {
            return "";
        }
        src.get(self.off..self.end).unwrap_or("")
    }

    fn lit_string(&self) -> String {
        match &self.lit {
            Lit::Fn(f) => f.to_string(),
            Lit::Alias(a) => a.to_string(),
            Lit::Import(i) => i.to_string(),
            Lit::Float(f) => format_float(*f),
            Lit::Dur(d) => format_dur(*d),
            Lit::Int(i) => i.to_string(),
            Lit::Bool(b) => b.to_string(),
            Lit::None => String::new(),
            Lit::Bytes(b) => quote_bytes(b),
            Lit::Str(s) => match self.head {
                // A resolved import holds its module key.
                Head::String | Head::Plit | Head::Import | Head::Test => quote_str(s),
                Head::Symbol => format!("'{s}"),
                _ => s.clone(),
            },
        }
    }

    fn write(&self, out: &mut String) {
        let has_lit = !matches!(self.lit, Lit::None);
        if self.kids.is_empty() && has_lit {
            let lit = self.lit_string();
            if self.head.is_bare_leaf() {
                out.push_str(&lit);
            } else {
                let _ = write!(out, "({} {})", self.head.as_str(), lit);
            }
            return;
        }
        out.push('(');
        out.push_str(self.head.as_str());
        if has_lit {
            out.push(' ');
            out.push_str(&self.lit_string());
        }
        for k in &self.kids {
            out.push(' ');
            k.write(out);
        }
        out.push(')');
    }
}

/// The S-expression format of `fung dump`.
impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        self.write(&mut s);
        f.write_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FileId, Pos};

    fn p() -> Pos {
        Pos {
            file: FileId(0),
            line: 1,
            col: 1,
        }
    }

    fn name(s: &str) -> Node {
        Node::leaf(Head::Name, p(), Lit::Str(s.into()))
    }

    fn int(i: i64) -> Node {
        Node::leaf(Head::Int, p(), Lit::Int(i))
    }

    fn bin(op: &str, a: Node, b: Node) -> Node {
        // the operator is the node's literal, not a kid
        Node::with_lit(Head::Binary, p(), Lit::Str(op.into()), vec![a, b])
    }

    #[test]
    fn s_expressions() {
        let n = Node::new(
            Head::Program,
            p(),
            vec![Node::new(
                Head::Assign,
                p(),
                vec![name("x"), bin("+", int(1), int(2))],
            )],
        );
        assert_eq!(n.to_string(), "(program (assign x (binary + 1 2)))");
    }

    #[test]
    fn string_formats_s_expressions() {
        let mul = bin("*", int(2), int(3));
        let add = bin("+", int(1), mul);
        let call = Node::new(Head::Call, p(), vec![name("print"), add]);
        assert_eq!(call.to_string(), "(call print (binary + 1 (binary * 2 3)))");
    }

    #[test]
    fn string_formats_leaves() {
        let cases: Vec<(Node, &str)> = vec![
            (Node::leaf(Head::Float, p(), Lit::Float(2.5)), "2.5"),
            (Node::leaf(Head::Float, p(), Lit::Float(1.0)), "1.0"),
            (Node::leaf(Head::Bool, p(), Lit::Bool(true)), "true"),
            (
                Node::leaf(Head::Prelude, p(), Lit::Str("print".into())),
                "(prelude print)",
            ),
            (
                Node::leaf(Head::String, p(), Lit::Str("a b".into())),
                "\"a b\"",
            ),
            (Node::new(Head::Do, p(), vec![]), "(do)"),
            (Node::new(Head::Call, p(), vec![name("f")]), "(call f)"),
        ];
        for (n, want) in cases {
            assert_eq!(n.to_string(), want);
        }
    }

    #[test]
    fn leaves_use_fung_formatting() {
        let cases: Vec<(Node, &str)> = vec![
            (Node::leaf(Head::Float, p(), Lit::Float(1e21)), "1e21"),
            (Node::leaf(Head::Float, p(), Lit::Float(1e6)), "1000000.0"),
            (
                Node::leaf(Head::Float, p(), Lit::Float(15000000000.0)),
                "15000000000.0",
            ),
            (Node::leaf(Head::Symbol, p(), Lit::Str("ok".into())), "'ok"),
            (
                Node::leaf(Head::Symbol, p(), Lit::Str("sym".into())),
                "'sym",
            ),
            (
                Node::leaf(Head::Duration, p(), Lit::Dur(90 * 60 * 1_000_000_000)),
                "1h30m",
            ),
            (
                Node::leaf(Head::Duration, p(), Lit::Dur(5_400_000_000_000)),
                "1h30m",
            ),
            (
                Node::leaf(Head::String, p(), Lit::Str("tab\t${".into())),
                "\"tab\\t\\${\"",
            ),
            (
                Node::leaf(Head::String, p(), Lit::Str("a\tb".into())),
                "\"a\\tb\"",
            ),
            (Node::new(Head::Nil, p(), vec![]), "(nil)"),
            (
                Node::new(Head::Assign, p(), vec![name("x"), int(1)]),
                "(assign x 1)",
            ),
        ];
        for (n, want) in cases {
            assert_eq!(n.to_string(), want);
        }
    }

    fn param(name: &str) -> Param {
        Param {
            name: name.into(),
            pos: p(),
            types: None,
            has_default: false,
            rest: false,
            pattern: None,
        }
    }

    fn info(name: &str, params: Vec<Param>) -> FnInfo {
        FnInfo {
            name: name.into(),
            name_pos: p(),
            params,
            ret: None,
            for_body: false,
            lambda: false,
            num_locals: 0,
            captures: vec![],
        }
    }

    #[test]
    fn fn_info_string() {
        let mut b = param("b");
        b.has_default = true;
        let mut pp = param("p");
        pp.types = Some(vec![
            TypeTerm::plain("int", p()),
            TypeTerm::plain("nil", p()),
        ]);
        let mut xs = param("xs");
        xs.rest = true;
        let mut f = info("f", vec![pp]);
        f.ret = Some(vec![TypeTerm::plain("str", p())]);
        let cases = vec![
            (info("add", vec![param("a"), b]), "add(a, b=?)"),
            (info("", vec![param("x")]), "(x)"),
            (f, "f(p:int|nil)->str"),
            (info("g", vec![xs]), "g(...xs)"),
            (info("h", vec![]), "h()"),
        ];
        for (i, want) in cases {
            assert_eq!(i.to_string(), want);
        }
        let n = Node::with_lit(
            Head::Fn,
            p(),
            Lit::Fn(Box::new(info("id", vec![param("x")]))),
            vec![Node::new(Head::Do, p(), vec![name("x")])],
        );
        assert_eq!(n.to_string(), "(fn id(x) (do x))");
    }

    #[test]
    fn pattern_param_prints_pattern_before_name() {
        let mut q = param("$p0");
        q.pattern = Some(Box::new(Node::new(Head::Plist, p(), vec![])));
        assert_eq!(info("f", vec![q]).to_string(), "f((plist)$p0)");
    }

    #[test]
    fn import_and_alias_strings() {
        let im = Import {
            spec: "./lib/x".into(),
            is_path: true,
            alias: "x".into(),
            names: Some(vec!["a".into(), "b".into()]),
            name_pos: vec![p(), p()],
            bind_pos: p(),
            bind_len: 1,
        };
        assert_eq!(im.to_string(), "\"./lib/x\" as x (a, b)");
        let m = Import {
            spec: "fs".into(),
            is_path: false,
            alias: String::new(),
            names: None,
            name_pos: vec![],
            bind_pos: p(),
            bind_len: 2,
        };
        assert_eq!(m.to_string(), "fs");
        assert_eq!(
            TypeAlias {
                name: "T".into(),
                name_pos: p(),
                alts: vec![TypeTerm::plain("int", p())]
            }
            .to_string(),
            "T"
        );
    }

    #[test]
    fn import_path_is_quoted_as_a_fung_literal() {
        let im = Import {
            spec: "a\u{7}\u{A0}b".into(),
            is_path: true,
            alias: String::new(),
            names: None,
            name_pos: vec![],
            bind_pos: p(),
            bind_len: 2,
        };
        assert_eq!(im.to_string(), "\"a\\u{7}\\u{a0}b\"");
    }

    #[test]
    fn head_names_round_trip_and_text_span() {
        assert_eq!(Head::SelfFn.as_str(), "self");
        assert_eq!(Head::Pwild.as_str(), "pwild");
        assert!(Head::Duration.is_bare_leaf() && !Head::Prelude.is_bare_leaf());
        let mut n = int(1);
        assert_eq!(n.text("abc"), "");
        n.off = 1;
        n.end = 3;
        assert_eq!(n.text("abcd"), "bc");
        n.end = 9;
        assert_eq!(n.text("abcd"), "");
    }
}
