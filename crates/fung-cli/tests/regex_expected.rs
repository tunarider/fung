//! `regex` programs against `expected/regex/`: Hangul and other
//! multibyte text under `\w` `\b` `\d` `\s` with rune positions, patterns
//! that match the empty string in `find_all`, `split` and `replace`, every
//! form of `$` in a replacement, argument errors, and the whole E0515
//! report for bad patterns.

mod common;

use std::process::Command;

use common::expected::{self, Entry};
use common::{fung_bin, repo};

/// Subjects, each tried with every pattern in [`PATTERNS`].
const SUBJECTS: &[&str] = &[
    "",
    "a",
    "baaa",
    "aaa",
    "abc",
    "a1 b22 c333",
    "가나다 나다",
    "가a1 é٣_ x",
    "é가 a",
    "한글 text 123 ٣٤٥ café naïve",
    "  spaced\tout\n lines \u{a0}nbsp\u{2003}em ",
    "line one\nline two\n",
    "x=1, y=22 ,z=333,",
    "😀a😀b😀",
    "AbC aBc ÉÈ éè ǅ ǆ K k",
    "2026-10-04 and 1999-01-31",
];

/// Patterns, including ones that match the empty string.
const PATTERNS: &[&str] = &[
    "",
    "a*",
    "x*",
    "a*?",
    "a+",
    r"\b",
    r"\B",
    "^",
    "$",
    "(?m)^",
    "(?m)$",
    "|a",
    "a|",
    "(a)|b",
    "(a*)*",
    "(a)?",
    r"\w+",
    r"\w*",
    r"\W+",
    r"\d+",
    r"\D",
    r"\s+",
    r"\s*",
    r"\S+",
    r"[\w]+",
    r"[^\d\s]+",
    r"\pL+",
    r"\p{Hangul}+",
    r"\PL",
    "[가-힣]+",
    ".",
    "(?s).",
    "나(다)",
    "(?i)é",
    "(?i)k",
    "(?i)abc",
    "😀",
    r"(?P<key>\w+)=(?P<val>\d+)",
    r"\s*,\s*",
    r"(\d+)-(\d+)-(\d+)",
    r"(?P<y>\d{4})-(?P<m>\d\d)-(?P<d>\d\d)",
    r"(?P<x>a)(?P<y>b)?",
    r"(?U)a+",
    r"\b\w",
    r"\w\b",
    r"[[:alpha:]]+",
    r"[[:^space:]]+",
    r"\Aa",
    r"a\z",
    "e\u{301}",
];

/// Replacement strings: every form of `$` reference and the malformed ones
/// kept as text.
const REPLACEMENTS: &[&str] = &[
    "-",
    "",
    "<$0>",
    "$1",
    "${1}",
    "$1a",
    "${1}a",
    "$9",
    "$$",
    "$",
    "${",
    "${1",
    "${}",
    "$}",
    "$01",
    "${01}",
    "$00",
    "$key",
    "${key}",
    "$val!",
    "$y/$m/$d",
    "${d}.${m}.${y}",
    "$x",
    "$_",
    "$한",
    "${이름}",
    "$a-b",
    "$1$2$3",
    "$10000000000",
    "$99999999",
    "$100000000",
    "[$2|$1]",
    "$$1",
    "$$$1",
    "$é",
    // A group name is ASCII letters, digits and `_`: superscript two,
    // Roman numeral four and one half end it, so these read group 1.
    "$1²",
    "$1Ⅳ",
    "$1½",
];

/// `bin -e src args…`: stdout, stderr, status.
fn run(bin: &std::path::Path, src: &str, args: &[&str]) -> (String, String, Option<i32>) {
    let o = common::output(
        Command::new(bin)
            .env("TZ", common::TZ)
            .args(["-e", src])
            .args(args)
            .current_dir(repo()),
    )
    .unwrap();
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        o.status.code(),
    )
}

/// How many subject and pattern pairs and programs there are.
const PAIR_CASES: usize = 800;
const PROGRAM_CASES: usize = 46;

/// A run as an entry: standard output, standard error, exit code.
fn entry(label: String, (out, err, code): (String, String, Option<i32>)) -> Entry {
    Entry::new(label)
        .field("stdout", out)
        .field("stderr", err)
        .field("exit", code.map_or("signal".into(), |c| c.to_string()))
}

/// Every pair in one run: one entry per pair with its printed line, then
/// one with the run's standard error and exit code. The pairs and the
/// replacements travel in files, so no fung escaping gets in the way.
fn pair_entries(bin: &std::path::Path, pairs: &[String]) -> Vec<Entry> {
    let dir = std::env::temp_dir().join(format!("fung-regex-pairs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pairs_file = dir.join("pairs.txt");
    let repl_file = dir.join("repls.txt");
    std::fs::write(&pairs_file, pairs.join("\u{1e}")).unwrap();
    std::fs::write(&repl_file, REPLACEMENTS.join("\u{1e}")).unwrap();
    let src = "import regex\nimport fs\nrepls = split(fs.read(args[1])!, \"\\u{1e}\")\nfor rec in split(fs.read(args[0])!, \"\\u{1e}\"):\n  [s, p] = split(rec, \"\\u{1f}\")\n  print([regex.is_match(s, p), regex.find(s, p), regex.find_all(s, p), regex.split(s, p), map(repls, r => regex.replace(s, p, r))])\n;;\n";
    let args = [pairs_file.to_str().unwrap(), repl_file.to_str().unwrap()];
    let (out, err, code) = run(bin, src, &args);
    let _ = std::fs::remove_dir_all(&dir);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), pairs.len(), "{err}");
    let mut entries: Vec<Entry> = pairs
        .iter()
        .zip(lines)
        .map(|(p, l)| Entry::new(format!("{p:?}")).field("out", format!("{l}\n")))
        .collect();
    entries.push(entry("(the run)".into(), (String::new(), err, code)));
    entries
}

