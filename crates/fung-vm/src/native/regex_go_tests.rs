//! `compile_go` against Go 1.27. Every expected value below was printed
//! by Go (`regexp.Compile(pat)`'s `err.Error()` or `SubexpNames()`, and
//! `FindAllStringSubmatchIndex(s, -1)`), by a small generator program kept
//! outside the repository; nothing here was written by hand.

use super::*;

#[path = "regex_go_fuzz.rs"]
mod fuzz;

/// Runs `f` on a thread with a fung-sized stack: the regex crate's
/// compiler recurses once per nesting level, and Go accepts 999 levels.
fn on_big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

/// Go `Regexp.FindAllStringSubmatchIndex(s, -1)` over a Rust engine: the
/// loop of Go's `allMatches`, where an empty match right after the
/// previous match is skipped and the search then moves on one character.
fn go_all(re: &Regex, s: &str) -> Vec<Vec<isize>> {
    let mut out = Vec::new();
    let mut locs = re.capture_locations();
    let mut pos = 0;
    let mut prev_end: isize = -1;
    while pos <= s.len() {
        let Some(m) = re.captures_read_at(&mut locs, s, pos) else {
            break;
        };
        let mut accept = true;
        if m.end() == pos {
            if m.start() as isize == prev_end {
                accept = false;
            }
            pos += s[pos..].chars().next().map_or(1, char::len_utf8);
        } else {
            pos = m.end();
        }
        prev_end = m.end() as isize;
        if accept {
            let mut one = Vec::new();
            for i in 0..locs.len() {
                match locs.get(i) {
                    Some((a, b)) => one.extend([a as isize, b as isize]),
                    None => one.extend([-1, -1]),
                }
            }
            out.push(one);
        }
    }
    out
}

