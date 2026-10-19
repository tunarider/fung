//! The spacing table.

use crate::lexer::Kind;

pub(super) fn is_opener(k: Kind) -> bool {
    matches!(k, Kind::LParen | Kind::LBracket | Kind::LBrace)
}

pub(super) fn is_closer(k: Kind) -> bool {
    matches!(k, Kind::RParen | Kind::RBracket | Kind::RBrace)
}

/// A token an index may follow directly: xs[0], "s"[0], f(x)[0].
fn is_value_end(k: Kind) -> bool {
    matches!(
        k,
        Kind::Name | Kind::Tag | Kind::RParen | Kind::RBracket | Kind::String | Kind::RawString
    )
}

/// A token a call may follow directly: f(x), Ok(x), f(x)(y), xs[0](y).
/// Strings are not callable, so `import "./x" (a)` keeps its space.
fn is_callee(k: Kind) -> bool {
    matches!(k, Kind::Name | Kind::Tag | Kind::RParen | Kind::RBracket)
}

/// A token after which `-` is a sign, not a subtraction: operators,
/// openers, separators, arrows, keywords.
pub(super) fn prefix_context(k: Kind) -> bool {
    use Kind::*;
    is_opener(k)
        || k.is_keyword()
        || matches!(
            k,
            Comma
                | Colon
                | Semi
                | DoubleSemi
                | FatArrow
                | Arrow
                | Assign
                | Eq
                | Ne
                | Lt
                | Gt
                | Le
                | Ge
                | Plus
                | Minus
                | Star
                | StarStar
                | Slash
                | SlashSlash
                | Percent
                | Pipe
                | Bar
                | Coalesce
                | DotDot
                | DotDotEq
        )
}

/// Whether two tokens written without a space would read back as other
/// tokens (`;` `;` as `;;`, `?` `?` as `??`, `;` `;;` as `;;` `;`).
fn would_join(p: Kind, n: Kind) -> bool {
    use Kind::*;
    matches!(
        (p, n),
        (Semi, Semi)
            | (Semi, DoubleSemi)
            | (Question, Question)
            | (Dot, Dot | DotDot | DotDotEq)
            | (DotDot, Dot | DotDot | DotDotEq)
    )
}

/// The number of spaces between two adjacent tokens on a line (spec:
/// 토큰 사이 공백). `prev_is_sign` says `p` is a prefix `-`;
/// `import_line` says the line is an import, whose name list is not a
/// call.
pub(super) fn gap(p: Kind, n: Kind, prev_is_sign: bool, import_line: bool) -> usize {
    use Kind::*;
    if import_line && n == LParen {
        return 1;
    }
    if would_join(p, n) {
        return 1;
    }
    let tight = is_opener(p)
        || is_closer(n)
        || matches!(n, Comma | Semi | Colon)
        || matches!(p, Dot | DotDot | DotDotEq)
        || matches!(n, Dot | DotDot | DotDotEq)
        || p == Ellipsis
        || matches!(n, Bang | Question)
        || (n == LParen && (is_callee(p) || p == KwFn))
        || (n == LBracket && is_value_end(p))
        || prev_is_sign
        || (p == DoubleSemi && is_closer(n));
    usize::from(!tight)
}
