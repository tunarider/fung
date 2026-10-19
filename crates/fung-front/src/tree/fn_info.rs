//! Literal payloads of `fn`, `typealias` and `import` nodes.

use std::fmt;

use super::Node;
use crate::source::Pos;

/// One declared parameter of a function.
#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub pos: Pos,
    /// Union alternatives; `None` means unchecked.
    pub types: Option<Vec<String>>,
    pub has_default: bool,
    /// `...name`: collects surplus positional arguments.
    pub rest: bool,
    /// A destructuring parameter: desugar gives it a hidden name.
    pub pattern: Option<Box<Node>>,
}

/// Where a closure copies a captured value from when it is created.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaptureKind {
    /// The creating frame's slot `index`.
    Local,
    /// The creating closure's `captured[index]`.
    Captured,
    /// The creating frame's own closure.
    SelfFn,
    /// The file variable `globals[index]`, as it is right now.
    Global,
}

/// One value a function copies from its enclosing function.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Capture {
    pub kind: CaptureKind,
    pub index: usize,
}

/// The literal of an `(import ...)` node.
#[derive(Clone, Debug)]
pub struct Import {
    /// `fs` or `./lib/rename`.
    pub spec: String,
    /// Written as a string: a file path.
    pub is_path: bool,
    /// After `as`; empty if none.
    pub alias: String,
    /// The `(a, b)` list; `None` if absent.
    pub names: Option<Vec<String>>,
    /// One per `names` entry, for errors.
    pub name_pos: Vec<Pos>,
}

impl fmt::Display for Import {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_path {
            f.write_str(&crate::diag::quote(&self.spec))?;
        } else {
            f.write_str(&self.spec)?;
        }
        if !self.alias.is_empty() {
            write!(f, " as {}", self.alias)?;
        }
        if let Some(names) = &self.names {
            write!(f, " ({})", names.join(", "))?;
        }
        Ok(())
    }
}

/// The literal of a `(typealias ...)` node.
#[derive(Clone, Debug)]
pub struct TypeAlias {
    pub name: String,
    /// Where the alias name is written, for errors about the name.
    pub name_pos: Pos,
    pub alts: Vec<String>,
}

impl fmt::Display for TypeAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// The literal of an `(fn ...)` node. The parser fills the declaration;
/// resolve fills `num_locals` and `captures`.
#[derive(Clone, Debug)]
pub struct FnInfo {
    /// Empty for lambdas and anonymous fns.
    pub name: String,
    pub params: Vec<Param>,
    /// `None` means unchecked.
    pub ret: Option<Vec<String>>,
    /// The synthetic lambda of a `for`: `return` inside is E0213.
    pub for_body: bool,
    pub num_locals: usize,
    pub captures: Vec<Capture>,
}

/// The signature for dumps: `add(a, b=?)->int`, `(x)` for lambdas.
impl fmt::Display for FnInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        f.write_str("(")?;
        for (i, p) in self.params.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            if p.rest {
                f.write_str("...")?;
            }
            if let Some(pat) = &p.pattern {
                write!(f, "{pat}")?;
            }
            f.write_str(&p.name)?;
            if let Some(t) = &p.types {
                write!(f, ":{}", t.join("|"))?;
            }
            if p.has_default {
                f.write_str("=?")?;
            }
        }
        f.write_str(")")?;
        if let Some(r) = &self.ret {
            write!(f, "->{}", r.join("|"))?;
        }
        Ok(())
    }
}
