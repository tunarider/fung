//! `install.sh` against a fake release the test builds itself: an archive
//! whose `fung` is a shell script, and the `SHA256SUMS` that goes with it.

mod common;

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// A scratch directory removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("fung-install-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sh_output(cmd: &mut Command) -> String {
    let out = common::Reaped::new(
        common::spawn(
            cmd.stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )
        .expect("spawn"),
    )
    .output()
    .unwrap();
    assert!(out.status.success(), "{cmd:?} failed: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// The `<os>_<arch>` suffix `install.sh` looks for on this machine.
fn platform() -> String {
    let os = match sh_output(Command::new("uname").arg("-s")).as_str() {
        "Linux" => "linux",
        "Darwin" => "darwin",
        other => panic!("install.sh does not support {other}"),
    };
    let arch = match sh_output(Command::new("uname").arg("-m")).as_str() {
        "x86_64" | "amd64" => "amd64",
        "aarch64" | "arm64" => "arm64",
        other => panic!("install.sh does not support {other}"),
    };
    format!("{os}_{arch}")
}

fn sha256(file: &Path) -> String {
    let sum = if common::output(Command::new("sha256sum").arg("--version")).is_ok() {
        sh_output(Command::new("sha256sum").arg(file))
    } else {
        sh_output(Command::new("shasum").args(["-a", "256"]).arg(file))
    };
    sum.split_whitespace().next().unwrap().to_string()
}

/// Writes what a GitHub release holds for this platform into
/// `dir/v<version>/`; `corrupt` flips the first digit of the listed checksum.
fn fake_release(dir: &Path, version: &str, corrupt: bool) {
    let name = format!("fung_{version}_{}", platform());
    let rel = dir.join(format!("v{version}"));
    let stage = dir.join("stage").join(&name);
    fs::create_dir_all(&rel).unwrap();
    fs::create_dir_all(&stage).unwrap();
    fs::write(
        stage.join("fung"),
        format!("#!/bin/sh\necho fung {version}\n"),
    )
    .unwrap();
    fs::write(stage.join("LICENSE"), "MIT\n").unwrap();
    sh_output(Command::new("chmod").arg("755").arg(stage.join("fung")));
    let archive = rel.join(format!("{name}.tar.gz"));
    sh_output(
        Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(dir.join("stage"))
            .arg(&name),
    );
    let mut sum = sha256(&archive);
    if corrupt {
        let first = if sum.starts_with('0') { "1" } else { "0" };
        sum = format!("{first}{}", &sum[1..]);
    }
    fs::write(rel.join("SHA256SUMS"), format!("{sum}  {name}.tar.gz\n")).unwrap();
}

/// Runs `install.sh` with `sh`; the combined output and whether it succeeded.
fn run_install(env: &[(&str, String)]) -> (String, bool) {
    let mut cmd = Command::new("sh");
    cmd.arg(common::repo().join("install.sh"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = common::Reaped::new(common::spawn(&mut cmd).expect("spawn sh"))
        .output()
        .unwrap();
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (text, out.status.success())
}

fn env_for(version: &str, bin: &Path, rel: &Path) -> Vec<(&'static str, String)> {
    vec![
        ("FUNG_VERSION", version.to_string()),
        ("FUNG_INSTALL_DIR", bin.display().to_string()),
        ("FUNG_DOWNLOAD_BASE", format!("file://{}", rel.display())),
    ]
}

fn have(tool: &str) -> bool {
    common::output(Command::new(tool).arg("--version"))
        .map(|_| true)
        .unwrap_or(false)
}

#[test]
fn installs_and_verifies() {
    if !have("curl") {
        eprintln!("curl not found; skipping");
        return;
    }
    let rel = Scratch::new("ok-rel");
    let bin = Scratch::new("ok-bin");
    fake_release(&rel.0, "9.9.9", false);
    let (out, ok) = run_install(&env_for("9.9.9", &bin.0, &rel.0));
    assert!(ok, "install failed:\n{out}");
    let got = sh_output(&mut Command::new(bin.0.join("fung")));
    assert_eq!(got, "fung 9.9.9", "{out}");
    assert!(
        out.contains("PATH"),
        "a directory not on PATH should be pointed out:\n{out}"
    );
}

#[test]
fn rejects_bad_checksum() {
    if !have("curl") {
        eprintln!("curl not found; skipping");
        return;
    }
    let rel = Scratch::new("bad-rel");
    let bin = Scratch::new("bad-bin");
    fake_release(&rel.0, "9.9.9", true);
    let (out, ok) = run_install(&env_for("v9.9.9", &bin.0, &rel.0));
    assert!(!ok, "install succeeded with a bad checksum:\n{out}");
    assert!(
        out.contains("checksum"),
        "the error should name the checksum:\n{out}"
    );
    assert!(
        !bin.0.join("fung").exists(),
        "a binary with a bad checksum was installed"
    );
}
