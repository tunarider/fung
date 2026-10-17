//! What the parity tests share: building the Go implementation, running a
//! binary, and comparing two runs byte for byte.

// Each test binary uses its own subset of these helpers.
#![allow(dead_code)]

use std::{
    ffi::OsString,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Builds the Go implementation. `None` only when `go` cannot be run at all
/// and parity is not required (`FUNG_REQUIRE_PARITY=1` makes that a
/// failure); a failing `go build` always panics. `tag` keeps the binaries
/// of concurrently running test programs apart.
pub fn go_fung(tag: &str) -> Option<PathBuf> {
    go_fung_with(tag, &[])
}

/// [`go_fung`] with extra `go build` flags (e.g. `-ldflags=...`).
pub fn go_fung_with(tag: &str, flags: &[&str]) -> Option<PathBuf> {
    let out = std::env::temp_dir().join(format!("fung-go-{tag}-{}", std::process::id()));
    let built = Command::new("go")
        .arg("build")
        .args(flags)
        .arg("-o")
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

/// The Rust `fung` binary under test.
pub fn rust_fung() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fung"))
}

/// Standard output, standard error (bytes, compared exactly) and the exit
/// status, compared as its debug text so two different signal deaths differ.
pub type Run = (Vec<u8>, Vec<u8>, String);

/// How long one run may take before it is killed.
pub const RUN_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

/// Runs `bin` from the repository root with no standard input.
pub fn run(bin: &Path, args: &[OsString]) -> Run {
    run_with_stdin(bin, args, None)
}

/// Runs `bin` from the repository root; `stdin` is written to its
/// standard input and closed (`None`: standard input is closed at once).
pub fn run_with_stdin(bin: &Path, args: &[OsString], stdin: Option<&[u8]>) -> Run {
    let (out, err, status) = run_status(bin, args, stdin);
    (out, err, format!("{status:?}"))
}

/// Like [`run_with_stdin`], with the exit status itself.
pub fn run_status(
    bin: &Path,
    args: &[OsString],
    stdin: Option<&[u8]>,
) -> (Vec<u8>, Vec<u8>, std::process::ExitStatus) {
    let mut child = Command::new(bin)
        .args(args)
        .current_dir(repo())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let writer = stdin.map(|data| {
        let mut pipe = child.stdin.take().unwrap();
        let data = data.to_vec();
        // A thread, so a child that writes much before reading cannot
        // deadlock against us. A child that exits early closes the pipe.
        std::thread::spawn(move || {
            let _ = pipe.write_all(&data);
        })
    });
    // A run that hangs is killed (and then differs) rather than hanging
    // the test.
    let pid = child.id();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let watch = {
        let done = done.clone();
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + RUN_LIMIT;
            while std::time::Instant::now() < until {
                if done.load(std::sync::atomic::Ordering::Acquire) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            let _ = Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status();
        })
    };
    let o = child.wait_with_output().unwrap();
    done.store(true, std::sync::atomic::Ordering::Release);
    let _ = watch.join();
    if let Some(w) = writer {
        w.join().unwrap();
    }
    (o.stdout, o.stderr, o.status)
}

/// Where two runs first differ, or `None` when they are the same.
pub fn diff(go: &Run, rust: &Run) -> Option<String> {
    if go == rust {
        return None;
    }
    Some(if go.0 != rust.0 {
        // A run that failed on one side prints less: say how it ended.
        let mut s = format!("stdout {}", first_diff(&go.0, &rust.0));
        if go.1 != rust.1 {
            s += &format!("\n  stderr {}", first_diff(&go.1, &rust.1));
        }
        if go.2 != rust.2 {
            s += &format!("\n  exit {} vs {}", go.2, rust.2);
        }
        s
    } else if go.1 != rust.1 {
        format!("stderr {}", first_diff(&go.1, &rust.1))
    } else {
        format!("exit {} vs {}", go.2, rust.2)
    })
}

/// The first differing line of two outputs.
pub fn first_diff(a: &[u8], b: &[u8]) -> String {
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

/// The examples only the Rust implementation runs (stage 6a): they use
/// agents (`api.fung`, `serve.fung`) or the module `conc`
/// (`checkurls.fung`), which Go does not have. The parity tests compare
/// only their syntax (`dump ast`, `fmt`) with Go and check, test and run
/// them with Rust alone; `parity.rs` checks that the list is complete.
pub const RUST_ONLY_EXAMPLES: &[&str] = &[
    "examples/api.fung",
    "examples/checkurls.fung",
    "examples/serve.fung",
];

/// Whether an example's source uses what only Rust has: an agent or the
/// module `conc`.
pub fn uses_rust_only(src: &str) -> bool {
    src.contains("agent(") || src.lines().any(|l| l.trim() == "import conc")
}
