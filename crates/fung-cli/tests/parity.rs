// crates/fung-cli/tests/parity.rs
use std::{
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::Command,
};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Builds the Go implementation. `None` only when `go` cannot be run at all
/// and parity is not required; a failing `go build` always panics.
fn go_fung() -> Option<PathBuf> {
    let out = std::env::temp_dir().join(format!("fung-go-parity-{}", std::process::id()));
    let built = Command::new("go")
        .args(["build", "-o"])
        .arg(&out)
        .arg("./cmd/fung")
        .current_dir(repo())
        .output();
    match built {
        Err(e) => {
            assert!(
                std::env::var("FUNG_REQUIRE_PARITY").as_deref() != Ok("1"),
                "FUNG_REQUIRE_PARITY=1 but `go` cannot be run: {e}"
            );
            None
        }
        Ok(o) if o.status.success() => Some(out),
        Ok(o) => panic!("go build failed:\n{}", String::from_utf8_lossy(&o.stderr)),
    }
}

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

/// Standard output, standard error (bytes, compared exactly) and the exit
/// status, compared as its debug text so two different signal deaths differ.
fn run(bin: &Path, args: &[OsString]) -> (Vec<u8>, Vec<u8>, String) {
    let o = Command::new(bin)
        .args(args)
        .current_dir(repo())
        .output()
        .unwrap();
    (o.stdout, o.stderr, format!("{:?}", o.status))
}

fn first_diff(a: &[u8], b: &[u8]) -> String {
    let (a, b) = (String::from_utf8_lossy(a), String::from_utf8_lossy(b));
    if a == b {
        return "same text, different bytes (invalid UTF-8)".into();
    }
    for (i, (x, y)) in a.lines().zip(b.lines()).enumerate() {
        if x != y {
            return format!("line {}:\n  go:   {x}\n  rust: {y}", i + 1);
        }
    }
    format!(
        "lengths differ: go {} lines, rust {} lines",
        a.lines().count(),
        b.lines().count()
    )
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
    let Some(go) = go_fung() else {
        eprintln!("skipping parity: `go` is not available");
        return;
    };
    let rust = PathBuf::from(env!("CARGO_BIN_EXE_fung"));
    let mut failures = Vec::new();
    let mut cases: Vec<Vec<OsString>> = Vec::new();
    for f in corpus() {
        for stage in ["ast", "core", "bytecode"] {
            cases.push(vec!["dump".into(), stage.into(), f.clone().into()]);
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
        let (g, r) = (run(&go, a), run(&rust, a));
        if g != r {
            let what = if g.0 != r.0 {
                first_diff(&g.0, &r.0)
            } else if g.1 != r.1 {
                first_diff(&g.1, &r.1)
            } else {
                format!("exit {} vs {}", g.2, r.2)
            };
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
