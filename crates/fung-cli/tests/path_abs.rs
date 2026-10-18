//! `path.abs` resolves a relative path against `$PWD` when that names the
//! working directory, so a symlinked directory prints as
//! the user reached it, not as `getcwd` reports it.

mod common;

use std::process::Command;

use common::fung_bin;

fn run_in(bin: &std::path::Path, dir: &std::path::Path, pwd: &std::path::Path) -> String {
    let o = Command::new(bin)
        .args(["-e", "import path\nprint(path.abs(\"a\"))"])
        .current_dir(dir)
        .env("PWD", pwd)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

#[test]
fn abs_keeps_the_symlinked_working_directory() {
    let base = std::env::temp_dir().join(format!("fung-pathabs-{}", std::process::id()));
    let real = base.join("real");
    let link = base.join("link");
    std::fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let want = format!("{}\n", link.join("a").display());

    let bin = run_in(&fung_bin(), &link, &link);
    let _ = std::fs::remove_dir_all(&base);
    assert_eq!(bin, want);
}
