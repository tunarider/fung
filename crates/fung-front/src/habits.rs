//! Habits from other languages that fung recognizes at an error (FE43):
//! one table of words and one of tokens, each with the help that says how
//! fung writes it. The lexer, the parser and the resolver consult it only
//! once they have an error; nothing here changes what parses.

/// A word other languages use as a keyword where fung has none.
pub struct Word {
    pub words: &'static [&'static str],
    pub help: &'static str,
    /// A value word (`null`): looked up when the resolver finds the name
    /// undefined, as well as at the start of a statement.
    pub value: bool,
}

pub const WORDS: &[Word] = &[
    Word {
        words: &["elif", "elsif", "elseif"],
        help: "fung writes `else if`",
        value: false,
    },
    Word {
        words: &["while", "loop"],
        help: "fung has no loop statement: walk a collection with `for x in xs:`, or repeat with recursion, `until(x, f, pred)` or `iterate(x, f, n)`",
        value: false,
    },
    Word {
        words: &["let", "var", "const", "val"],
        help: "a name is bound without a keyword: `x = 1`",
        value: false,
    },
    Word {
        words: &["def", "function", "func", "fun"],
        help: "a function is defined with `fn`: `fn name(x): … ;;` or `fn name(x) = …`",
        value: false,
    },
    Word {
        words: &["lambda"],
        help: "a lambda is written with `=>`: `x => x + 1`",
        value: false,
    },
    Word {
        words: &["switch", "case"],
        help: "`match` compares a value with patterns: `match x: 1 => … ;;`",
        value: false,
    },
    Word {
        words: &["try", "catch", "except", "raise", "throw"],
        help: "fung has no exceptions: a function that can fail returns `Err(…)`; handle it with `?`, `!` or `match`",
        value: false,
    },
    Word {
        words: &["from"],
        help: "names are imported with `import m (a, b)`",
        value: false,
    },
    Word {
        words: &["end", "endif", "fi", "done"],
        help: "a block closes with `;;`",
        value: false,
    },
    Word {
        words: &["null", "undefined", "nullptr"],
        help: "fung's empty value is `nil`",
        value: true,
    },
];

/// The help for `name` used as a keyword, if it is a habit.
pub fn word(name: &str) -> Option<&'static str> {
    WORDS
        .iter()
        .find(|w| w.words.contains(&name))
        .map(|w| w.help)
}

/// The help for an undefined name that another language has as a value
/// (`null`).
pub fn value_word(name: &str) -> Option<&'static str> {
    WORDS
        .iter()
        .find(|w| w.value && w.words.contains(&name))
        .map(|w| w.help)
}

/// Whether `name` is a word that closes a block elsewhere (`end`).
pub fn is_block_end(name: &str) -> bool {
    WORDS
        .iter()
        .any(|w| w.words.contains(&"end") && w.words.contains(&name))
}

/// Operators of other languages, with fung's spelling.
pub const OPERATORS: &[(&str, &str)] = &[
    ("&&", "fung writes `and`"),
    ("||", "fung writes `or`"),
    (
        "!",
        "fung writes `not x`; a `!` after a value unwraps an `Ok`",
    ),
    (
        "->",
        "a lambda is written with `=>`: `(a, b) => a + b`; `->` only gives a function's return type",
    ),
    (
        "{",
        "a block starts with `:` and closes with `;;`: `fn f(x): … ;;`, `if c: … ;;`",
    ),
    (
        "++",
        "fung has no `++`: write the new value, `n = n + 1` (a function's local name may be bound again), or bind a new name",
    ),
];

/// The help for an operator habit (`&&`), if it is one.
pub fn operator(op: &str) -> Option<&'static str> {
    OPERATORS.iter().find(|(o, _)| *o == op).map(|(_, h)| *h)
}

/// The help for a compound assignment (`+=`).
pub fn compound(op: &str) -> String {
    format!(
        "fung has no `{op}=`: write the new value, `n = n {op} …` (a function's local name may be bound again), or bind a new name"
    )
}

/// The help for a reserved word written where a name is bound.
pub fn reserved(word: &str) -> String {
    format!("`{word}` is a reserved word, so it cannot be a name; pick another one")
}

/// The help for `=` where a condition or an argument goes on.
pub const COMPARE: &str = "to compare, write `==`; `=` binds a name, and only as a statement";

/// The help for `=` between a call's arguments.
pub const NAMED_ARG: &str = "a named argument is written `name: value`; to compare, write `==`";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_is_a_plain_name_not_a_fung_keyword() {
        for w in WORDS {
            for name in w.words {
                assert!(
                    !crate::lexer::Kind::keywords()
                        .iter()
                        .any(|(k, _)| k == name),
                    "{name}"
                );
            }
        }
        assert!(is_block_end("end") && !is_block_end("let"));
        assert_eq!(value_word("null"), Some("fung's empty value is `nil`"));
        assert_eq!(value_word("let"), None);
        assert_eq!(word("elif"), Some("fung writes `else if`"));
    }
}
