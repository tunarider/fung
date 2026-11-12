//! Bytes that are not UTF-8 go through fung as they are (L4): what
//! comes in by the arguments, the environment, standard input, a file or
//! a command goes out the same, and a source file that is not UTF-8 is
//! refused rather than read with U+FFFD in it. Golden files cannot hold
//! such bytes (they are UTF-8 text), so these runs live here.

mod common;

use std::ffi::OsStr;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};

use common::{fung_bin, spawn};

/// A directory of its own for one test, removed at the end.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("fung-bytes-{tag}-{}", std::process::id()));
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

/// Runs `fung -e code` with `arg`, `X` set to `env` and `stdin`.
fn run(code: &str, arg: &[u8], env: &[u8], stdin: &[u8]) -> (Vec<u8>, Vec<u8>, i32) {
    let mut child = spawn(
        Command::new(fung_bin())
            .arg("-e")
            .arg(code)
            .arg(OsStr::from_bytes(arg))
            .env("X", OsStr::from_bytes(env))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let o = child.wait_with_output().unwrap();
    (o.stdout, o.stderr, o.status.code().unwrap_or(-1))
}

#[test]
fn arguments_environment_and_input_keep_their_bytes() {
    let o = run(
        "print([args[0], env(\"X\"), read_line()!])\nprint(args[0], env(\"X\"))",
        b"q\xff",
        b"v\xfe",
        b"x\xffy\n",
    );
    assert_eq!(
        o,
        (
            b"[\"q\\xff\", \"v\\xfe\", \"x\\xffy\"]\nq\xff v\xfe\n".to_vec(),
            Vec::new(),
            0
        )
    );
}

/// The reproduction of the problem (L4): a file read and written back
/// was 12 bytes instead of 7, every bad sequence a U+FFFD.
#[test]
fn a_file_read_and_written_back_is_the_same() {
    let d = Scratch::new("file");
    let bad = d.0.join("bad.txt");
    let out = d.0.join("out.txt");
    std::fs::write(&bad, b"a\xffb\xe2\x82c\n").unwrap();
    let code = format!(
        "import fs\ns = fs.read(\"{}\")!\nprint(len(s), byte_len(s))\nfs.write(\"{}\", s)!",
        bad.display(),
        out.display()
    );
    let o = run(&code, b"", b"", b"");
    assert_eq!(o, (b"7 7\n".to_vec(), Vec::new(), 0));
    assert_eq!(std::fs::read(&out).unwrap(), b"a\xffb\xe2\x82c\n");
}

#[test]
fn a_command_output_and_its_environment_keep_their_bytes() {
    let o = run(
        "r = exec([\"/bin/sh\", \"-c\", \"printf 'o\\\\377'; printf \\\"\\$Y\\\" >&2\"], env: {Y: \"e\\xfe\"})!\nprint([r.out, r.err])",
        b"",
        b"",
        b"",
    );
    assert_eq!(o, (b"[\"o\\xff\", \"e\\xfe\"]\n".to_vec(), Vec::new(), 0));
}

#[test]
fn a_source_file_that_is_not_utf8_is_refused() {
    let d = Scratch::new("source");
    let p = d.0.join("x.fung");
    std::fs::write(&p, b"print(1)\nx = \"\xff\"\n").unwrap();
    let o = common::output(Command::new(fung_bin()).arg(&p)).unwrap();
    let err = String::from_utf8(o.stderr).unwrap();
    assert_eq!(o.status.code(), Some(2), "{err}");
    assert!(
        err.ends_with("x.fung`: not valid UTF-8 at line 2, column 6\n"),
        "{err}"
    );
    assert!(o.stdout.is_empty());
}

/// The code of `-e` is source text too.
#[test]
fn code_given_with_e_that_is_not_utf8_is_refused() {
    let o = common::output(
        Command::new(fung_bin())
            .arg("-e")
            .arg(OsStr::from_bytes(b"print(\"\xff\")")),
    )
    .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        o.stderr,
        b"error: cannot read `-e`: not valid UTF-8 at line 1, column 8\n"
    );
}

/// File names are bytes too. macOS (APFS) refuses to make a name that is
/// not UTF-8 (`Illegal byte sequence`), so this runs on Linux.
#[cfg(target_os = "linux")]
#[test]
fn file_names_keep_their_bytes() {
    let d = Scratch::new("names");
    std::fs::write(d.0.join(OsStr::from_bytes(b"n\xff")), b"x").unwrap();
    let code = format!(
        "import fs\nnames = fs.list(\"{0}\")!\nprint(names, fs.read(\"{0}/\" + names[0])!)",
        d.0.display()
    );
    let o = run(&code, b"", b"", b"");
    assert_eq!(o, (b"[\"n\\xff\"] x\n".to_vec(), Vec::new(), 0));
}
