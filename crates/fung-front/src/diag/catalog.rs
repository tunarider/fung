//! The error catalog: every error and warning code with its message
//! template, underline label and help. `spec/errors.txt` is generated
//! from this table (`FUNG_UPDATE_SPEC=1 cargo test spec_file`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

pub struct Entry {
    pub message: String,
    pub label: String,
    pub help: String,
}

/// `[code, message template, underline label, help]`, sorted by code.
/// Numbers are assigned once and never change.
const CATALOG: &[[&str; 4]] = &[
    ["E0101", "unexpected character %q", "", ""],
    ["E0102", "malformed number %q", "", ""],
    ["E0103", r#"unknown escape \%s in string"#, "", ""],
    ["E0104", "unterminated string", "", ""],
    ["E0105", "malformed duration %q", "", ""],
    ["E0110", "unexpected %s", "", ""],
    [
        "E0111",
        "comparison operators cannot be chained",
        "",
        "split it with `and`: a < b and b < c",
    ],
    ["E0112", "expected %s, found %s", "", ""],
    ["E0113", "duplicate key `%s` in map literal", "", ""],
    ["E0114", "test must be at the top level", "", ""],
    [
        "E0204",
        "undefined name `%s`",
        "not found in this scope",
        "",
    ],
    [
        "E0205",
        "cannot reassign `%s` here: it was defined outside this block",
        "",
        "a block cannot change an outer variable; build the value with an expression instead, e.g. `x = if … ;;` or `fold`",
    ],
    [
        "E0206",
        "`%s` is not visible here: it was defined inside a block",
        "",
        "assign the block's value instead: `x = if … ;;`",
    ],
    [
        "E0208",
        "local function `%s` is defined later in this block",
        "",
        "local functions can only call themselves; move both functions to the top level",
    ],
    [
        "E0209",
        "cannot reassign `%s`: it is a function or an imported name",
        "",
        "",
    ],
    ["E0210", "function `%s` is defined twice", "", ""],
    [
        "E0211",
        "return outside a function",
        "",
        "use `exit()` to stop a script early",
    ],
    ["E0212", "unknown type `%s`", "", ""],
    [
        "E0213",
        "return inside a for body",
        "",
        "`for` builds a list; use `find`, `all`/`any`, or `fold` instead",
    ],
    [
        "E0214",
        "`?` outside a function",
        "",
        "use `!` to stop the script on Err",
    ],
    [
        "E0215",
        "`_` can only be a call argument or a pattern",
        "",
        "",
    ],
    [
        "E0216",
        "alternatives of a pattern must bind the same names",
        "",
        "",
    ],
    ["E0217", "type `%s` is defined twice", "", ""],
    ["E0218", "type `%s` refers to itself", "", ""],
    ["E0219", "type aliases must be at the top level", "", ""],
    [
        "E0220",
        "`?` inside a for body",
        "",
        "`for` builds a list and cannot return early; handle the Err with `match`, or use `all`/`any`/`fold`",
    ],
    [
        "E0221",
        "name `%s` is bound twice in one pattern",
        "",
        "use a guard to compare the two positions: `[a, b] when a == b`",
    ],
    [
        "E0301",
        "cannot find module `%s`",
        "",
        r#"standard library modules are imported by name (`import fs`); your own files by path (`import "./lib/x"`)"#,
    ],
    ["E0302", "circular import: %s", "", ""],
    ["E0303", "module `%s` has no member `%s`", "", ""],
    [
        "E0304",
        "`%s` is private to module `%s`",
        "",
        "names starting with `_` are not exported",
    ],
    ["E0305", "`%s` is not a valid module name; use `as`", "", ""],
    [
        "E0306",
        "only import, fn, type, and assignment are allowed at the top level of an imported module",
        "",
        "move the statement into a function, or into the entry file",
    ],
    ["E0307", "cannot import the entry file", "", ""],
    ["E0308", "cannot shadow module `%s`", "", ""],
    ["E0309", "name `%s` is already bound to a module", "", ""],
    ["E0310", "`%s` is internal; import `%s` instead", "", ""],
    ["E0311", "import must be at the top level", "", ""],
    ["E0401", "cannot call a value of type %s", "", ""],
    ["E0402", "%s takes %d argument(s), got %d", "", ""],
    ["E0403", "argument `%s` must be %s, got %s", "", ""],
    ["E0404", "return value must be %s, got %s", "", ""],
    ["E0405", "argument `%s` given twice", "", ""],
    ["E0406", "%s has no argument named `%s`", "", ""],
    ["E0407", "missing argument `%s`", "", ""],
    ["E0501", "cannot %s %s and %s", "", ""],
    ["E0502", "division by zero", "", ""],
    ["E0503", "integer overflow", "", ""],
    ["E0504", "result is not a finite number", "", ""],
    ["E0505", "expected bool, got %s", "", ""],
    ["E0506", "cannot %s %s", "", ""],
    ["E0507", "index %d is out of range (length %d)", "", ""],
    [
        "E0508",
        "key `%s` not found",
        "",
        "use get(m, key, default) or ?? for a default",
    ],
    ["E0509", "cannot index a value of type %s", "", ""],
    ["E0511", "cannot slice a value of type %s", "", ""],
    [
        "E0512",
        "cannot test membership in a value of type %s",
        "",
        "",
    ],
    ["E0513", "cannot convert %s to %s", "", ""],
    [
        "E0514",
        "cannot use a value of type %s as a map key",
        "",
        "",
    ],
    ["E0515", "invalid pattern: %s", "", ""],
    ["E0601", "no pattern matched %s", "", ""],
    ["E0602", "value %s does not match the pattern", "", ""],
    ["E0603", "unhandled Err: %s", "", ""],
    ["E0604", "`%s` on nil", "", ""],
    [
        "E0605",
        "`%s` on a value of type %s (not Ok or Err)",
        "",
        "",
    ],
    ["E0701", "task cancelled", "", ""],
    ["E0702", "deadlock: all tasks are waiting", "", ""],
    ["E0703", "atom swapped inside its own swap", "", ""],
    ["E0704", "agent called from inside its own swap", "", ""],
    ["E0705", "channel sent into itself", "", ""],
    ["E0801", "internal error: %v", "", ""],
    ["E0802", "expression nesting too deep (limit %d)", "", ""],
    ["E0803", "%s", "", ""],
    [
        "E0804",
        "stack overflow: call depth exceeded %d",
        "",
        "make the recursion a tail call or use `fold`",
    ],
    ["E0805", "not supported yet: %s", "", ""],
    ["E0806", "too many %s (limit %d)", "", ""],
    ["E0807", "assertion failed: %s", "", ""],
    [
        "E0808",
        "`%s` has no value: the statement that assigned it failed",
        "",
        "",
    ],
    ["W0101", "unused import `%s`", "", ""],
    [
        "W0102",
        "unused variable `%s`",
        "",
        "prefix it with `_` if it is intentional",
    ],
    ["W0103", "`%s` shadows a prelude function", "", ""],
    [
        "W0104",
        "`%s` is assigned as the last statement of a block used as a value, so the value is nil",
        "",
        "put the value last: end the block with an expression, e.g. the assigned name on its own line",
    ],
];

/// Looks up the catalog entry for a code.
pub fn entry(code: &str) -> Option<&'static Entry> {
    static CATALOG_MAP: OnceLock<BTreeMap<&'static str, Entry>> = OnceLock::new();
    CATALOG_MAP
        .get_or_init(|| {
            CATALOG
                .iter()
                .map(|[code, message, label, help]| {
                    (
                        *code,
                        Entry {
                            message: message.to_string(),
                            label: label.to_string(),
                            help: help.to_string(),
                        },
                    )
                })
                .collect()
        })
        .get(code)
}

#[cfg(test)]
const HEADER: &str = "\
# fung error and warning catalog. Generated by `FUNG_UPDATE_SPEC=1 cargo test spec_file`; do not edit.
# code<TAB>message template<TAB>underline label<TAB>help
";

/// The content `spec/errors.txt` must have.
#[cfg(test)]
fn render() -> String {
    let mut out = HEADER.to_string();
    for fields in CATALOG {
        for f in fields {
            assert!(
                !f.contains(['\t', '\n']),
                "{}: a catalog field contains a tab or newline: {f:?}",
                fields[0]
            );
        }
        out.push_str(&fields.join("\t"));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{CATALOG, render};

    #[test]
    fn codes_are_sorted_and_unique() {
        assert!(CATALOG.windows(2).all(|w| w[0][0] < w[1][0]));
    }

    #[test]
    fn spec_file_matches_the_catalog() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/errors.txt");
        let want = render();
        if std::env::var_os("FUNG_UPDATE_SPEC").is_some() {
            std::fs::write(path, &want).expect(path);
            return;
        }
        let got = std::fs::read_to_string(path).expect(path);
        assert!(
            got == want,
            "spec/errors.txt is out of date: run `FUNG_UPDATE_SPEC=1 cargo test spec_file`"
        );
    }
}
