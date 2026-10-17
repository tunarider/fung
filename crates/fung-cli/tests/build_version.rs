//! `fung version` prints the version the build was given in
//! `FUNG_BUILD_VERSION` (`scripts/build-release.sh` sets it; install.sh's
//! `FUNG_VERSION` is a different thing, the version to install). Cargo
//! rebuilds the binary when that variable changes, so the binary under
//! test was built with the variable this test process sees. CI runs this
//! once with the variable set (`ci.yml`).

use std::process::Command;

#[test]
fn fung_version_reports_the_build_version() {
    let want = match std::env::var("FUNG_BUILD_VERSION") {
        Ok(v) if !v.is_empty() && v != "(devel)" => v.strip_prefix('v').unwrap_or(&v).to_string(),
        _ => "0.0.0-dev".to_string(),
    };
    let out = Command::new(env!("CARGO_BIN_EXE_fung"))
        .arg("version")
        .output()
        .expect("run fung version");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("fung {want}\n")
    );
}
