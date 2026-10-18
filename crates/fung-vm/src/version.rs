//! The toolchain version: what `fung version` prints and the `http`
//! client names in `User-Agent`.
//!
//! The crates are all `0.0.0`, so `CARGO_PKG_VERSION` would say `0.0.0`
//! for every build. A release build sets `FUNG_BUILD_VERSION` when it
//! compiles; anything else is `0.0.0-dev`.

/// The version of this build.
pub fn version() -> String {
    resolve(option_env!("FUNG_BUILD_VERSION"))
}

/// The build-time version without a tag's leading `v`, or the
/// development marker when there is none.
fn resolve(built: Option<&str>) -> String {
    match built {
        Some(v) if !v.is_empty() => v.strip_prefix('v').unwrap_or(v).to_string(),
        _ => "0.0.0-dev".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_version_or_the_development_marker() {
        assert_eq!(resolve(Some("0.1.0")), "0.1.0");
        assert_eq!(resolve(Some("v0.1.0")), "0.1.0");
        assert_eq!(resolve(Some("")), "0.0.0-dev");
        assert_eq!(resolve(None), "0.0.0-dev");
        assert_eq!(resolve(Some("vv1")), "v1", "only one v is dropped");
    }
}
