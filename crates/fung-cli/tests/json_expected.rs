//! `json` programs against `expected/json/`: number kinds and their
//! limits, key order, duplicate keys, escapes, malformed input (the
//! message and its line and column), deep nesting, and what `stringify`
//! writes or rejects.

mod common;

use std::process::{Command, Stdio};

use common::expected::{self, Entry};
use common::{fung_bin, repo};

/// Inputs to `json.parse`, each printed with its result. Raw strings so
/// the JSON escapes reach the parser as written.
const INPUTS: &[&str] = &[
    // numbers: int or float by the literal, limits, signs
    "1",
    "1.0",
    "1e2",
    "1E2",
    "-0",
    "-0.0",
    "0.5",
    "9223372036854775807",
    "-9223372036854775808",
    "9223372036854775808",
    "-9223372036854775809",
    "123456789012345678901234567890",
    "1e400",
    "-1e400",
    "1e-400",
    "1.5e-7",
    "4.9e-324",
    "2.2250738585072014e-308",
    "1.7976931348623157e308",
    "1.7976931348623159e308",
    "0.1",
    "100000000000000000000000",
    "[1e21, 1e20, 123456789.0, 1e-5, 0.0001]",
    // key order and duplicates
    r#"{"z": 1, "y": 2, "x": {"b": 1, "a": 2}}"#,
    r#"{"a": 1, "a": 2}"#,
    r#"{"a": 1, "b": 2, "a": 3}"#,
    r#"{"A": 1, "a": 2}"#,
    r#"{"": 1, " ": 2}"#,
    r#"{"k1":1,"k2":2,"k3":3,"k4":4,"k5":5,"k6":6,"k7":7,"k8":8,"k9":9,"k10":10,"k11":11,"k12":12,"k13":13,"k14":14,"k15":15,"k16":16,"k17":17,"k1":0}"#,
    // strings and escapes
    r#""\u003c\u003e\u0026 \u2028\u2029""#,
    r#""\ud83d\ude00 \u00e9 \/ \b\f\n\r\t \" \\""#,
    r#""\ud800""#,
    r#""\ud800x""#,
    r#""\ud800\ud800""#,
    r#""\udc00""#,
    r#""\u0000""#,
    "\"é한😀\"",
    "\"\u{7f}\"",
    // whitespace, empty input, BOM
    "",
    " ",
    " \t\r\n 1 \n",
    "\u{feff}1",
    "\u{a0}1",
    // malformed: the message and where it points
    "{bad}",
    r#"{"a": 1,}"#,
    "[1, 2",
    "[1,",
    "[1",
    "[",
    "{",
    "{\"a\"",
    "{\"a\":",
    "{\"a\":1",
    "{\"a\":1,",
    "{\n  \"a\": tru\n}",
    "tru",
    "trux",
    "nul",
    "nullx",
    "null1",
    "[nul]",
    "{\"a\":nul}",
    "\"abc",
    "[\"a",
    "{\"a",
    "{\"a\":\"",
    "1 2",
    "1 ]",
    "1,",
    "[1]x",
    "[1]]",
    "{\"a\":1}}",
    "\"a\"x",
    "[1 2]",
    "{1:2}",
    "{true:1}",
    "{[]:1}",
    "{{}:1}",
    "]",
    "}",
    "[}",
    "{]",
    "{\"a\" 1}",
    "{\"a\":1 \"b\":2}",
    "{\"a\": [1, {\"b\": }]}",
    "{\"a\":\n  1\n  \"b\": 2}",
    "[,1]",
    "[1,,2]",
    "[1,2,3,]",
    "{,\"a\":1}",
    "{\"a\":1,,}",
    "{\"a\"::1}",
    "{\"a\",1}",
    ":",
    ",",
    "01",
    "[01]",
    "-",
    "[-]",
    "1.",
    "1.x",
    "1e",
    "[1e]",
    "1e+",
    ".5",
    "+1",
    "-x",
    "[1.5e",
    "{\"a\":-",
    r#""\x""#,
    r#""\u12""#,
    r#""\u12x"zz"#,
    r#""\ud800\u12""#,
    r#""\ud800\uzzzz""#,
    r#""\ue""#,
    r#"[1,{"a":[2,"\q"]}]"#,
    "\"a\tb\"",
    "\"\u{1}\"",
    "é",
    "[é]",
    "{\"é\":1,é}",
    "\n\n  [1,\n   é]",
    "가나다 ",
    "😀",
    "\u{2028}",
    "[\n1\n,\n2\nx",
    "NaN",
    "Infinity",
    "'a'",
    "[1]\n\n  x",
];

