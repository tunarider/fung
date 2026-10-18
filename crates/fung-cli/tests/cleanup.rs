//! Cleanup through the command (concurrency v2, L3): what `with` leaves
//! behind, and what `fung test` makes of a release cut short.

mod common;

use std::process::{Command, Stdio};
use std::time::Duration;

use common::{Reaped, fung_bin};

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("fung-cleanup-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `fung args…` with `TMPDIR` set to `tmp`: (stdout, stderr, code).
fn fung(args: &[&str], tmp: &std::path::Path) -> (String, String, Option<i32>) {
    let child = Reaped::new(
        Command::new(fung_bin())
            .args(args)
            .env("TMPDIR", tmp)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let (o, killed) = child.output_within(Duration::from_secs(20)).unwrap();
    assert!(!killed, "fung did not end");
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        o.status.code(),
    )
}

/// The deadline passes unnoticed during native work that makes no call
/// (`repeat`), so the first to see it is the wait that makes the
/// directory, which then cuts the call right after it: the directory is
/// acquired inside the `with`, so it is removed anyway.
#[test]
fn with_temp_dir_leaves_nothing_when_cut_right_after_acquiring() {
    let tmp = Scratch::new("tmpdir");
    let (out, err, code) = fung(
        &[
            "-e",
            "import fs\nprint(timeout(1ms, fn(): s = repeat(\"x\", 100000000); fs.with_temp_dir(d => 1) ;;))",
        ],
        &tmp.0,
    );
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("Err({kind: 'timeout})\n", "", Some(0))
    );
    let left: Vec<_> = std::fs::read_dir(&tmp.0).unwrap().collect();
    assert!(left.is_empty(), "left behind: {left:?}");
}

/// A release cut short by a deadlock fails its test block.
#[test]
fn a_deadlock_in_a_release_fails_the_test_block() {
    let tmp = Scratch::new("test-block");
    let file = tmp.0.join("d_test.fung");
    std::fs::write(
        &file,
        "test \"deadlock in release\":\n  (_tx, rx) = channel()\n  with(() => nil, _ => 1, _ => recv(rx))\n;;\n",
    )
    .unwrap();
    let (out, err, code) = fung(&["test", file.to_str().unwrap()], &tmp.0);
    assert!(
        err.contains("E0702") || out.contains("E0702"),
        "{out}\n{err}"
    );
    assert!(out.contains("FAIL"), "{out}");
    assert_eq!(code, Some(1), "{out}\n{err}");
}
