//! Pattern trees stored as constants and interpreted by the VM's MATCH
//! instruction (09: 패턴 의미가 Go 함수 하나에 모인다).

use std::fmt;

use crate::consts::{Const, format};

/// What a pattern node matches.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PatKind {
    /// `_`
    #[default]
    Wild,
    /// A name: writes the matched value.
    Bind,
    /// A literal compared with `==`.
    Lit,
    /// `Name(p…)`
    Tag,
    /// `(p, …)`
    Tuple,
    /// `[p, …]` with an optional rest.
    List,
    /// `{k: p, …}`: other keys are ignored.
    Map,
    /// `p | p`
    Alt,
}

/// One pattern node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pattern {
    pub kind: PatKind,
    /// Lit: the literal.
    pub lit: Option<Const>,
    /// Tag: the tag name.
    pub name: String,
    /// Bind: write to a global instead of a slot.
    pub global: bool,
    /// Bind: slot or global index.
    pub index: usize,
    /// Tag args, Tuple/List items, Map values, Alt alternatives.
    pub kids: Vec<Pattern>,
    /// Map: keys parallel to `kids`.
    pub keys: Vec<String>,
    /// List: `None` = exact length; Wild ignores the rest; Bind binds it.
    pub rest: Option<Box<Pattern>>,
}

fn join(f: &mut fmt::Formatter<'_>, kids: &[Pattern], sep: &str) -> fmt::Result {
    for (i, k) in kids.iter().enumerate() {
        if i > 0 {
            f.write_str(sep)?;
        }
        write!(f, "{k}")?;
    }
    Ok(())
}

/// Renders a pattern for disassembly: bound slots print as `$n` and
/// globals as `@n`.
impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            PatKind::Wild => f.write_str("_"),
            PatKind::Bind => {
                let sigil = if self.global { '@' } else { '$' };
                write!(f, "{sigil}{}", self.index)
            }
            PatKind::Lit => f.write_str(&format(self.lit.as_ref().unwrap_or(&Const::Nil))),
            PatKind::Tag => {
                write!(f, "{}(", self.name)?;
                join(f, &self.kids, ", ")?;
                f.write_str(")")
            }
            PatKind::Tuple => {
                f.write_str("(")?;
                join(f, &self.kids, ", ")?;
                if self.kids.len() == 1 {
                    f.write_str(",")?;
                }
                f.write_str(")")
            }
            PatKind::List => {
                f.write_str("[")?;
                join(f, &self.kids, ", ")?;
                if let Some(rest) = &self.rest {
                    if !self.kids.is_empty() {
                        f.write_str(", ")?;
                    }
                    f.write_str("...")?;
                    if rest.kind != PatKind::Wild {
                        write!(f, "{rest}")?;
                    }
                }
                f.write_str("]")
            }
            PatKind::Map => {
                f.write_str("{")?;
                for (i, (k, v)) in self.keys.iter().zip(&self.kids).enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                f.write_str("}")
            }
            PatKind::Alt => join(f, &self.kids, " | "),
        }
    }
}