fn check_compile(table: &[(&str, Result<&[&str], &str>)]) {
    let mut bad = Vec::new();
    for &(pat, want) in table {
        let got = compile_go(pat);
        match (want, &got) {
            (Err(w), Err(g)) if w == g => {}
            (Ok(names), Ok(re)) => {
                let got_names = go_capture_names(re);
                if got_names != names {
                    bad.push(format!("{pat:?}: names {got_names:?}, Go {names:?}"));
                }
            }
            _ => bad.push(format!(
                "{pat:?}: got {:?}, Go {want:?}",
                got.as_ref().map(|_| "Ok")
            )),
        }
    }
    assert!(
        bad.is_empty(),
        "{} differences:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

fn check_matches(table: &[(&str, &str, &[&[isize]])]) {
    let mut bad = Vec::new();
    for &(pat, s, want) in table {
        let re = match compile_go(pat) {
            Ok(re) => re,
            Err(e) => {
                bad.push(format!("{pat:?}: {e}"));
                continue;
            }
        };
        let got = go_all(&re, s);
        if got != want {
            bad.push(format!("{pat:?} on {s:?}: got {got:?}, Go {want:?}"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} differences:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn compile_results_match_go() {
    check_compile(COMPILE);
}

#[test]
fn random_patterns_compile_like_go() {
    check_compile(fuzz::FUZZ_COMPILE);
}

#[test]
fn matches_follow_go() {
    check_matches(MATCHES);
}

#[test]
fn random_patterns_match_like_go() {
    check_matches(fuzz::FUZZ_MATCH);
}

/// One pattern per Go error code reachable under the Perl flags with a
/// `&str` pattern. `ErrInternalError` and `ErrInvalidCharClass` are never
/// returned by Go's parser, and `ErrInvalidUTF8` needs invalid UTF-8.
#[test]
fn every_reachable_error_code_is_covered() {
    for code in [
        "invalid character class range",
        "invalid escape sequence",
        "invalid named capture",
        "invalid or unsupported Perl syntax",
        "invalid nested repetition operator",
        "invalid repeat count",
        "missing closing ]",
        "missing closing )",
        "missing argument to repetition operator",
        "trailing backslash at end of expression",
        "unexpected )",
    ] {
        let prefix = format!("error parsing regexp: {code}: `");
        assert!(
            COMPILE
                .iter()
                .any(|(_, w)| w.is_err_and(|e| e.starts_with(&prefix))),
            "no case for {code}"
        );
    }
}

fn large(pat: &str) -> Result<(), String> {
    Err(format!(
        "error parsing regexp: expression too large: `{pat}`"
    ))
}

fn too_deep(pat: &str) -> Result<(), String> {
    Err(format!(
        "error parsing regexp: expression nests too deeply: `{pat}`"
    ))
}

/// Go's size and depth limits (`ErrLarge`, `ErrNestingDepth`): the whole
/// pattern is the expression in the message. Boundaries from Go 1.27.
#[test]
fn limits_match_go() {
    on_big_stack(|| {
        let ok = |p: &str| compile_go(p).map(|_| ());
        let nest = |n: usize, close: &str| "(".repeat(n) + "a" + &close.repeat(n);
        assert_eq!(ok(&nest(999, ")")), Ok(()));
        let p = nest(1000, ")");
        assert_eq!(ok(&p), too_deep(&p));
        assert_eq!(ok(&nest(499, ")*")), Ok(()));
        let p = nest(500, ")*");
        assert_eq!(ok(&p), too_deep(&p));
        let p = "a|(".repeat(1000) + &")".repeat(1000);
        assert_eq!(ok(&p), too_deep(&p));
        // Non-capturing groups add no level.
        assert_eq!(ok(&("(?:".repeat(1001) + "a" + &")".repeat(1001))), Ok(()));
        // The first error in the pattern wins.
        let p = "\\8".to_string() + &nest(1001, ")");
        assert_eq!(
            ok(&p),
            Err("error parsing regexp: invalid escape sequence: `\\8`".into())
        );
        let p = nest(1001, ")") + "\\8";
        assert_eq!(ok(&p), too_deep(&p));
        // Program size: Go counts instructions.
        let p = "a{1000}".repeat(3356);
        assert_eq!(ok(&p), large(&p));
        let p = "[a-z]{1000}".repeat(3356);
        assert_eq!(ok(&p), large(&p));
        // Class runes: \pL holds about 1,300 ranges; Go's parser takes
        // 20,000 of them, not 26,000.
        assert!(parse(&"\\pL".repeat(20000)).is_ok());
        let p = "\\pL".repeat(26000);
        assert_eq!(parse(&p).map(|_| ()), large(&p));
        assert_eq!(ok(&"a{1000}".repeat(30)), Ok(()));
        // Go's program fits its budget but the Rust engine's does not: a
        // known difference, reported with Go's message for too large.
        let p = "[a-z]{1000}".repeat(3355);
        assert!(parse(&p).is_ok());
        assert_eq!(ok(&p), large(&p));
        assert_eq!(ok(&"x".repeat(1_000_000)), Ok(()));
    });
}

/// Go accepts `\pL{1000}` (one instruction per class); the Rust engine
/// needs about 30 MB for it, inside the raised size limit.
#[test]
fn large_unicode_repetition_compiles() {
    let re = compile_go("\\pL{1000}").unwrap();
    assert!(re.is_match(&"é".repeat(1000)));
    assert!(!re.is_match(&"é".repeat(999)));
}

/// Truncated and garbled patterns never panic: every prefix and suffix of
/// every table pattern, and each with one metacharacter inserted at every
/// position. All go through the Go parser and the translation; every
/// seventh also through the Rust engine (slow in debug builds, so not the
/// variants of `\pL{1000}`).
#[test]
fn garbled_patterns_do_not_panic() {
    let pats = COMPILE
        .iter()
        .map(|p| p.0)
        .chain(fuzz::FUZZ_COMPILE.iter().map(|p| p.0));
    let mut n = 0;
    let mut try_one = |p: &str| {
        let _ = translate(p);
        n += 1;
        if n % 7 == 0 && !p.contains("1000") {
            let _ = compile_go(p);
        }
    };
    for pat in pats {
        let cuts: Vec<usize> = pat
            .char_indices()
            .map(|(i, _)| i)
            .chain([pat.len()])
            .collect();
        for &i in &cuts {
            try_one(&pat[..i]);
            try_one(&pat[i..]);
            for ins in [
                "(", ")", "[", "]", "{", "}", "\\", "*", "?", "-", "^", "|", "(?", "\\p{", "\u{e9}",
            ] {
                try_one(&format!("{}{ins}{}", &pat[..i], &pat[i..]));
            }
        }
    }
}

/// The fold orbits read from spec/regex_unicode.txt are in Go's
/// `unicode.SimpleFold` order: ascending, wrapping to the smallest.
#[test]
fn fold_orbits_follow_simple_fold() {
    assert_eq!(simple_fold('k' as i32), 0x212A);
    assert_eq!(simple_fold(0x212A), 'K' as i32);
    assert_eq!(simple_fold('K' as i32), 'k' as i32);
    assert_eq!(simple_fold('s' as i32), 0x17F);
    assert_eq!(simple_fold(0x17F), 'S' as i32);
    assert_eq!(simple_fold('1' as i32), '1' as i32);
    assert_eq!(simple_fold(-1), -1);
    assert_eq!(simple_fold(0x110000), 0x110000);
}

/// Go `regexp.Compile` results: `Ok(SubexpNames())` or `Err(err.Error())`.
const COMPILE: &[(&str, Result<&[&str], &str>)] = &[
    (
        "(?x)a",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?x`"),
    ),
    (
        "(?x)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?x`"),
    ),
    (
        "\\Z",
        Err("error parsing regexp: invalid escape sequence: `\\Z`"),
    ),
    ("\\z", Ok(&[""])),
    ("\\A", Ok(&[""])),
    (
        "\\G",
        Err("error parsing regexp: invalid escape sequence: `\\G`"),
    ),
    (
        "\\K",
        Err("error parsing regexp: invalid escape sequence: `\\K`"),
    ),
    (
        "\\e",
        Err("error parsing regexp: invalid escape sequence: `\\e`"),
    ),
    (
        "\\cA",
        Err("error parsing regexp: invalid escape sequence: `\\c`"),
    ),
    (
        "\\x{110000}",
        Err("error parsing regexp: invalid escape sequence: `\\x{110000`"),
    ),
    ("\\x{10FFFF}", Ok(&[""])),
    ("\\x{D800}", Ok(&[""])),
    ("[[:word:]]", Ok(&[""])),
    ("[[:alpha:]]", Ok(&[""])),
    (
        "[[:foo:]]",
        Err("error parsing regexp: invalid character class range: `[:foo:]`"),
    ),
    ("[a-z&&b]", Ok(&[""])),
    (
        "[a--b]",
        Err("error parsing regexp: invalid character class range: `a--`"),
    ),
    ("[a~~b]", Ok(&[""])),
    ("[[a]]", Ok(&[""])),
    ("[a[b]]", Ok(&[""])),
    ("[]a]", Ok(&[""])),
    ("[^]a]", Ok(&[""])),
    ("[]", Err("error parsing regexp: missing closing ]: `[]`")),
    ("[^]", Err("error parsing regexp: missing closing ]: `[^]`")),
    ("(?<name>x)", Ok(&["", "name"])),
    ("(?P<name>x)", Ok(&["", "name"])),
    (
        "(?P=name)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?P`"),
    ),
    (
        "(?'n'x)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?'`"),
    ),
    ("(?<n>x)(?<n>y)", Ok(&["", "n", "n"])),
    ("(?P<n>x)(?P<n>y)", Ok(&["", "n", "n"])),
    ("(?<1a>x)", Ok(&["", "1a"])),
    (
        "(?<a.b>x)",
        Err("error parsing regexp: invalid named capture: `(?<a.b>`"),
    ),
    (
        "(?<\u{e9}>x)",
        Err("error parsing regexp: invalid named capture: `(?<\u{e9}>`"),
    ),
    (
        "(?P<>x)",
        Err("error parsing regexp: invalid named capture: `(?P<>`"),
    ),
    (
        "(?P<n",
        Err("error parsing regexp: invalid named capture: `(?P<n`"),
    ),
    ("\\p{Greek}", Ok(&[""])),
    ("\\pN", Ok(&[""])),
    ("\\p{^Greek}", Ok(&[""])),
    ("\\PL", Ok(&[""])),
    ("\\P{^L}", Ok(&[""])),
    ("\\p{L}", Ok(&[""])),
    ("\\p{Letter}", Ok(&[""])),
    ("\\p{letter}", Ok(&[""])),
    ("\\p{Lc}", Ok(&[""])),
    ("\\p{LC}", Ok(&[""])),
    ("\\p{Any}", Ok(&[""])),
    ("\\p{Assigned}", Ok(&[""])),
    ("\\p{ASCII}", Ok(&[""])),
    (
        "\\p{Alphabetic}",
        Err("error parsing regexp: invalid character class range: `\\p{Alphabetic}`"),
    ),
    (
        "\\p{sc=Greek}",
        Err("error parsing regexp: invalid character class range: `\\p{sc=Greek}`"),
    ),
    (
        "\\p{Greek",
        Err("error parsing regexp: invalid character class range: `\\p{Greek`"),
    ),
    (
        "\\p{}",
        Err("error parsing regexp: invalid character class range: `\\p{}`"),
    ),
    ("\\pZ", Ok(&[""])),
    (
        "\\p\u{e9}",
        Err("error parsing regexp: invalid character class range: `\\p\u{e9}`"),
    ),
    (
        "\\p",
        Err("error parsing regexp: invalid character class range: `\\p`"),
    ),
    (
        "\\P",
        Err("error parsing regexp: invalid character class range: `\\P`"),
    ),
    ("(?i:x)", Ok(&[""])),
    ("(?i)", Ok(&[""])),
    ("(?U)", Ok(&[""])),
    (
        "(?u)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?u`"),
    ),
    (
        "(?-u:a)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?-u`"),
    ),
    ("(?s-i:a)", Ok(&[""])),
    (
        "(?i-)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?i-)`"),
    ),
    (
        "(?-)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?-)`"),
    ),
    ("(?i-i)", Ok(&[""])),
    (
        "(?",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?`"),
    ),
    ("(?:", Err("error parsing regexp: missing closing ): `(?:`")),
    ("(?)", Ok(&[""])),
    (
        "(?P",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?P`"),
    ),
    (
        "(?P>",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?P`"),
    ),
    (
        "x{2,1}",
        Err("error parsing regexp: invalid repeat count: `{2,1}`"),
    ),
    (
        "x{1001}",
        Err("error parsing regexp: invalid repeat count: `{1001}`"),
    ),
    ("x{1000}", Ok(&[""])),
    ("x{1000,}", Ok(&[""])),
    ("x{,5}", Ok(&[""])),
    ("x{01}", Ok(&[""])),
    (
        "x{1}{2}",
        Err("error parsing regexp: invalid nested repetition operator: `{1}{2}`"),
    ),
    ("x{", Ok(&[""])),
    ("x{a}", Ok(&[""])),
    (
        "{1}",
        Err("error parsing regexp: missing argument to repetition operator: `{1}`"),
    ),
    (
        "a{1}*",
        Err("error parsing regexp: invalid nested repetition operator: `{1}*`"),
    ),
    (
        "**",
        Err("error parsing regexp: missing argument to repetition operator: `*`"),
    ),
    (
        "a**",
        Err("error parsing regexp: invalid nested repetition operator: `**`"),
    ),
    (
        "a*+",
        Err("error parsing regexp: invalid nested repetition operator: `*+`"),
    ),
    ("a*?", Ok(&[""])),
    (
        "a+?+",
        Err("error parsing regexp: invalid nested repetition operator: `+?+`"),
    ),
    (
        "a???",
        Err("error parsing regexp: invalid nested repetition operator: `???`"),
    ),
    ("a??", Ok(&[""])),
    ("(?:)", Ok(&[""])),
    ("()", Ok(&["", ""])),
    ("(|)", Ok(&["", ""])),
    ("|", Ok(&[""])),
    (
        "a|*",
        Err("error parsing regexp: missing argument to repetition operator: `*`"),
    ),
    (
        "(*)",
        Err("error parsing regexp: missing argument to repetition operator: `*`"),
    ),
    (
        "(?i)*",
        Err("error parsing regexp: missing argument to repetition operator: `*`"),
    ),
    ("^*", Ok(&[""])),
    ("$+", Ok(&[""])),
    ("\\b*", Ok(&[""])),
    (
        "\\8",
        Err("error parsing regexp: invalid escape sequence: `\\8`"),
    ),
    ("\\0", Ok(&[""])),
    ("\\01", Ok(&[""])),
    ("\\012", Ok(&[""])),
    ("\\0123", Ok(&[""])),
    (
        "\\1",
        Err("error parsing regexp: invalid escape sequence: `\\1`"),
    ),
    ("\\08", Ok(&[""])),
    ("\\Q..\\E", Ok(&[""])),
    ("\\Qa*", Ok(&[""])),
    ("\\Q", Ok(&[""])),
    ("\\b{start}", Ok(&[""])),
    ("\\<", Ok(&[""])),
    ("\\>", Ok(&[""])),
    (
        "(?R)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?R`"),
    ),
    (
        "(?#c)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?#`"),
    ),
    (
        "(?=a)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?=`"),
    ),
    (
        "(?!a)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?!`"),
    ),
    (
        "(?<=a)",
        Err("error parsing regexp: invalid named capture: `(?<=a)`"),
    ),
    (
        "(?<!a)",
        Err("error parsing regexp: invalid named capture: `(?<!a)`"),
    ),
    (
        "(?>a)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?>`"),
    ),
    (
        "(?|a)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?|`"),
    ),
    (
        "\\",
        Err("error parsing regexp: trailing backslash at end of expression: ``"),
    ),
    (
        "a\\",
        Err("error parsing regexp: trailing backslash at end of expression: ``"),
    ),
    ("[a", Err("error parsing regexp: missing closing ]: `[a`")),
    ("[a-", Err("error parsing regexp: missing closing ]: `[a-`")),
    ("[a-]", Ok(&[""])),
    ("[-a]", Ok(&[""])),
    (
        "[z-a]",
        Err("error parsing regexp: invalid character class range: `z-a`"),
    ),
    ("[\\d-z]", Ok(&[""])),
    (
        "[a-\\d]",
        Err("error parsing regexp: invalid escape sequence: `\\d`"),
    ),
    ("[\\w]", Ok(&[""])),
    ("[^\\w]", Ok(&[""])),
    ("[\\W]", Ok(&[""])),
    ("\\pL{1000}", Ok(&[""])),
    ("(a)", Ok(&["", ""])),
    ("a)", Err("error parsing regexp: unexpected ): `a)`")),
    (")", Err("error parsing regexp: unexpected ): `)`")),
    ("(a", Err("error parsing regexp: missing closing ): `(a`")),
    (
        "((a)",
        Err("error parsing regexp: missing closing ): `((a)`"),
    ),
    (
        "\\x",
        Err("error parsing regexp: invalid escape sequence: `\\x`"),
    ),
    (
        "\\x1",
        Err("error parsing regexp: invalid escape sequence: `\\x1`"),
    ),
    (
        "\\xZZ",
        Err("error parsing regexp: invalid escape sequence: `\\xZZ`"),
    ),
    (
        "\\x{}",
        Err("error parsing regexp: invalid escape sequence: `\\x{}`"),
    ),
    (
        "\\x{zz}",
        Err("error parsing regexp: invalid escape sequence: `\\x{z`"),
    ),
    (
        "\\x{41",
        Err("error parsing regexp: invalid escape sequence: `\\x{41`"),
    ),
    ("A", Ok(&[""])),
    (
        "\\U00000041",
        Err("error parsing regexp: invalid escape sequence: `\\U`"),
    ),
    (
        "\\N{LATIN}",
        Err("error parsing regexp: invalid escape sequence: `\\N`"),
    ),
    (
        "\\C",
        Err("error parsing regexp: invalid escape sequence: `\\C`"),
    ),
    (
        "\\X",
        Err("error parsing regexp: invalid escape sequence: `\\X`"),
    ),
    (
        "\\R",
        Err("error parsing regexp: invalid escape sequence: `\\R`"),
    ),
    (
        "\\h",
        Err("error parsing regexp: invalid escape sequence: `\\h`"),
    ),
    ("\\v", Ok(&[""])),
    (
        "\\V",
        Err("error parsing regexp: invalid escape sequence: `\\V`"),
    ),
    (
        "\\H",
        Err("error parsing regexp: invalid escape sequence: `\\H`"),
    ),
    ("\\a\\f\\t\\n\\r", Ok(&[""])),
    ("\\_", Ok(&[""])),
    ("\\-", Ok(&[""])),
    ("\\:", Ok(&[""])),
    ("\\#", Ok(&[""])),
    ("\\ ", Ok(&[""])),
    (
        "\\\u{e9}",
        Err("error parsing regexp: invalid escape sequence: `\\\u{e9}`"),
    ),
    (
        "\\y",
        Err("error parsing regexp: invalid escape sequence: `\\y`"),
    ),
    (
        "\\o{101}",
        Err("error parsing regexp: invalid escape sequence: `\\o`"),
    ),
    (
        "[\\b]",
        Err("error parsing regexp: invalid escape sequence: `\\b`"),
    ),
    (
        "[\\B]",
        Err("error parsing regexp: invalid escape sequence: `\\B`"),
    ),
    (
        "[\\A]",
        Err("error parsing regexp: invalid escape sequence: `\\A`"),
    ),
    (
        "[\\z]",
        Err("error parsing regexp: invalid escape sequence: `\\z`"),
    ),
    (
        "[\\Q]",
        Err("error parsing regexp: invalid escape sequence: `\\Q`"),
    ),
    ("[\\pL]", Ok(&[""])),
    ("[\\p{Greek}a]", Ok(&[""])),
    ("[[:^alpha:]]", Ok(&[""])),
    ("[[=a=]]", Ok(&[""])),
    ("[[.a.]]", Ok(&[""])),
    ("[a-z[:digit:]]", Ok(&[""])),
    ("[:alpha:]", Ok(&[""])),
    ("(?i)k", Ok(&[""])),
    (".*", Ok(&[""])),
    ("\\d+", Ok(&[""])),
    ("\\s", Ok(&[""])),
    ("\\S", Ok(&[""])),
    ("\\w\\W\\D", Ok(&[""])),
    ("(?m)^a$", Ok(&[""])),
    ("(?sm).", Ok(&[""])),
    (
        "x{2}{3}",
        Err("error parsing regexp: invalid nested repetition operator: `{2}{3}`"),
    ),
    ("(x{2}){3}", Ok(&["", ""])),
    ("(?:x{100}){10}", Ok(&[""])),
    (
        "(?:x{100}){11}",
        Err("error parsing regexp: invalid repeat count: `{11}`"),
    ),
    ("((x{10}){10}){10}", Ok(&["", "", ""])),
    (
        "((x{10}){10}){11}",
        Err("error parsing regexp: invalid repeat count: `{11}`"),
    ),
    (
        "x{0}{2000}",
        Err("error parsing regexp: invalid repeat count: `{2000}`"),
    ),
    (
        "(?:x{0}){1001}",
        Err("error parsing regexp: invalid repeat count: `{1001}`"),
    ),
    ("", Ok(&[""])),
    (
        "a**b",
        Err("error parsing regexp: invalid nested repetition operator: `**`"),
    ),
    ("\\x{41}", Ok(&[""])),
    (
        "a{1000}{1}",
        Err("error parsing regexp: invalid nested repetition operator: `{1000}{1}`"),
    ),
    ("\u{ff}", Ok(&[""])),
    ("\u{e9}", Ok(&[""])),
    (
        "(?P<n>",
        Err("error parsing regexp: missing closing ): `(?P<n>`"),
    ),
    (
        "(?<n",
        Err("error parsing regexp: invalid named capture: `(?<n`"),
    ),
    ("(?i)\u{212a}", Ok(&[""])),
    (
        "[\\x{10FFFF}-\\x{0}]",
        Err("error parsing regexp: invalid character class range: `\\x{10FFFF}-\\x{0}`"),
    ),
    (
        "\\8\\9",
        Err("error parsing regexp: invalid escape sequence: `\\8`"),
    ),
    (
        "a(?<n",
        Err("error parsing regexp: invalid named capture: `(?<n`"),
    ),
    (
        "x(?P<a.b>y)",
        Err("error parsing regexp: invalid named capture: `(?P<a.b>`"),
    ),
    ("ab[c", Err("error parsing regexp: missing closing ]: `[c`")),
    ("ab[^", Err("error parsing regexp: missing closing ]: `[^`")),
    ("ab[]", Err("error parsing regexp: missing closing ]: `[]`")),
    (
        "a[\\",
        Err("error parsing regexp: trailing backslash at end of expression: ``"),
    ),
    (
        "[\\",
        Err("error parsing regexp: trailing backslash at end of expression: ``"),
    ),
    (
        "[a-\\",
        Err("error parsing regexp: trailing backslash at end of expression: ``"),
    ),
    (
        "(?i",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?i`"),
    ),
    (
        "(?i-s",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?i-s`"),
    ),
    (
        "a(?z)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?z`"),
    ),
    ("(?im-sU:x)", Ok(&[""])),
    ("(?m:^a$)", Ok(&[""])),
    ("(?s:.)", Ok(&[""])),
    ("(?-i)", Ok(&[""])),
    (
        "(?--i)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?--`"),
    ),
    (
        "(?i-:x)",
        Err("error parsing regexp: invalid or unsupported Perl syntax: `(?i-:`"),
    ),
    ("a)b(", Err("error parsing regexp: unexpected ): `a)b(`")),
    (
        "((a)|b",
        Err("error parsing regexp: missing closing ): `((a)|b`"),
    ),
    ("a|b)", Err("error parsing regexp: unexpected ): `a|b)`")),
    (
        "(a|b))",
        Err("error parsing regexp: unexpected ): `(a|b))`"),
    ),
    ("a{2}{", Ok(&[""])),
    ("a{2}?", Ok(&[""])),
    (
        "a{2}??",
        Err("error parsing regexp: invalid nested repetition operator: `{2}??`"),
    ),
    ("a{2,}?{", Ok(&[""])),
    (
        "x{1000,1001}",
        Err("error parsing regexp: invalid repeat count: `{1000,1001}`"),
    ),
    ("x{2,1000}", Ok(&[""])),
    (
        "x{100000000}",
        Err("error parsing regexp: invalid repeat count: `{100000000}`"),
    ),
    (
        "x{99999999}",
        Err("error parsing regexp: invalid repeat count: `{99999999}`"),
    ),
    (
        "x{1,99999999999}",
        Err("error parsing regexp: invalid repeat count: `{1,99999999999}`"),
    ),
    (
        "x{0,1001}",
        Err("error parsing regexp: invalid repeat count: `{0,1001}`"),
    ),
    ("(?:x{2}){500}", Ok(&[""])),
    (
        "(?:x{2}){501}",
        Err("error parsing regexp: invalid repeat count: `{501}`"),
    ),
    (
        "a|*b",
        Err("error parsing regexp: missing argument to repetition operator: `*`"),
    ),
    (
        "(?:*)",
        Err("error parsing regexp: missing argument to repetition operator: `*`"),
    ),
    (
        "(|*)",
        Err("error parsing regexp: missing argument to repetition operator: `*`"),
    ),
    ("^?", Ok(&[""])),
    ("\\b+", Ok(&[""])),
    (
        "\\Q\\E*",
        Err("error parsing regexp: missing argument to repetition operator: `*`"),
    ),
    ("\\Qab\\E*", Ok(&[""])),
    ("[[:alpha:]-z]", Ok(&[""])),
    (
        "[a-[:alpha:]]",
        Err("error parsing regexp: invalid character class range: `a-[`"),
    ),
    ("[\\pL-z]", Ok(&[""])),
    (
        "[a-\\pL]",
        Err("error parsing regexp: invalid escape sequence: `\\p`"),
    ),
    (
        "[\\x{41}-\\x{40}]",
        Err("error parsing regexp: invalid character class range: `\\x{41}-\\x{40}`"),
    ),
    (
        "[\\0-\\1]",
        Err("error parsing regexp: invalid escape sequence: `\\1`"),
    ),
    ("[\\08]", Ok(&[""])),
    (
        "[\\x]",
        Err("error parsing regexp: invalid escape sequence: `\\x]`"),
    ),
    (
        "[\\x{110000}]",
        Err("error parsing regexp: invalid escape sequence: `\\x{110000`"),
    ),
    (
        "[\\8]",
        Err("error parsing regexp: invalid escape sequence: `\\8`"),
    ),
    (
        "[a-\\8]",
        Err("error parsing regexp: invalid escape sequence: `\\8`"),
    ),
    (
        "[\\Z]",
        Err("error parsing regexp: invalid escape sequence: `\\Z`"),
    ),
    (
        "\\pL\\p",
        Err("error parsing regexp: invalid character class range: `\\p`"),
    ),
    (
        "[\\p{Foo}]",
        Err("error parsing regexp: invalid character class range: `\\p{Foo}`"),
    ),
    (
        "[\\p{Greek]",
        Err("error parsing regexp: invalid character class range: `\\p{Greek]`"),
    ),
    (
        "[[:alpha:]",
        Err("error parsing regexp: missing closing ]: `[[:alpha:]`"),
    ),
    (
        "[[:alpha:",
        Err("error parsing regexp: missing closing ]: `[[:alpha:`"),
    ),
    (
        "[[:foo:]",
        Err("error parsing regexp: invalid character class range: `[:foo:]`"),
    ),
    (
        "[[:^foo:]]",
        Err("error parsing regexp: invalid character class range: `[:^foo:]`"),
    ),
    ("[[:]]", Ok(&[""])),
    (
        "[[::]]",
        Err("error parsing regexp: invalid character class range: `[::]`"),
    ),
    ("\\p{greek}", Ok(&[""])),
    ("\\p{GREEK}", Ok(&[""])),
    ("\\p{Old_Italic}", Ok(&[""])),
    ("\\p{olditalic}", Ok(&[""])),
    ("\\p{Old Italic}", Ok(&[""])),
    ("\\p{old-italic}", Ok(&[""])),
    ("\\p{ lu }", Ok(&[""])),
    ("\\p{L u}", Ok(&[""])),
    ("\\p{Uppercase_Letter}", Ok(&[""])),
    ("\\p{uppercaseletter}", Ok(&[""])),
    ("\\p{cntrl}", Ok(&[""])),
    ("\\p{digit}", Ok(&[""])),
    ("\\p{punct}", Ok(&[""])),
    ("\\p{ascii}", Ok(&[""])),
    ("\\p{any}", Ok(&[""])),
    ("\\p{assigned}", Ok(&[""])),
    ("\\p{^Any}", Ok(&[""])),
    ("\\P{^Any}", Ok(&[""])),
    ("\\p{Cased_Letter}", Ok(&[""])),
    ("\\p{Lu}", Ok(&[""])),
    ("\\p{lu}", Ok(&[""])),
    ("\\p{LU}", Ok(&[""])),
    ("\\pl", Ok(&[""])),
    ("\\pC", Ok(&[""])),
    ("\\p{Han}", Ok(&[""])),
    ("\\p{Hangul}", Ok(&[""])),
    ("\\p{Common}", Ok(&[""])),
    ("\\p{Inherited}", Ok(&[""])),
    (
        "\\p{Zzzz}",
        Err("error parsing regexp: invalid character class range: `\\p{Zzzz}`"),
    ),
    (
        "\\p{Unknown}",
        Err("error parsing regexp: invalid character class range: `\\p{Unknown}`"),
    ),
    (
        "\\p{Latin1}",
        Err("error parsing regexp: invalid character class range: `\\p{Latin1}`"),
    ),
    (
        "\\p{^}",
        Err("error parsing regexp: invalid character class range: `\\p{^}`"),
    ),
    (
        "\\p{^^L}",
        Err("error parsing regexp: invalid character class range: `\\p{^^L}`"),
    ),
    ("\\x{0}", Ok(&[""])),
    ("\\x{000041}", Ok(&[""])),
    ("\\x41", Ok(&[""])),
    (
        "\\x4",
        Err("error parsing regexp: invalid escape sequence: `\\x4`"),
    ),
    (
        "\\x{FFFFFFFFF}",
        Err("error parsing regexp: invalid escape sequence: `\\x{FFFFFF`"),
    ),
    ("\\x{10ffff}", Ok(&[""])),
    (
        "\\x{-1}",
        Err("error parsing regexp: invalid escape sequence: `\\x{-`"),
    ),
    (
        "\\7",
        Err("error parsing regexp: invalid escape sequence: `\\7`"),
    ),
    ("\\77", Ok(&[""])),
    ("\\777", Ok(&[""])),
    ("\\7777", Ok(&[""])),
    (
        "\\18",
        Err("error parsing regexp: invalid escape sequence: `\\1`"),
    ),
    (
        "\\9",
        Err("error parsing regexp: invalid escape sequence: `\\9`"),
    ),
    ("(?P<name>a)(?P<other>b)(c)", Ok(&["", "name", "other", ""])),
    ("(?P<n1>a)|(?P<n2>b)", Ok(&["", "n1", "n2"])),
    ("(?P<_>x)", Ok(&["", "_"])),
    ("(?P<__init__>x)", Ok(&["", "__init__"])),
    ("(?<A1_b2>x)", Ok(&["", "A1_b2"])),
    (
        "(?P<na me>x)",
        Err("error parsing regexp: invalid named capture: `(?P<na me>`"),
    ),
    (
        "(?P<n>x",
        Err("error parsing regexp: missing closing ): `(?P<n>x`"),
    ),
    ("(?:(?:(?:a)))", Ok(&[""])),
    ("(a(b(c)))", Ok(&["", "", "", ""])),
    ("a{,}", Ok(&[""])),
    ("a{1,2,3}", Ok(&[""])),
    ("a{ 1}", Ok(&[""])),
    ("a{1 }", Ok(&[""])),
    ("{", Ok(&[""])),
    ("}", Ok(&[""])),
    ("]", Ok(&[""])),
    ("a]", Ok(&[""])),
    ("{}", Ok(&[""])),
    ("a{}", Ok(&[""])),
    ("x{2}{,}", Ok(&[""])),
    ("(?i)\u{1c5}", Ok(&[""])),
    ("(?i)[\u{1c5}]", Ok(&[""])),
    ("(?i)[^\u{1c5}]", Ok(&[""])),
    ("[Aa]", Ok(&[""])),
    ("[Kk]", Ok(&[""])),
    ("[k\u{212a}]", Ok(&[""])),
];

