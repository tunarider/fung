//! `scripts/changelog-section.sh`, which the release step relies on, and
//! the pairing of the real CHANGELOG with the manual's sidebar version.

mod common;

use std::{
    fs,
    process::{Command, Stdio},
};

/// The notes of `version` in `file`, or the failure output.
fn section(file: &std::path::Path, version: &str) -> Result<String, String> {
    let child = common::spawn(
        Command::new("sh")
            .arg(common::repo().join("scripts/changelog-section.sh"))
            .arg(version)
            .arg(file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
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

/// The real changelog must have a section for the newest frozen manual
/// (`docs/versions.js`, written by `manual_build.rs`'s freeze), so a
/// release tag never ships with empty notes. Before the first frozen
/// version there is nothing to check.
#[test]
fn covers_the_newest_frozen_manual() {
    let js = fs::read_to_string(common::repo().join("docs/versions.js")).unwrap();
    let frozen = js
        .lines()
        .filter(|l| l.trim_start().starts_with('{') && !l.contains("\"tag\""))
        .map(|l| {
            let rest = &l[l.find("\"name\": \"").expect("a version without a name") + 9..];
            rest[..rest.find('"').unwrap()].to_string()
        })
        .next();
    let Some(version) = frozen else {
        return;
    };
    let got = section(&common::repo().join("CHANGELOG.md"), &version)
        .unwrap_or_else(|e| panic!("CHANGELOG.md has no notes for {version}: {e}"));
    assert!(
        !got.trim().is_empty(),
        "CHANGELOG.md has no notes for {version}"
    );
}
