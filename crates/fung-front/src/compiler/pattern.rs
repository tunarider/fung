//! Converts resolved pattern subtrees into the constant pattern trees the
//! VM interprets (09. 패턴 매칭).

use super::{Compiler, internal, lit_index, lit_str};
use crate::bytecode::{PatKind, Pattern};
use crate::consts::Const;
use crate::diag::Diag;
use crate::tree::{Head, Lit, Node};

fn pattern(kind: PatKind) -> Pattern {
    Pattern {
        kind,
        ..Pattern::default()
    }
}

fn build(n: &Node) -> Result<Pattern, Diag> {
    match n.head {
        Head::Pwild => Ok(pattern(PatKind::Wild)),
        Head::Pbind | Head::Prest => {
            let Some(r) = n.kids.first() else {
                if n.head == Head::Prest {
                    return Ok(pattern(PatKind::Wild));
                }
                return Err(internal(n.pos, "compiler: unresolved pattern name".into()));
            };
            Ok(Pattern {
                global: r.head == Head::Global,
                index: lit_index(r),
                ..pattern(PatKind::Bind)
            })
        }
        Head::Plit => {
            let lit = match &n.lit {
                Lit::Int(i) => Const::Int(*i),
                Lit::Float(f) => Const::Float(*f),
                Lit::Str(s) => Const::Str(s.clone()),
                Lit::Bool(b) => Const::Bool(*b),
                Lit::Dur(d) => Const::Dur(*d),
                _ => return Err(internal(n.pos, "compiler: unknown pattern literal".into())),
            };
            Ok(Pattern {
                lit: Some(lit),
                ..pattern(PatKind::Lit)
            })
        }
        Head::Pnil => Ok(Pattern {
            lit: Some(Const::Nil),
            ..pattern(PatKind::Lit)
        }),
        Head::Psym => Ok(Pattern {
            lit: Some(Const::Sym(lit_str(n).to_string())),
            ..pattern(PatKind::Lit)
        }),
        Head::Ptag => kids(
            Pattern {
                name: lit_str(n).to_string(),
                ..pattern(PatKind::Tag)
            },
            &n.kids,
        ),
        Head::Ptuple => kids(pattern(PatKind::Tuple), &n.kids),
        Head::Plist => {
            let mut p = pattern(PatKind::List);
            let mut items = &n.kids[..];
            if let Some((last, init)) = items.split_last()
                && last.head == Head::Prest
            {
                p.rest = Some(Box::new(build(last)?));
                items = init;
            }
            kids(p, items)
        }
        Head::Pmap => {
            let mut p = pattern(PatKind::Map);
            for entry in &n.kids {
                p.kids.push(build(&entry.kids[0])?);
                p.keys.push(lit_str(entry).to_string());
            }
            Ok(p)
        }
        Head::Palt => kids(pattern(PatKind::Alt), &n.kids),
        h => Err(internal(
            n.pos,
            format!("compiler: unknown pattern {}", h.as_str()),
        )),
    }
}

fn kids(mut p: Pattern, kids: &[Node]) -> Result<Pattern, Diag> {
    for k in kids {
        p.kids.push(build(k)?);
    }
    Ok(p)
}

impl Compiler<'_> {
    /// Registers a pattern constant and returns its index.
    pub(super) fn add_pattern(&mut self, n: &Node) -> Result<usize, Diag> {
        let p = build(n)?;
        self.proto.patterns.push(p);
        Ok(self.proto.patterns.len() - 1)
    }
}
