// crates/fung-cli/tests/parity.rs
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn go_fung() -> Option<PathBuf> {
    let out = std::env::temp_dir().join(format!("fung-go-parity-{}", std::process::id()));
    let ok = Command::new("go")
        .args(["build", "-o"])
        .arg(&out)
        .arg("./cmd/fung")
        .current_dir(repo())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then_some(out)
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

fn run(bin: &Path, args: &[&str]) -> (String, String, i32) {
    let o = Command::new(bin)
        .args(args)
        .current_dir(repo())
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&o.stdout).into(),
        String::from_utf8_lossy(&o.stderr).into(),
        o.status.code().unwrap_or(-1),
    )
}

fn first_diff(a: &str, b: &str) -> String {
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

#[test]
fn dumps_match_the_go_implementation() {
    let Some(go) = go_fung() else {
        eprintln!("skipping parity: `go` is not available");
        return;
    };
    let rust = PathBuf::from(env!("CARGO_BIN_EXE_fung"));
    let mut failures = Vec::new();
    let mut cases: Vec<Vec<String>> = Vec::new();
    for f in corpus() {
        for stage in ["ast", "core", "bytecode"] {
            cases.push(vec!["dump".into(), stage.into(), f.clone()]);
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
        cases.push(extra.into_iter().map(String::from).collect());
    }
    let deep = format!("x = {}1{}", "(".repeat(20_000), ")".repeat(20_000));
    cases.push(vec!["dump".into(), "ast".into(), "-e".into(), deep]);
    let dir = std::env::temp_dir().join(format!("fung-parity-files-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in odd_files() {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        for stage in ["ast", "core", "bytecode"] {
            cases.push(vec![
                "dump".into(),
                stage.into(),
                path.to_string_lossy().into_owned(),
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
        cases.push(extra.into_iter().map(String::from).collect());
    }
    for code in deep_shapes() {
        for stage in ["ast", "core", "bytecode"] {
            cases.push(vec!["dump".into(), stage.into(), "-e".into(), code.clone()]);
        }
    }
    for args in &cases {
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        let (g, r) = (run(&go, &a), run(&rust, &a));
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
                a.iter().take(3).cloned().collect::<Vec<_>>().join(" ")
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
