//! `scripts/changelog-section.sh`, which the release step relies on, and
//! the pairing of the real CHANGELOG with the manual's sidebar version.

mod common;

use std::{
    fs,
    process::{Command, Stdio},
};

/// The notes of `version` in `file`, or the failure output.
fn section(file: &std::path::Path, version: &str) -> Result<String, String> {
    let child = Command::new("sh")
        .arg(common::repo().join("scripts/changelog-section.sh"))
        .arg(version)
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sh");
    let out = common::Reaped::new(child).output().unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    if out.status.success() {
        Ok(text)
    } else {
        Err(text)
    }
}

#[test]
fn section_extracts_one_version() {
    let dir = std::env::temp_dir().join(format!("fung-changelog-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let f = dir.join("CHANGELOG.md");
    fs::write(
        &f,
        "# Changelog\n\n## [Unreleased]\n\n- pending\n\n## [0.2.0] - 2026-11-01\n\n### Added\n- two\n\n\
         ## [0.1.0] - 2026-10-14\n\n- one\n\n[0.2.0]: https://example.com/0.2.0\n",
    )
    .unwrap();
    let got = section(&f, "v0.2.0");
    let missing = section(&f, "0.3.0");
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(got.unwrap(), "### Added\n- two\n");
    assert!(
        missing.is_err(),
        "a version with no section must fail the release"
    );
}

/// The real changelog must have a section for the version in the manual,
/// so a release tag never ships with empty notes.
#[test]
fn covers_the_manual_version() {
    let page = fs::read_to_string(common::repo().join("docs/index.html")).unwrap();
    let rest = page
        .split_once("<p class=\"side-foot\">fung ")
        .expect("no version in the manual sidebar")
        .1;
    let version = rest.split(' ').next().unwrap();
    let got = section(&common::repo().join("CHANGELOG.md"), version)
        .unwrap_or_else(|e| panic!("CHANGELOG.md has no notes for {version}: {e}"));
    assert!(
        !got.trim().is_empty(),
        "CHANGELOG.md has no notes for {version}"
    );
}