/// Inputs whose error falls at a 64-byte boundary (a character cut there
/// must still show whole), and errors right after a separator.
fn buffer_inputs() -> Vec<String> {
    let ones = |n: usize| "1,".repeat(n);
    vec![
        format!("[{}é]", ones(31)),
        format!("[{}1 é]", ones(30)),
        format!("[{}1é]", ones(31)),
        format!("{}é", " ".repeat(63)),
        format!("\"{}\\u\n00\"", "a".repeat(61)),
        format!("{{\"{}\":tré}}", "a".repeat(60)),
        format!("[{}é]", "\"é\",".repeat(40)),
        format!("[{}1 , 99999999999999999999]", ones(31)),
        format!("[{}\"\\u12\n\"]", ones(30)),
        "[1 , 99999999999999999999]".into(),
        "[1,  99999999999999999999]".into(),
        "\"\\é\"".into(),
        "{\"a\" : 1e999}".into(),
    ]
}

/// Writes `inputs` to a file joined by U+001E and runs a program that
/// parses each piece with each binary.
fn parse_all(bin: &std::path::Path, file: &std::path::Path) -> (String, String, Option<i32>) {
    let src = "import json\nimport fs\nfor s in split(fs.read(args[0])!, \"\\u{1e}\"):\n  print(json.parse(s))\n;;\n";
    run(bin, src, &[file.to_str().unwrap()])
}

fn run(bin: &std::path::Path, src: &str, args: &[&str]) -> (String, String, Option<i32>) {
    let o = Command::new(bin)
        .env("TZ", common::TZ)
        .args(["-e", src])
        .args(args)
        .current_dir(repo())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        o.status.code(),
    )
}

/// `stringify` programs, and parse programs whose input is easier to build
/// in fung (deep nesting, round trips).
const PROGRAMS: &[&str] = &[
    r#"print(json.stringify({b: 1, a: [true, nil, "x<y>&z", 2.0], t: (1, 2), s: 'ok', e: [], m: {}}))"#,
    r#"print(json.stringify("\u{2028}<>&\u{7f}\u{1}\u{8}\u{c}\n\r\t\"\\\u{fffd}\u{2029}é\u{1f}😀"))"#,
    r#"print(json.stringify([1e21, 1e20, 1e-7, 1e-4, 0.1, -0.0, 1.0, 123456789.0, 1.5e300, -2.5e-300, 9223372036854775807, -9223372036854775808]))"#,
    r#"print(json.stringify({"A": 1, "a": 2, "a b": 3, "é": 4, "\"": 5}))"#,
    r#"print(json.stringify(time.utc(time.from_unix(1791072307))), json.stringify([time.from_unix(0)]))"#,
    r#"print(json.stringify({a: [1, {b: 'sym}], t: (1, 2), e: [], m: {}}, indent: 2))"#,
    r#"print(json.stringify([[], {}, [[]]], indent: 1), json.stringify({}, indent: 0), json.stringify([1, 2], indent: 0), json.stringify(1, indent: 4))"#,
    r#"print(json.stringify("x", nil), json.stringify("x", indent: nil))"#,
    r#"print(json.stringify([], indent: 4611686018427387904), json.stringify(1, indent: 9223372036854775807))"#,
    r#"print(json.stringify([1], indent: 4611686018427387904))"#,
    r#"print(json.stringify([1], indent: 1000000000000000000))"#,
    r#"print(json.stringify({d: 5s}))"#,
    r#"print(json.stringify([1, 2s]))"#,
    r#"print(json.stringify({a: 1..2}))"#,
    r#"print(json.stringify(Ok(1)))"#,
    r#"print(json.stringify(print))"#,
    r#"print(json.stringify((x) => x))"#,
    r#"(_tx, rx) = channel(); print(json.stringify(rx))"#,
    r#"print(json.stringify(scope(s => spawn(s, () => 1))))"#,
    r#"print(json.stringify(json))"#,
    r#"print(json.stringify(1, indent: -1))"#,
    r#"print(json.stringify(1, indent: 1.0))"#,
    r#"print(json.stringify(1, indent: "2"))"#,
    r#"print(json.stringify(1, 2, 3))"#,
    r#"print(json.stringify())"#,
    r#"print(json.parse(1))"#,
    r#"print(json.parse(nil))"#,
    r#"print(json.parse())"#,
    r#"print(json.parse(s: "[1]"), json.stringify(v: [1], indent: 1))"#,
    // fs.open handles cannot be converted either
    r#"import fs
fs.with_open("Cargo.toml", (f) => print(json.stringify(f)))"#,
    // round trips keep order, kinds and text
    r#"s = `{"z":1,"y":{"b":2,"a":3},"x":[{"k":"v"}],"f":1.0,"i":-0,"n":null,"t":true,"u":"\u00e9\n"}`
v = json.parse(s)!
print(v, json.stringify(v), json.stringify(json.parse(json.stringify(v))!) == json.stringify(v))"#,
    // nesting: 10,000 levels are read
    r#"n = 10000
r = json.parse(fold(0..n, "", (acc, _) => acc + "[") + fold(0..n, "", (acc, _) => acc + "]"))
print(type_of(r))"#,
    r#"n = 10001
print(json.parse(fold(0..n, "", (acc, _) => acc + "[") + fold(0..n, "", (acc, _) => acc + "]")))"#,
    r#"print(json.parse(fold(0..200000, "", (acc, _) => acc + "{\"a\":")))"#,
    r#"print(json.parse(fold(0..20000, "", (acc, _) => acc + " [\n")))"#,
    // stringify of a deep value built in fung
    r#"v = fold(0..100000, [], (acc, i) => i % 2 == 0 then [acc] else {k: acc})
s = json.stringify(v)
print(len(s), take(chars(s), 12))"#,
    r#"v = fold(0..5, [], (acc, _) => [acc, 1])
print(json.stringify(v, indent: 1))"#,
];