/// Go `FindAllStringSubmatchIndex(s, -1)` (byte offsets, -1 for a group
/// that did not take part).
const MATCHES: &[(&str, &str, &[&[isize]])] = &[
    (
        "\\w+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[0, 1],
            &[3, 6],
            &[14, 15],
            &[17, 22],
            &[26, 27],
            &[29, 30],
            &[33, 34],
        ],
    ),
    (
        "\\W+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[1, 3],
            &[6, 14],
            &[15, 17],
            &[22, 26],
            &[27, 29],
            &[30, 33],
        ],
    ),
    (
        "\\d+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[&[21, 22], &[33, 34]],
    ),
    (
        "\\D+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[&[0, 21], &[22, 33]],
    ),
    (
        "[\\w]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[0, 1],
            &[3, 6],
            &[14, 15],
            &[17, 22],
            &[26, 27],
            &[29, 30],
            &[33, 34],
        ],
    ),
    (
        "[^\\w]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[1, 3],
            &[6, 14],
            &[15, 17],
            &[22, 26],
            &[27, 29],
            &[30, 33],
        ],
    ),
    (
        "[\\W]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[1, 3],
            &[6, 14],
            &[15, 17],
            &[22, 26],
            &[27, 29],
            &[30, 33],
        ],
    ),
    (
        "[^\\W]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[0, 1],
            &[3, 6],
            &[14, 15],
            &[17, 22],
            &[26, 27],
            &[29, 30],
            &[33, 34],
        ],
    ),
    (
        "[^\\d]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[&[0, 21], &[22, 33]],
    ),
    (
        "[\\d]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[&[21, 22], &[33, 34]],
    ),
    (
        "\\b",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[0, 0],
            &[1, 1],
            &[3, 3],
            &[6, 6],
            &[14, 14],
            &[15, 15],
            &[17, 17],
            &[22, 22],
            &[26, 26],
            &[27, 27],
            &[29, 29],
            &[30, 30],
            &[33, 33],
            &[34, 34],
        ],
    ),
    (
        "\\B",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[4, 4],
            &[5, 5],
            &[7, 7],
            &[10, 10],
            &[13, 13],
            &[18, 18],
            &[19, 19],
            &[20, 20],
            &[21, 21],
            &[23, 23],
            &[25, 25],
            &[31, 31],
        ],
    ),
    (
        "\\b\\w+\\b",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[0, 1],
            &[3, 6],
            &[14, 15],
            &[17, 22],
            &[26, 27],
            &[29, 30],
            &[33, 34],
        ],
    ),
    (
        "\u{e9}\\b",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[&[1, 3], &[31, 33]],
    ),
    (
        "\\b\u{e9}",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[&[1, 3]],
    ),
    (
        "[[:word:]]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[0, 1],
            &[3, 6],
            &[14, 15],
            &[17, 22],
            &[26, 27],
            &[29, 30],
            &[33, 34],
        ],
    ),
    (
        "[[:alpha:]]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[&[0, 1], &[3, 6], &[14, 15], &[17, 20], &[26, 27], &[29, 30]],
    ),
    (
        "[[:^alpha:]]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[1, 3],
            &[6, 14],
            &[15, 17],
            &[20, 26],
            &[27, 29],
            &[30, 34],
        ],
    ),
    (
        "\\pL+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[0, 6],
            &[7, 13],
            &[14, 20],
            &[26, 27],
            &[29, 30],
            &[31, 33],
        ],
    ),
    (
        "\\PL+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[6, 7],
            &[13, 14],
            &[20, 26],
            &[27, 29],
            &[30, 31],
            &[33, 34],
        ],
    ),
    (
        "\\pN+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[&[21, 22], &[23, 25], &[27, 29], &[33, 34]],
    ),
    (
        "\\p{Hangul}+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[&[7, 13]],
    ),
    (
        "[\\pL\\d]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[0, 6],
            &[7, 13],
            &[14, 20],
            &[21, 22],
            &[26, 27],
            &[29, 30],
            &[31, 34],
        ],
    ),
    (
        "[^\\pL]+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[6, 7],
            &[13, 14],
            &[20, 26],
            &[27, 29],
            &[30, 31],
            &[33, 34],
        ],
    ),
    (
        "\\p{^L}+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[6, 7],
            &[13, 14],
            &[20, 26],
            &[27, 29],
            &[30, 31],
            &[33, 34],
        ],
    ),
    (
        "\\P{^L}+",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[0, 6],
            &[7, 13],
            &[14, 20],
            &[26, 27],
            &[29, 30],
            &[31, 33],
        ],
    ),
    (
        ".",
        "h\u{e9}llo \u{d55c}\u{ae00} w\u{f6}rld_1 \u{663} x\u{663}y \u{e9}9",
        &[
            &[0, 1],
            &[1, 3],
            &[3, 4],
            &[4, 5],
            &[5, 6],
            &[6, 7],
            &[7, 10],
            &[10, 13],
            &[13, 14],
            &[14, 15],
            &[15, 17],
            &[17, 18],
            &[18, 19],
            &[19, 20],
            &[20, 21],
            &[21, 22],
            &[22, 23],
            &[23, 25],
            &[25, 26],
            &[26, 27],
            &[27, 29],
            &[29, 30],
            &[30, 31],
            &[31, 33],
            &[33, 34],
        ],
    ),
    (
        "\\s",
        "a\u{a0}b\u{2003}c\u{9}d\u{a}e\u{b}f\u{c}g\u{d}h i\u{85}j",
        &[&[8, 9], &[10, 11], &[14, 15], &[16, 17], &[18, 19]],
    ),
    (
        "\\S+",
        "a\u{a0}b\u{2003}c\u{9}d\u{a}e\u{b}f\u{c}g\u{d}h i\u{85}j",
        &[
            &[0, 8],
            &[9, 10],
            &[11, 14],
            &[15, 16],
            &[17, 18],
            &[19, 23],
        ],
    ),
    (
        "[\\s]",
        "a\u{a0}b\u{2003}c\u{9}d\u{a}e\u{b}f\u{c}g\u{d}h i\u{85}j",
        &[&[8, 9], &[10, 11], &[14, 15], &[16, 17], &[18, 19]],
    ),
    (
        "[^\\s]+",
        "a\u{a0}b\u{2003}c\u{9}d\u{a}e\u{b}f\u{c}g\u{d}h i\u{85}j",
        &[
            &[0, 8],
            &[9, 10],
            &[11, 14],
            &[15, 16],
            &[17, 18],
            &[19, 23],
        ],
    ),
    (
        "[[:space:]]",
        "a\u{a0}b\u{2003}c\u{9}d\u{a}e\u{b}f\u{c}g\u{d}h i\u{85}j",
        &[
            &[8, 9],
            &[10, 11],
            &[12, 13],
            &[14, 15],
            &[16, 17],
            &[18, 19],
        ],
    ),
    (
        "\\pZ",
        "a\u{a0}b\u{2003}c\u{9}d\u{a}e\u{b}f\u{c}g\u{d}h i\u{85}j",
        &[&[1, 3], &[4, 7], &[18, 19]],
    ),
    (
        "\\v",
        "a\u{a0}b\u{2003}c\u{9}d\u{a}e\u{b}f\u{c}g\u{d}h i\u{85}j",
        &[&[12, 13]],
    ),
    (
        "[\\v]",
        "a\u{a0}b\u{2003}c\u{9}d\u{a}e\u{b}f\u{c}g\u{d}h i\u{85}j",
        &[&[12, 13]],
    ),
    (
        "\\x0b",
        "a\u{a0}b\u{2003}c\u{9}d\u{a}e\u{b}f\u{c}g\u{d}h i\u{85}j",
        &[&[12, 13]],
    ),
    (
        "(?i)k+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 5]],
    ),
    (
        "(?i)K",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 1], &[1, 2], &[2, 5]],
    ),
    (
        "(?i)\\x{212a}",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 1], &[1, 2], &[2, 5]],
    ),
    (
        "(?i)s+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[6, 10]],
    ),
    (
        "(?i)\u{17f}",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[6, 7], &[7, 8], &[8, 10]],
    ),
    (
        "(?i)\u{3c3}+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[11, 17]],
    ),
    (
        "(?i)\u{3c2}",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[11, 13], &[13, 15], &[15, 17]],
    ),
    (
        "(?i)[a-z]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 5], &[6, 10]],
    ),
    (
        "(?i)\\w+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 5], &[6, 10]],
    ),
    (
        "(?i)\\W+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[5, 6], &[10, 35]],
    ),
    (
        "(?i)[^k]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[5, 35]],
    ),
    (
        "(?i)[^\\W]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 5], &[6, 10]],
    ),
    (
        "(?i)[\\W]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[5, 6], &[10, 35]],
    ),
    (
        "(?i)[[:upper:]]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 5], &[6, 10]],
    ),
    (
        "(?i)[[:^lower:]]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[5, 6], &[10, 35]],
    ),
    (
        "(?i)\\p{Lu}+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 5], &[6, 10], &[11, 17], &[18, 28], &[29, 35]],
    ),
    (
        "(?i)\\P{Lu}+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[5, 6], &[10, 11], &[17, 18], &[28, 29]],
    ),
    (
        "(?i)\\p{Greek}+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[11, 17], &[29, 35]],
    ),
    (
        "(?i)\u{1c5}",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[18, 20], &[20, 22], &[22, 24]],
    ),
    (
        "(?i)[\u{1c5}]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[18, 24]],
    ),
    (
        "(?i)\\x{3bc}",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[29, 31], &[31, 33], &[33, 35]],
    ),
    (
        "(?i)[k-s]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 5], &[6, 10]],
    ),
    (
        "(?i)[^a-z\\s]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[11, 17], &[18, 28], &[29, 35]],
    ),
    (
        "(?i)[[:word:]]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 5], &[6, 10]],
    ),
    (
        "(?i)\\p{Ascii}+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 11], &[17, 18], &[28, 29]],
    ),
    (
        "(?i)\\P{Ascii}+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[11, 17], &[18, 28], &[29, 35]],
    ),
    (
        "(?i)[\\x{100}-\\x{17f}]+",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[6, 10], &[24, 28]],
    ),
    (
        "(?i:k)K",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[&[0, 2]],
    ),
    (
        "(?i)k(?-i)k",
        "kK\u{212a} sS\u{17f} \u{3c3}\u{3c2}\u{3a3} \u{1c4}\u{1c5}\u{1c6}\u{132}\u{133} \u{b5}\u{39c}\u{3bc}",
        &[],
    ),
    (
        ".+",
        "ab\u{a}cd\u{a}\u{a}ef\u{a}",
        &[&[0, 2], &[3, 5], &[7, 9]],
    ),
    ("(?s).+", "ab\u{a}cd\u{a}\u{a}ef\u{a}", &[&[0, 10]]),
    ("^", "ab\u{a}cd\u{a}\u{a}ef\u{a}", &[&[0, 0]]),
    ("$", "ab\u{a}cd\u{a}\u{a}ef\u{a}", &[&[10, 10]]),
    (
        "(?m)^",
        "ab\u{a}cd\u{a}\u{a}ef\u{a}",
        &[&[0, 0], &[3, 3], &[6, 6], &[7, 7], &[10, 10]],
    ),
    (
        "(?m)$",
        "ab\u{a}cd\u{a}\u{a}ef\u{a}",
        &[&[2, 2], &[5, 5], &[6, 6], &[9, 9], &[10, 10]],
    ),
    ("^ab$", "ab\u{a}cd\u{a}\u{a}ef\u{a}", &[]),
    (
        "(?m)^\\w+$",
        "ab\u{a}cd\u{a}\u{a}ef\u{a}",
        &[&[0, 2], &[3, 5], &[7, 9]],
    ),
    ("\\A\\w", "ab\u{a}cd\u{a}\u{a}ef\u{a}", &[&[0, 1]]),
    ("\\w\\z", "ab\u{a}cd\u{a}\u{a}ef\u{a}", &[]),
    (
        "(?m)^$",
        "ab\u{a}cd\u{a}\u{a}ef\u{a}",
        &[&[6, 6], &[10, 10]],
    ),
    (
        "(?s:.)\\n",
        "ab\u{a}cd\u{a}\u{a}ef\u{a}",
        &[&[1, 3], &[4, 6], &[8, 10]],
    ),
    (
        "[^a]",
        "ab\u{a}cd\u{a}\u{a}ef\u{a}",
        &[
            &[1, 2],
            &[2, 3],
            &[3, 4],
            &[4, 5],
            &[5, 6],
            &[6, 7],
            &[7, 8],
            &[8, 9],
            &[9, 10],
        ],
    ),
    (
        "(?m:$)\\n",
        "ab\u{a}cd\u{a}\u{a}ef\u{a}",
        &[&[2, 3], &[5, 6], &[6, 7], &[9, 10]],
    ),
    ("(?-m:^)", "ab\u{a}cd\u{a}\u{a}ef\u{a}", &[&[0, 0]]),
    ("a+?", "aaa", &[&[0, 1], &[1, 2], &[2, 3]]),
    ("(?U)a+", "aaa", &[&[0, 1], &[1, 2], &[2, 3]]),
    ("(?U)a+?", "aaa", &[&[0, 3]]),
    ("(?U)a{1,3}", "aaa", &[&[0, 1], &[1, 2], &[2, 3]]),
    ("a{1,3}?", "aaa", &[&[0, 1], &[1, 2], &[2, 3]]),
    ("a??b", "ab", &[&[0, 2]]),
    (
        "(?P<first>\\w+) (?<last>\\w+)",
        "john smith, jane doe",
        &[&[0, 10, 0, 4, 5, 10], &[12, 20, 12, 16, 17, 20]],
    ),
    (
        "(?P<n>a)|(?P<n>b)",
        "ab",
        &[&[0, 1, 0, 1, -1, -1], &[1, 2, -1, -1, 1, 2]],
    ),
    ("(?<1a>x)", "x", &[&[0, 1, 0, 1]]),
    ("\\<a\\>", "<a>", &[&[0, 3]]),
    ("\\b{start}", "a{start}", &[&[1, 8]]),
    ("[[a]]", "a] [a]", &[&[0, 2], &[4, 6]]),
    ("[a-z&&b]+", "a&&b", &[&[0, 4]]),
    ("[a~~b]+", "~a", &[&[0, 2]]),
    ("\\Qa.b\\E+", "a.bb a.b", &[&[0, 4], &[5, 8]]),
    ("\\Q(a)*", "(a)*", &[&[0, 4]]),
    ("\\x{10FFFF}", "\u{10ffff}", &[&[0, 4]]),
    ("\\x{D800}|a", "a", &[&[0, 1]]),
    ("[\\x{D7FF}-\\x{E000}]+", "\u{d7ff}\u{e000}", &[&[0, 6]]),
    ("\\0", "\u{0}", &[&[0, 1]]),
    ("\\01", "\u{1}", &[&[0, 1]]),
    ("\\101", "A", &[&[0, 1]]),
    ("\\08", "\u{0}8", &[&[0, 2]]),
    ("x{,5}", "x{,5}", &[&[0, 5]]),
    ("x{a}", "x{a}", &[&[0, 4]]),
    ("x{", "x{", &[&[0, 2]]),
    ("a{,}", "a{,}", &[&[0, 4]]),
    ("(?:)", "ab", &[&[0, 0], &[1, 1], &[2, 2]]),
    ("()", "ab", &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[2, 2, 2, 2]]),
    ("(|a)", "a", &[&[0, 0, 0, 0], &[1, 1, 1, 1]]),
    ("(a|)", "a", &[&[0, 1, 0, 1]]),
    ("a|ab", "ab", &[&[0, 1]]),
    ("ab|a", "ab", &[&[0, 2]]),
    ("(a*)*", "b", &[&[0, 0, 0, 0], &[1, 1, 1, 1]]),
    ("(a*)+", "b", &[&[0, 0, 0, 0], &[1, 1, 1, 1]]),
    ("(a|b)*", "ab", &[&[0, 2, 1, 2]]),
    ("(a*)+$", "aab", &[&[3, 3, 3, 3]]),
    ("x*", "xax", &[&[0, 1], &[2, 3]]),
    ("a*", "baaab", &[&[0, 0], &[1, 4], &[5, 5]]),
    ("(a){0}(b)", "ab", &[&[1, 2, -1, -1, 1, 2]]),
    ("(a){0,0}", "a", &[&[0, 0, -1, -1], &[1, 1, -1, -1]]),
    ("(?:(a)|b){0}(c)", "c", &[&[0, 1, -1, -1, 0, 1]]),
    ("(a)?", "b", &[&[0, 0, -1, -1], &[1, 1, -1, -1]]),
    ("(a)|b", "b", &[&[0, 1, -1, -1]]),
    ("(a)(?:(b)|c)", "ac", &[&[0, 2, 0, 1, -1, -1]]),
    ("(?:(a)|(b))+", "ab", &[&[0, 2, 0, 1, 1, 2]]),
    ("(a+)(b+)?", "aab", &[&[0, 3, 0, 2, 2, 3]]),
    (
        "\\a\\f\\t\\n\\r\\v",
        "\u{7}\u{c}\u{9}\u{a}\u{d}\u{b}",
        &[&[0, 6]],
    ),
    ("\\_\\-\\:\\#\\ ", "_-:# ", &[&[0, 5]]),
    ("[]a]+", "]a", &[&[0, 2]]),
    ("[^]a]+", "]ab", &[&[2, 3]]),
    ("[a-]+", "-a", &[&[0, 2]]),
    ("[-a]+", "a-", &[&[0, 2]]),
    ("[\\d-z]+", "1-z", &[&[0, 3]]),
    ("(?i-i)a", "aA", &[&[0, 1]]),
    ("(?)a", "a", &[&[0, 1]]),
    ("ab|ac|ad", "ad ac ab", &[&[0, 2], &[3, 5], &[6, 8]]),
    (
        "abc|abd|aef|bcx|bcy",
        "bcy aef abd",
        &[&[0, 3], &[4, 7], &[8, 11]],
    ),
    ("[ab]c|[ab]d", "bd ac", &[&[0, 2], &[3, 5]]),
    ("a|b|c|d", "dcba", &[&[0, 1], &[1, 2], &[2, 3], &[3, 4]]),
    ("a(?:b|c)|a(?:d|e)", "ae", &[&[0, 2]]),
    ("(?:a|a)b|ab", "ab", &[&[0, 2]]),
    ("(?i)ab|ac", "AB aC", &[&[0, 2], &[3, 5]]),
    ("(?i:a)b|ac", "Ab ac aC", &[&[0, 2], &[3, 5]]),
    ("x|(?:)", "x", &[&[0, 1]]),
    ("(?:|x)+", "xx", &[&[0, 0], &[1, 1], &[2, 2]]),
    ("(?:a{2}){3}", "aaaaaa", &[&[0, 6]]),
    ("^*a", "a", &[&[0, 1]]),
    ("$+", "a", &[&[1, 1]]),
    ("\\b*a", "a", &[&[0, 1]]),
    ("(?:^|a)+", "aa", &[&[0, 0], &[1, 2]]),
    ("(?:$|a)+", "aa", &[&[0, 2]]),
    ("\\bx\\B", "xa x", &[&[0, 1]]),
    (
        "(\\w+)@(\\w+)\\.com",
        "me@x.com you@\u{e4}.com",
        &[&[0, 8, 0, 2, 3, 4]],
    ),
    ("(?s)a.c", "a\u{a}c", &[&[0, 3]]),
    ("a.c", "a\u{a}c abc", &[&[4, 7]]),
    ("[^\\n]+", "ab\u{a}cd", &[&[0, 2], &[3, 5]]),
    ("\\pL\\p{Greek}\\pN", "a\u{3b2}1", &[&[0, 4]]),
    ("\\p{Assigned}+", "a\u{378}b", &[&[0, 1], &[3, 4]]),
    ("\\p{Any}+", "a\u{a}b", &[&[0, 3]]),
    ("\\P{Any}", "a", &[]),
    ("\\p{Lc}+", "aB\u{1c5}", &[&[0, 4]]),
    ("\\p{Cs}", "a", &[]),
];
