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
    ["E0103", "unknown escape %s in string", "", ""],
    ["E0104", "unterminated string", "", ""],
    ["E0105", "malformed duration %q", "", ""],
    [
        "E0106",
        "`\\u{%s}` is not a Unicode scalar value",
        "",
        "`\\u{…}` takes a code point up to 10FFFF that is not a surrogate (D800–DFFF); for a byte, write `\\xHH`",
    ],
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
        "E0125",
        "`%s` binds looser than `..`, so it cannot be an end of a slice",
        "",
        "put the end in parentheses: `xs[(a or b)..]`",
    ],
    [
        "E0126",
        "a positional argument cannot follow a named one",
        "",
        "put the positional arguments first: `f(1, b: 2)`",
    ],
    [
        "E0127",
        "`??` and `|>` cannot be mixed without parentheses",
        "",
        "say which one goes first: `(x |> f) ?? d` or `x |> (f ?? d)`",
    ],
    [
        "E0128",
        "a symbol is not written with an uppercase letter: `'%s`",
        "",
        "an uppercase symbol is written as the tag, without the quote: `Red`",
    ],
    [
        "E0129",
        "`$%s` in a string is not interpolation",
        "",
        "write `${name}` for the value, or `\\$name` for the text",
    ],
    [
        "E0130",
        "a type annotation has no element type",
        "",
        "write `list` or `map` alone: the elements are not checked, so they are not written",
    ],
    [
        "E0131",
        "this statement comes after `return`, so it never runs",
        "never runs",
        "remove it, or move the `return` after it",
    ],
    [
        "E0132",
        "`return` used as a value",
        "",
        "`return` stands as a statement or ends a branch (`if`, `match`, `then … else`); compute the value first, then return it",
    ],
    [
        "E0133",
        "a spread `...` cannot stand %s",
        "",
        "`...xs` spreads a list or a range into a call's last positional argument: `f(a, ...xs, k: v)`; to join lists, use `+`",
    ],
    ["E0134", "`%s` is not a keyword in fung", "", ""],
    ["E0135", "number %q is out of range", "", ""],
    [
        "E0136",
        "a statement after another cannot begin its line with `-`",
        "",
        "to continue the line before, end that line with the `-`; a negative value on a line of its own is written in parentheses: `(-x)`",
    ],
    [
        "E0204",
        "undefined name `%s`",
        "not found in this scope",
        "",
    ],
    [
        "E0205",
        "cannot assign to `%s` here: it is bound outside this block",
        "",
        "a name is bound once, and a block cannot bind an outer one again; build the value with an expression instead, e.g. `x = if … ;;` or `fold`",
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
    // not raised since it was merged into E0238 (D88)
    [
        "E0209",
        "cannot reassign `%s`: it is a function or an imported name",
        "",
        "",
    ],
    ["E0210", "function `%s` is defined twice", "", ""],
    ["E0211", "%s %s", "", "use `exit()` to stop a script early"],
    ["E0212", "unknown type `%s`", "", ""],
    // not raised since it was merged into E0237 (D42)
    [
        "E0213",
        "return inside a for body",
        "",
        "not used: merged into E0237",
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
    // not raised since it was merged into E0237 (D42)
    [
        "E0220",
        "`?` inside a for body",
        "",
        "not used: merged into E0237",
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
        "a block used as a value %s, so the value is nil",
        "",
        "end the block with the value it stands for, e.g. the assigned name on its own line; write `nil` last if nil is meant",
    ],
    [
        "E0225",
        "`%s` starts with an uppercase letter, so it is a tag and cannot name a variable",
        "this is a tag",
        "name variables in lowercase: `{HOME: home} = cfg`, `max_retries = 3`",
    ],
    // not raised since it was merged into E0237 (D42)
    [
        "E0226",
        "`?` inside a lambda given to %s",
        "",
        "not used: merged into E0237",
    ],
    [
        "E0227",
        "`_` in a call on the right of `|>`",
        "",
        "the piped value is always the first argument; to put it elsewhere, call a parenthesised partial: `x |> (f(1, _))` (or a lambda: `x |> (v => f(1, v))`)",
    ],
    [
        "E0228",
        "the rest parameter `...%s` is always a list",
        "",
        "annotate it `list`, or not at all",
    ],
    [
        "E0229",
        "`%s` on the right of `|>` applies to the call, not to the pipe",
        "",
        "put the pipe in parentheses: `(xs |> try_map(f))?`",
    ],
    ["E0230", "too many %s (limit %d)", "", ""],
    // not raised since it was merged into E0224 (SP24)
    [
        "E0231",
        "a block used as a value %s, so the value is nil",
        "",
        "not used: merged into E0224",
    ],
    // not raised since element types left the language (D52, E0130)
    [
        "E0232",
        "`%s` takes no element type",
        "",
        "not used: element types are gone, see E0130",
    ],
    [
        "E0233",
        "the test name %q %s",
        "",
        "give each test in a file its own name: the report names a failed test by it",
    ],
    [
        "E0234",
        "%s is computed and discarded",
        "",
        "use the value, or write `_ = …` to discard it on purpose",
    ],
    [
        "E0235",
        "this arm %s",
        "",
        "move it last, or give it a guard: `n when n > 0 =>`",
    ],
    [
        "E0236",
        "`%s` takes exactly one value, found %d",
        "",
        "put several values in a tuple: `Ok((a, b))` or `Err((a, b))`",
    ],
    [
        "E0237",
        "`%s` inside %s",
        "",
        "a lambda `=>` and a `for` body cannot return early; write an anonymous `fn(x): … ;;` to return from, or use `try_map`, `find`, `all`/`any` or `fold`",
    ],
    [
        "E0238",
        "cannot bind `%s` again: a name is bound once",
        "",
        "give the new value a name of its own, or compute it in one expression (a pipe, `x = if … ;;`, `fold`)",
    ],
    [
        "E0239",
        "unused %s `%s`",
        "",
        "remove it, or start its name with `_` if it is kept on purpose",
    ],
    [
        "E0240",
        "`%s` hides %s",
        "",
        "a visible name means one thing: give the new one a name of its own",
    ],
    [
        "E0241",
        "this alternative never matches: %s",
        "",
        "remove it",
    ],
    [
        "E0242",
        "the test %q checks nothing: its body is empty",
        "",
        "give it an `assert`, or remove it",
    ],
    [
        "E0243",
        "`??` after %s: it is never nil or a result, so the right side is never used",
        "",
        "remove the `??`, or put it after the value that can be missing",
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
    [
        "E0309",
        "`%s` is already imported or bound to a module",
        "",
        "",
    ],
    ["E0310", "`%s` is internal; %s", "", ""],
    ["E0311", "import must be at the top level", "", ""],
    ["E0312", "cannot read module `%s`: %s", "", ""],
    [
        "E0313",
        "`import` after another statement",
        "",
        "put every import at the top of the file, before the other statements",
    ],
    ["E0401", "cannot call a value of type %s", "", ""],
    ["E0402", "%s takes %d argument(s), got %d", "", ""],
    ["E0403", "argument `%s` must be %s, got %s", "", ""],
    ["E0404", "return value must be %s, got %s", "", ""],
    ["E0405", "argument `%s` given twice", "", ""],
    ["E0406", "%s has no argument named `%s`", "", ""],
    ["E0407", "missing argument %s", "", ""],
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
    ["E0509", "cannot index %s with %s", "", ""],
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
        "map keys are strings: convert the key with `str(k)`",
    ],
    ["E0515", "invalid pattern %s", "", ""],
    [
        "E0516",
        "slice start %d is after its end %d (length %d)",
        "",
        "",
    ],
    // not raised since values of different types compare unequal (D91)
    [
        "E0517",
        "cannot compare values of different types: %s and %s",
        "",
        "they are never equal; convert one side first (`int(s)`, `str(n)`, `sym(s)`), or test the type with a pattern or `type_of`",
    ],
    [
        "E0518",
        "%s is not valid UTF-8: %s",
        "",
        "a string can hold any bytes, but this needs text; check where the bytes came from, or test them with `is_utf8(s)`",
    ],
    [
        "E0519",
        "%s in %s: it is not put into text silently",
        "",
        "unwrap a result with `!` or `?` and give `nil` a default with `??` first; to show any value as `print` does, write `str(x)`",
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
    [
        "E0522",
        "range start %d is after its end %d",
        "",
        "the empty ranges are `n..n` and `n..=n-1`; if the end may fall below the start, say so: `a..max(a, b)`",
    ],
    [
        "E0523",
        "the int %d has no exact float value",
        "",
        "a float holds every int up to 2^53 exactly; to round on purpose, write `float(n)`",
    ],
    [
        "E0524",
        "`%d ** %d`: an int to a negative int power is not an int",
        "",
        "write the exponent as a float: `2 ** -1.0`",
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
    // not raised since it was merged into E0709 (SP24)
    [
        "E0705",
        "channel sent into itself",
        "",
        "not used: merged into E0709",
    ],
    [
        "E0706",
        "`%s` on a closed scope",
        "",
        "use a scope's handle only inside the function `scope` or `supervise` runs",
    ],
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
    // not raised since every builtin was implemented
    [
        "E0805",
        "not supported yet: %s",
        "",
        "not used: every builtin is implemented",
    ],
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
    // not raised since unused names became the error E0239 (D38)
    [
        "W0101",
        "unused import `%s`",
        "",
        "not used: became the error E0239",
    ],
    // not raised since unused names became the error E0239 (D38)
    [
        "W0102",
        "unused variable `%s`",
        "",
        "not used: became the error E0239",
    ],
    // not raised since hiding a prelude function became the error E0240 (D38)
    [
        "W0103",
        "`%s` shadows a prelude function",
        "",
        "not used: became the error E0240",
    ],
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