/// Every subject with every pattern: one line per pair with `is_match`,
/// `find`, `find_all`, `split` and `replace` with each replacement.
#[test]
fn regex_pairs_match_the_expected_output() {
    let mut pairs = Vec::new();
    for s in SUBJECTS {
        for p in PATTERNS {
            pairs.push(format!("{s}\u{1f}{p}"));
        }
    }
    assert_eq!(pairs.len(), PAIR_CASES, "pairs");
    let got = pair_entries(&fung_bin(), &pairs);
    expected::verify(
        "regex/pairs.txt",
        "`regex` on each subject and pattern (subject\\u{1f}pattern), one printed line each\n\
         (crates/fung-cli/tests/regex_expected.rs). Update: FUNG_EXPECTED=bless cargo test -p fung-cli --test regex_expected",
        64 * 1024,
        &got,
    );
}

/// Programs whose output or error report is compared whole.
const PROGRAMS: &[&str] = &[
    // positions in multibyte text, and slicing with them
    r#"s = "가나다 나다"
m = regex.find(s, "나(다)")
print(m.text, m.start, m.end, s[m.start..m.end], m.groups)"#,
    r#"for m in regex.find_all("é가 a b", `\b`):
  print(m)
;;"#,
    r#"print(regex.find_all("aaa", "a*"), regex.find_all("baaac", "a*"))"#,
    // named arguments and the direct bridge
    r#"print(regex.find(s: "k=v", pat: `(\w)=(\w)`), regex.replace(repl: "$2=$1", pat: `(\w)=(\w)`, s: "k=v"))"#,
    r#"import _native.regex as n
print(n.split("a,b", ","), n.is_match("x", "x"))"#,
    // argument errors: the subject first, then the pattern, which is
    // compiled before the replacement is checked
    r#"print(regex.is_match(1, "x"))"#,
    r#"print(regex.find("x", nil))"#,
    r#"print(regex.find_all(["x"], "x"))"#,
    r#"print(regex.split("x", 1.5))"#,
    r#"print(regex.replace(1, "(", 2))"#,
    r#"print(regex.replace("x", "(", 2))"#,
    r#"print(regex.replace("x", "x", 2))"#,
    r#"print(regex.replace("x", "x", 'sym))"#,
    r#"print(regex.find("x"))"#,
    r#"print(regex.split("x", "y", "z"))"#,
    r#"print(regex.find(s: "x", pattern: "x"))"#,
    // a bad pattern inside a function: the call path
    r#"fn f(p) = regex.find("x", p)
print(f("a)"))"#,
];

/// Bad patterns and ones at the edge of the syntax: the whole report.
const BAD_PATTERNS: &[&str] = &[
    "(",
    ")",
    "a)",
    "[a",
    "[z-a]",
    "a**",
    "a{1001}",
    "a{2}{1001}",
    "*a",
    "+",
    "?",
    "(?x)a",
    r"\Z",
    r"\8",
    r"\Q",
    r"\pN{",
    r"\p{Nope}",
    "(?P<>a)",
    "(?P<a",
    "(?P<a>x)(?P<a",
    "(?i",
    "(?z)",
    "x{1,2}{3,4}{5}",
    r"[[:nope:]]",
    "[]a",
    r"\",
    "a(?",
    "가(",
    "(가나]",
];

fn program_entries(bin: &std::path::Path) -> Vec<Entry> {
    let mut sources: Vec<String> = PROGRAMS
        .iter()
        .map(|p| format!("import regex\n{p}"))
        .collect();
    sources.extend(
        BAD_PATTERNS
            .iter()
            .map(|p| format!("import regex\nprint(regex.find(\"x\", `{p}`))")),
    );
    assert_eq!(sources.len(), PROGRAM_CASES, "programs");
    common::par_map(&sources, |src| {
        entry(
            expected::short_label(&format!("{src:?}")),
            run(bin, src, &[]),
        )
    })
}

#[test]
fn regex_programs_match_the_expected_output() {
    let got = program_entries(&fung_bin());
    expected::verify(
        "regex/programs.txt",
        "`regex` programs (crates/fung-cli/tests/regex_expected.rs), each run with `fung -e`.\n\
         Update: FUNG_EXPECTED=bless cargo test -p fung-cli --test regex_expected",
        64 * 1024,
        &got,
    );
}

/// The comparison is not vacuous: a bad pattern is E0515 with its reason.
#[test]
fn bad_pattern_report() {
    let (out, err, code) = run(
        &fung_bin(),
        "import regex\nprint(regex.find(\"x\", \"a(\"))",
        &[],
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (
            "",
            "error[E0515]: invalid pattern `a(`: unclosed group\n  --> -e:2:17\n  |\n2 | print(regex.find(\"x\", \"a(\"))\n  |                 ^\n  |\n call path (innermost first):\n   -e:2       <entry>\n",
            Some(1)
        )
    );
}
