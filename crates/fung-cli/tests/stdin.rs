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
