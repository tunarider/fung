// crates/fung-cli/tests/parity.rs
mod common;

use std::{ffi::OsString, os::unix::ffi::OsStringExt};

use common::{RUST_ONLY_EXAMPLES, diff, go_fung, repo, run, rust_fung, uses_rust_only};

fn corpus() -> Vec<String> {
    let mut files = Vec::new();
    for dir in ["tests/lang", "examples", "extras", "bench"] {
        let mut stack = vec![repo().join(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "fung") {
                    files.push(
                        p.strip_prefix(repo())
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
    }
    files.sort();
    files
}

/// Whether a corpus file's names resolve the same in both
/// implementations. Since stage 3b they differ for a `# only:` golden
/// file (atoms are Go only, agents Rust only) and since stage 6a for the
/// Rust-only examples (`common::RUST_ONLY_EXAMPLES`): their syntax is
/// shared, so only `dump ast` is compared for them.
fn resolves_alike(path: &str) -> bool {
    if RUST_ONLY_EXAMPLES.contains(&path) {
        return false;
    }
    let src = std::fs::read_to_string(repo().join(path)).unwrap();
    !src.lines().any(|l| l.trim().starts_with("# only:"))
}

#[test]
fn the_rust_only_examples_are_listed() {
    let examples: Vec<String> = corpus()
        .into_iter()
        .filter(|f| f.starts_with("examples/"))
        .collect();
    for path in &examples {
        let src = std::fs::read_to_string(repo().join(path)).unwrap();
        assert_eq!(
            uses_rust_only(&src),
            RUST_ONLY_EXAMPLES.contains(&path.as_str()),
            "{path}: RUST_ONLY_EXAMPLES is out of date"
        );
    }
    for path in RUST_ONLY_EXAMPLES {
        assert!(examples.iter().any(|f| f == path), "{path} is gone");
    }
    assert!(resolves_alike("tests/lang/arith.fung"));
    assert!(!resolves_alike("tests/lang/agents.fung"));
    assert!(!resolves_alike("tests/lang/atoms.fung"));
}

/// Files the corpus does not have: BOM, CRLF, tabs, invalid UTF-8.
fn odd_files() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("bom_crlf.fung", b"\xef\xbb\xbfx = 1\r\nprint(x)\r\n"),
        ("bom_crlf_err.fung", b"\xef\xbb\xbfx = 1\r\ny = )\r\n"),
        ("tab_err.fung", b"x = 1\n\tprint(nope)\n"),
        ("bad_utf8_string.fung", b"x = \"a\xffb\"\nprint(x)\n"),
        ("bad_utf8_code.fung", b"print(1)\n\xff\n"),
        ("bad_utf8_run.fung", b"x = \"\xeb\x8b\" + nope\n"),
        ("bad_utf8_ok.fung", b"x = \"\xeb\x8b\"\nprint(x)\n"),
        ("badmod.fung", b"x = \"\xff\"\n"),
        (
            "uses_badmod.fung",
            b"import \"./badmod\"\nprint(badmod.x)\n",
        ),
        // A module stem Go's `unicode.IsLetter` rejects (So, Nl).
        ("mⓐ.fung", b"v = 1\n"),
        (
            "uses_odd_stem.fung",
            "import \"./mⓐ\"\nprint(1)\n".as_bytes(),
        ),
        ("mⅻ.fung", b"v = 1\n"),
        (
            "uses_numeral_stem.fung",
            "import \"./mⅻ\"\nprint(1)\n".as_bytes(),
        ),
        // Format (Cf) characters print as escapes, U+13440 does not.
        (
            "format_chars.fung",
            "x = \"\u{890}\u{110bd}\u{110cd}\u{1343f}\u{13440}\u{1bca3}\u{1d17a}\"\nprint(x)\n"
                .as_bytes(),
        ),
    ]
}

/// Deep shapes whose recursion the parser's nesting limit does not count.
fn deep_shapes() -> Vec<String> {
    let n = 20_000;
    let nested_type = format!("{}int{}", "list[".repeat(n), "]".repeat(n));
    let mut else_if = String::from("x = 1\ny = if x == 0:\n  0\n");
    for i in 1..10_000 {
        else_if.push_str(&format!("else if x == {i}:\n  {i}\n"));
    }
    else_if.push_str("else:\n  -1\n;;\nprint(y)\n");
    let fns = 10_000;
    let mut nested_fn = String::new();
    for i in 0..fns {
        nested_fn.push_str(&format!("fn f{i}():\n"));
    }
    nested_fn.push_str("1\n");
    nested_fn.push_str(&";;\n".repeat(fns));
    vec![
        format!("fn f(x: {nested_type}) = x"),
        format!("type t = {nested_type}"),
        else_if,
        nested_fn,
        format!("x = {{a: 1}}\ny = x{}", ".a".repeat(n)),
        format!("fn f() = f\ny = f{}", "()".repeat(n)),
        format!("xs = [1]\ny = xs{}", "[0]".repeat(n)),
        format!("y = 1{}", " + 1".repeat(50_000)),
        format!("fn f(x) = x\ny = 1{}", " |> f".repeat(50_000)),
        format!("y = true{}", " and true".repeat(50_000)),
        format!("y = {}1{}", "[".repeat(n), "]".repeat(n)),
        format!("y = {}1", "-".repeat(n)),
    ]
}

/// Long flat chains: the parser loops, but later passes recurse once per
/// link, so they need a stack as deep as Go's (see `STACK` in main.rs).
fn long_chains() -> Vec<String> {
    let n = 200_000;
    vec![
        format!("y = 1{}", " + 1".repeat(n)),
        format!("x = {{b: 1}}\ny = x{}", ".b".repeat(n)),
        format!("y = nope{}", ".b".repeat(n)),
        format!("fn f() = f\ny = f{}", "()".repeat(n)),
        format!("y = nope{}", "()".repeat(n)),
        format!("fn f(x) = x\ny = 1{}", " |> f".repeat(n)),
        // One error at the end: n undefined names would make Go's output
        // (an excerpt of the whole line per error) quadratic.
        format!("fn f(x) = x\ny = 1{} |> nope", " |> f".repeat(n)),
    ]
}

#[test]
fn dumps_match_the_go_implementation() {
    let Some(go) = go_fung("dump") else {
        eprintln!("skipping parity: `go` is not available");
        return;
    };
    let rust = rust_fung();
    let mut failures = Vec::new();
    let mut cases: Vec<Vec<OsString>> = Vec::new();
    for f in corpus() {
        let stages: &[&str] = if resolves_alike(&f) {
            &["ast", "core", "bytecode"]
        } else {
            &["ast"]
        };
        for stage in stages {
            cases.push(vec!["dump".into(), (*stage).into(), f.clone().into()]);
        }
    }
    for extra in [
        vec!["dump"],
        vec!["dump", "nope"],
        vec!["dump", "core"],
        vec!["dump", "core", "-e"],
        vec!["dump", "core", "no/such/file.fung"],
        vec!["dump", "bytecode", "-e", "x = )"],
    ] {
        cases.push(extra.into_iter().map(OsString::from).collect());
    }
    let deep = format!("x = {}1{}", "(".repeat(20_000), ")".repeat(20_000));
    cases.push(vec!["dump".into(), "ast".into(), "-e".into(), deep.into()]);
    let dir = std::env::temp_dir().join(format!("fung-parity-files-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Arguments that are not UTF-8: Go decodes each bad byte to U+FFFD.
    cases.push(vec![
        "dump".into(),
        "core".into(),
        "-e".into(),
        OsString::from_vec(b"print(\"\xff\")".to_vec()),
    ]);
    let mut bad_path = dir.clone().into_os_string().into_vec();
    bad_path.extend_from_slice(b"/\xff.fung");
    cases.push(vec![
        "dump".into(),
        "core".into(),
        OsString::from_vec(bad_path),
    ]);
    for (name, bytes) in odd_files() {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        for stage in ["ast", "core", "bytecode"] {
            cases.push(vec![
                "dump".into(),
                stage.into(),
                path.clone().into_os_string(),
            ]);
        }
    }
    for extra in [
        vec!["dump", "core", "."],
        vec!["dump", "core", "a", "b"],
        vec!["dump", "ast", "-e", "x = 1", "extra"],
        vec!["dump", "core", "-e", "print(없는)"],
        vec!["dump", "core", "-e", "\tprint(nope)"],
        // Go's Unicode classes (spec/unicode.txt): identifiers stop at
        // characters `unicode.IsLetter`/`IsDigit` reject, and quoting
        // escapes what `unicode.IsPrint` rejects.
        vec!["dump", "ast", "-e", "x² = 1"],
        vec!["dump", "core", "-e", "aⅫ = 1"],
        vec!["dump", "ast", "-e", "Ⓐ = 1"],
        vec!["dump", "ast", "-e", "हिंदी = 1"],
        vec!["dump", "core", "-e", "ß1 = 2\nprint(ß1)"],
        vec!["dump", "ast", "-e", "Σ1 = 1\nǅx = 2"],
        vec!["dump", "core", "-e", "x٣ = 1\nprint(x٣)"],
        vec!["dump", "ast", "-e", "x = ٣"],
        vec!["dump", "ast", "-e", "x = \u{378}"],
        vec!["dump", "ast", "-e", "x = \"a\u{378}b\u{E000}\""],
        vec!["dump", "bytecode", "-e", "x = \"a\u{378}b\""],
        vec!["dump", "core", "-e", "fn f(x: Ⓐ) = x"],
    ] {
        cases.push(extra.into_iter().map(OsString::from).collect());
    }
    // Linux limits one argument to 128 KiB, so long sources go in files.
    for (i, code) in deep_shapes().into_iter().chain(long_chains()).enumerate() {
        let src: Vec<OsString> = if code.len() < 100_000 {
            vec!["-e".into(), code.into()]
        } else {
            let path = dir.join(format!("deep_{i}.fung"));
            std::fs::write(&path, code).unwrap();
            vec![path.into_os_string()]
        };
        for stage in ["ast", "core", "bytecode"] {
            let mut args: Vec<OsString> = vec!["dump".into(), stage.into()];
            args.extend(src.iter().cloned());
            cases.push(args);
        }
    }
    for a in &cases {
        if let Some(what) = diff(&run(&go, a), &run(&rust, a)) {
            failures.push(format!(
                "{}: {what}",
                a.iter()
                    .take(3)
                    .map(|x| x.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
    }
    let _ = std::fs::remove_file(&go);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}
