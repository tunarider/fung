//! The open file limit (concurrency v2 R4a): `fung` raises its soft
//! limit at start, and the commands it runs get the original one.

mod common;

use std::process::{Command, Stdio};
use std::time::Duration;

use common::{Reaped, fung_bin};

#[test]
fn fung_raises_its_soft_limit_and_children_get_the_original() {
    let dir = std::env::temp_dir().join(format!("fung-rlimit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("f");
    let src = format!(
        "import fs\np = \"{}\"\nfs.write(p, \"x\")!\nhs = map(0..300, _ => fs.open(p)!)\nprint(len(hs))\nprint(trim(sh(\"ulimit -Sn\")!.out))",
        file.display()
    );
    let child = Reaped::new(
        Command::new("sh")
            .arg("-c")
            .arg("ulimit -Sn 256 && exec \"$0\" -e \"$1\"")
            .arg(fung_bin())
            .arg(&src)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let (o, killed) = child.output_within(Duration::from_secs(20)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!killed, "fung did not end");
    assert_eq!(
        (
            String::from_utf8_lossy(&o.stdout).as_ref(),
            String::from_utf8_lossy(&o.stderr).as_ref(),
            o.status.code()
        ),
        ("300\n256\n", "", Some(0))
    );
}
