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
    ["E0103", "unknown escape %q in string", "", ""],
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
    [
        "E0113",
        "duplicate key `%s` in a map literal or pattern",
        "",
        "",
    ],
    ["E0114", "test must be at the top level", "", ""],
    ["E0115", "expression nesting too deep (limit %d)", "", ""],
    ["E0116", "argument `%s` given twice", "", ""],
    [
        "E0117",
        "`%s()` has no arguments",
        "",
        "a tag without arguments is a symbol: write it without the parentheses",
    ],
    [
        "E0118",
        "a range cannot be an operand of another range",
        "",
        "`..` does not chain: put the inner range in parentheses if you mean it",
    ],
    ["E0119", "parameter `%s` is bound twice", "", ""],
    [
        "E0120",
        "a required parameter cannot follow an optional one",
        "",
        "move the parameters with a default to the end, or give this one a default too",
    ],
    [
        "E0121",
        "a tag takes its values by position, not by name",
        "",
        "write `Tag(a, b)`; for named fields put a map in it: `Tag({a: 1})`",
    ],
    [
        "E0122",
        "a string with `${…}` cannot be %s",
        "",
        "only a constant string fits here: drop the interpolation",
    ],
    [
        "E0123",
        "`_` cannot be a named argument",
        "",
        "`_` fills a position; for a named one write a lambda: `x => f(a: x)`",
    ],
    [
        "E0124",
        "an anonymous function cannot be written with `=`",
        "",
        "anonymous functions take a block: `fn(x): … ;;`, or write a lambda `x => …`",
    ],
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
        "`for` builds a list and cannot return early; handle the Err with `match`, or use `try_map`/`all`/`any`/`fold`",
    ],
    [
        "E0221",
        "name `%s` is bound twice in one pattern",
        "",
        "use a guard to compare the two positions: `[a, b] when a == b`",
    ],
    [
        "E0222",
        "`%s` is a built-in type and cannot be a type alias",
        "",
        "give the alias another name",
    ],
    [
        "E0223",
        "`if` without `else` used as a value: it is nil when the condition is false",
        "this if has no else",
        "add an `else:` branch; write `else: nil` if nil is meant (a function that returns nothing ends in `else: nil` or `nil`)",
    ],
    [
        "E0224",
        "`%s` is assigned as the last statement of a block used as a value, so the value is nil",
        "",
        "put the value last: end the block with an expression, e.g. the assigned name on its own line",
    ],
    [
        "E0225",
        "`%s` starts with an uppercase letter, so it is a tag and cannot name a variable",
        "this is a tag",
        "name variables in lowercase: `{HOME: home} = cfg`, `max_retries = 3`",
    ],
    [
        "E0226",
        "`?` inside a lambda given to %s",
        "",
        "the result keeps the Err as an element; with map use `try_map`, which stops at the first Err; with conc.pmap return the Err and look for it in the results",
    ],
    [
        "E0227",
        "`_` in a call on the right of `|>`",
        "",
        "the piped value is always the first argument; to put it elsewhere, use a lambda: `x |> (v => f(1, v))`",
    ],
    [
        "E0228",
        "the rest parameter `...%s` is always a list",
        "",
        "annotate it `list` (or not at all); `list[int]` names the elements without checking them",
    ],
    [
        "E0229",
        "`%s` on the right of `|>` applies to the call, not to the pipe",
        "",
        "put the pipe in parentheses: `(xs |> try_map(f))?`",
    ],
    ["E0230", "too many %s (limit %d)", "", ""],
    [
        "E0231",
        "a block used as a value %s, so the value is nil",
        "",
        "end the block with the value it stands for; write `nil` last if nil is meant",
    ],
    [
        "E0232",
        "`%s` takes no element type",
        "",
        "only `list` and `map` hold elements: write `list[str]`, `map[int]`, or the type alone",
    ],
    [
        "E0233",
        "the test name %q %s",
        "",
        "give each test in a file its own name: the report names a failed test by it",
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
        "only import, fn, type, test, and assignment are allowed at the top level of an imported module",
        "",
        "move the statement into a function, or into the entry file",
    ],
    ["E0307", "cannot import the entry file", "", ""],
    [
        "E0308",
        "cannot shadow module `%s`",
        "",
        "import it under another name: `import fs as m`",
    ],
    ["E0309", "name `%s` is already bound to a module", "", ""],
    ["E0310", "`%s` is internal; %s", "", ""],
    ["E0311", "import must be at the top level", "", ""],
    [
        "E0312",
        "cannot read module `%s`: %s",
        "",
        "fung source is UTF-8 text",
    ],
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
    ["E0507", "%s %d is out of range (length %d)", "", ""],
    [
        "E0508",
        "key `%s` not found",
        "",
        "use get(m, key, default) for a default",
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
        "map keys are strings: convert the key with `str(...)`, e.g. `group_by(xs, x => str(x % 2))`",
    ],
    ["E0515", "invalid pattern %s", "", ""],
    [
        "E0516",
        "slice start %d is after its end %d (length %d)",
        "",
        "",
    ],
    [
        "E0517",
        "cannot compare a symbol with a string: %s and %s",
        "",
        "they are never equal; a value from JSON, a file or a command is a string: convert it with `sym(s)`",
    ],
    [
        "E0518",
        "%s is not valid UTF-8: %s",
        "",
        "a string can hold any bytes, but this needs text; check where the bytes came from, or test them with `valid_utf8(s)`",
    ],
    [
        "E0519",
        "an `%s` value in %s: a result is not put into text silently",
        "",
        "unwrap it with `!` or `?` first; if the result itself is meant, write `str(x)`",
    ],
    [
        "E0520",
        "file `%s` is not open for %s",
        "",
        "open it with the mode the code needs: `fs.open(p, 'read)` to read, `'write` or `'append` to write",
    ],
    [
        "E0521",
        "the replacement names group `%s`, which the pattern does not have",
        "",
        "name a group the pattern has: `$1`, `${name}`; write `${1}a` for group 1 then `a` (`$1a` is the name `1a`), and `$$` for a `$`",
    ],
    ["E0601", "no pattern matched %s", "", ""],
    ["E0602", "value %s does not match the pattern", "", ""],
    ["E0603", "unhandled Err: %s", "", ""],
    ["E0604", "`%s` on nil", "", ""],
    [
        "E0605",
        "`%s` on %s, which is not a result (Ok(x) or Err(e))",
        "",
        "",
    ],
    [
        "E0606",
        "discarded Err: %s",
        "this value is discarded",
        "handle it with `!`, `?` or `match`, or discard it on purpose: `_ = ...`",
    ],
    [
        "E0607",
        "task's Err was never awaited: %s",
        "this task's result is discarded",
        "`await` it, or end the task's function with `nil`",
    ],
    ["E0701", "task cancelled", "", ""],
    ["E0702", "deadlock: all tasks are waiting", "", ""],
    // not raised since atoms became agents (E0704); the number stays taken
    [
        "E0703",
        "atom swapped inside its own swap",
        "",
        "not used: atoms became agents, see E0704",
    ],
    // not raised since agents became atoms again (E0707 covers it)
    [
        "E0704",
        "agent called from inside its own swap",
        "",
        "not used: agents became atoms, see E0707",
    ],
    ["E0705", "channel sent into itself", "", ""],
    ["E0706", "spawn into a closed scope", "", ""],
    [
        "E0707",
        "`%s` has an effect, which a swap's function must not have",
        "",
        "",
    ],
    [
        "E0708",
        "an atom cannot hold an atom, a channel end, a task, a scope, a file or a socket",
        "",
        "",
    ],
    [
        "E0709",
        "this value would make a cycle through channels or tasks, which could never be freed",
        "",
        "send or return the data, not the channel end or task handle that leads back here",
    ],
    ["E0801", "internal error: %v", "", ""],
    // not raised since the nesting limit became the syntax error E0115
    [
        "E0802",
        "expression nesting too deep (limit %d)",
        "",
        "not used: became the syntax error E0115",
    ],
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
    [
        "E0809",
        "`%s` is read before its assignment ran",
        "",
        "a top-level fn that reads it was called while the file was starting, above the assignment; move the assignment up or the call down",
    ],
    [
        "E0810",
        "a file dropped without `close` failed: %s",
        "",
        "close it with `close` (or use `fs.with_open`) to handle the error where it happens",
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
        "not used: became the error E0224",
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
    fn messages_match_what_the_language_allows() {
        // A module's top level also takes `test` blocks (04).
        let e0306 = super::entry("E0306").unwrap();
        assert!(e0306.message.contains("test"), "{}", e0306.message);
        // `m.k ?? d` stops at the missing key before `??` runs, so the
        // help names only `get`.
        let e0508 = super::entry("E0508").unwrap();
        assert!(!e0508.help.contains("??"), "{}", e0508.help);
        assert!(
            e0508.help.contains("get(m, key, default)"),
            "{}",
            e0508.help
        );
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