/// How many parse inputs and programs there are.
const PARSE_CASES: usize = 143;
const PROGRAM_CASES: usize = 37;

/// A run as an entry: standard output, standard error, exit code.
fn entry(label: String, (out, err, code): (String, String, Option<i32>)) -> Entry {
    Entry::new(label)
        .field("stdout", out)
        .field("stderr", err)
        .field("exit", code.map_or("signal".into(), |c| c.to_string()))
}

/// Every input parsed in one run: one entry per input with its printed
/// line, then one with the run's standard error and exit code.
fn parse_entries(bin: &std::path::Path, inputs: &[String]) -> Vec<Entry> {
    let dir = std::env::temp_dir().join(format!("fung-json-inputs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("inputs.txt");
    std::fs::write(&file, inputs.join("\u{1e}")).unwrap();
    let (out, err, code) = parse_all(bin, &file);
    let _ = std::fs::remove_dir_all(&dir);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), inputs.len(), "{out}\n{err}");
    let mut entries: Vec<Entry> = inputs
        .iter()
        .zip(lines)
        .map(|(i, l)| {
            Entry::new(expected::short_label(&format!("{i:?}"))).field("out", format!("{l}\n"))
        })
        .collect();
    entries.push(entry("(the run)".into(), (String::new(), err, code)));
    entries
}

#[test]
fn json_parse_matches_the_expected_output() {
    let mut inputs: Vec<String> = INPUTS.iter().map(|s| s.to_string()).collect();
    inputs.extend(buffer_inputs());
    assert_eq!(inputs.len(), PARSE_CASES, "parse inputs");
    let got = parse_entries(&fung_bin(), &inputs);
    expected::verify(
        "json/parse.txt",
        "`json.parse` of each input, one printed line each (crates/fung-cli/tests/json_expected.rs).\n\
         Update: FUNG_EXPECTED=bless cargo test -p fung-cli --test json_expected",
        64 * 1024,
        &got,
    );
}

fn program_entries(bin: &std::path::Path) -> Vec<Entry> {
    PROGRAMS
        .iter()
        .map(|src| {
            let src = format!("import json\nimport time\n{src}");
            entry(
                expected::short_label(&format!("{src:?}")),
                run(bin, &src, &[]),
            )
        })
        .collect()
}

#[test]
fn json_programs_match_the_expected_output() {
    assert_eq!(PROGRAMS.len(), PROGRAM_CASES, "programs");
    let got = program_entries(&fung_bin());
    expected::verify(
        "json/programs.txt",
        "`json` programs (crates/fung-cli/tests/json_expected.rs), each run with `fung -e`.\n\
         Update: FUNG_EXPECTED=bless cargo test -p fung-cli --test json_expected",
        64 * 1024,
        &got,
    );
}
