//! `compile_go` against Go. Every expected value is Go's own result
//! (`regexp.Compile(pat)`'s `err.Error()` or `SubexpNames()`, and
//! `FindAllStringSubmatchIndex(s, -1)`), written to `regex_go_fixtures.rs`
//! by `go test ./tests -run TestRegexFixtures -update`
//! (`tests/regex_fixtures_test.go`, which also holds the hand-picked
//! cases, the fuzz seed and alphabet); nothing here was written by hand.

use super::*;

#[rustfmt::skip]
#[path = "regex_go_fixtures.rs"]
mod fixtures;

use fixtures::{COMPILE, MATCHES};

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
    check_compile(fixtures::FUZZ_COMPILE);
}

#[test]
fn matches_follow_go() {
    check_matches(MATCHES);
}

#[test]
fn random_patterns_match_like_go() {
    check_matches(fixtures::FUZZ_MATCH);
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
/// every table pattern, and each hand-picked pattern with one
/// metacharacter inserted at every position. All go through the Go parser
/// and the translation (which in debug builds also checks that the
/// printed pattern reads back as the same tree); every 7th also through
/// the Rust engine (slow in debug builds, so not the variants of
/// `\pL{1000}`).
#[test]
fn garbled_patterns_do_not_panic() {
    let mut n = 0;
    let mut try_one = |p: &str| {
        let _ = translate(p);
        n += 1;
        if n % 7 == 0 && !p.contains("1000") {
            let _ = compile_go(p);
        }
    };
    let hand = COMPILE.iter().map(|p| (p.0, true));
    let fuzz = fixtures::FUZZ_COMPILE.iter().map(|p| (p.0, false));
    for (pat, insert) in hand.chain(fuzz) {
        let cuts: Vec<usize> = pat
            .char_indices()
            .map(|(i, _)| i)
            .chain([pat.len()])
            .collect();
        for &i in &cuts {
            try_one(&pat[..i]);
            try_one(&pat[i..]);
            if !insert {
                continue;
            }
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
