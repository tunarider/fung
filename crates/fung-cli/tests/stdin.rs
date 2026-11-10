//! Standard input as the command reads it: only on demand, a buffer of
//! 64 KiB at a time (the pipe buffer size on Linux), so what the program never asked
//! for is left for the next reader of the same input.

mod common;

use std::process::{Command, Stdio};

use common::fung_bin;

/// A shell runs fung on a file and then `cat` on the same input: fung's
/// one `read_line` reads one 64 KiB buffer, and `cat` gets the rest.
#[test]
fn one_read_line_takes_one_buffer_of_a_file() {
    let dir = std::env::temp_dir().join(format!("fung-stdin-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("input.txt");
    let content: String = (0..20000).map(|i| format!("line {i}\n")).collect();
    std::fs::write(&path, &content).unwrap();
    let o = Command::new("/bin/sh")
        .arg("-c")
        .arg("\"$0\" -e 'print(read_line())'; cat")
        .arg(fung_bin())
        .stdin(Stdio::from(std::fs::File::open(&path).unwrap()))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let out = String::from_utf8(o.stdout).unwrap();
    assert_eq!(out, format!("Ok(\"line 0\")\n{}", &content[65536..]));
}

/// Feeds `src`'s standard input with `a`s and no newline until fung
/// closes it (at most 300 MB), within 60 s: its stdout and stderr.
fn endless_input(src: &str) -> (String, String) {
    use std::io::Write;
    let mut child = common::Reaped::new(
        common::spawn(
            Command::new(fung_bin())
                .args(["-e", src])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )
        .unwrap(),
    );
    let mut input = child.stdin.take().unwrap();
    let feed = std::thread::spawn(move || {
        let chunk = vec![b'a'; 1 << 16];
        for _ in 0..4600 {
            if input.write_all(&chunk).is_err() {
                return;
            }
        }
    });
    let (o, killed) = child
        .output_within(std::time::Duration::from_secs(60))
        .unwrap();
    feed.join().unwrap();
    assert!(!killed, "fung did not stop reading");
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// LB29 (D22): a line read from outside stops being read once it passes
/// the string limit, and the read gives its `Err`: endless input with no
/// newline is not held whole. The bytes of the line read so far are
/// dropped; the next read goes on from there.
#[test]
fn a_line_past_the_string_limit_is_an_err_while_it_is_read() {
    const ERR: &str = "Err({kind: 'io, message: \"cannot read standard input: line is longer than the string limit (100000000 characters)\"})";
    let (out, err) = endless_input("print(read_line())\nprint(read_lines())\nexit(0)");
    assert_eq!(
        (out.as_str(), err.as_str()),
        (format!("{ERR}\n{ERR}\n").as_str(), "")
    );
}

/// The same for a FIFO, as a handle's lines and as `fs.read`: the writer
/// writes a terabyte of zeros (endless here; `head` writes in large
/// pieces, which macOS's FIFOs pass on fast), and a cancel ends it.
#[test]
fn a_fifo_past_the_string_limit_is_an_err_while_it_is_read() {
    let dir = std::env::temp_dir().join(format!("fung-stdin-fifo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fifo = dir.join("f");
    let fifo_s = fifo.to_str().unwrap().to_owned();
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    // fung starts `cat` into the FIFO in a task, then reads it.
    let src = format!(
        r#"import fs
p = "{fifo_s}"
fn feed() = sh("head -c 1000000000000 /dev/zero > '{fifo_s}'")
scope(fn(s):
  t = spawn(s, feed)
  h = fs.open(p)!
  print(read_line(h))
  cancel(t)
;;)
scope(fn(s):
  t = spawn(s, feed)
  print(fs.read(p))
  cancel(t)
;;)
exit(0)"#
    );
    let (out, err) = endless_input(&src);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        (out.as_str(), err.as_str()),
        (
            format!(
                "Err({{kind: 'io, message: \"cannot read `{fifo_s}`: line is longer than the string limit (100000000 characters)\"}})\n\
                 Err({{kind: 'io, message: \"cannot read `{fifo_s}`: file is longer than the string limit (100000000 characters)\"}})\n"
            )
            .as_str(),
            ""
        )
    );
}
