//! The toolchain version `fung version` prints.
//!
//! The crates are all `0.0.0`, so `CARGO_PKG_VERSION` would print `0.0.0`
//! for every build. A release build sets `FUNG_BUILD_VERSION` when it
//! compiles; anything else is `0.0.0-dev`.

/// The version of this binary.
pub fn version() -> String {
    resolve(option_env!("FUNG_BUILD_VERSION"))
}

/// Go `resolveVersion` with one source: the build-time version unless it
/// is empty or Go's `(devel)` marker, without the tag's leading `v`, else
/// the development marker.
fn resolve(built: Option<&str>) -> String {
    match built {
        Some(v) if !v.is_empty() && v != "(devel)" => v.strip_prefix('v').unwrap_or(v).to_string(),
        _ => "0.0.0-dev".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_matches_go() {
        // Go `TestResolveVersion`, with the link-time value as the build
        // value and no module version.
        assert_eq!(resolve(Some("0.1.0")), "0.1.0");
        assert_eq!(resolve(Some("v0.1.0")), "0.1.0");
        assert_eq!(resolve(Some("(devel)")), "0.0.0-dev");
        assert_eq!(resolve(Some("")), "0.0.0-dev");
        assert_eq!(resolve(None), "0.0.0-dev");
        assert_eq!(resolve(Some("vv1")), "v1", "only one v is dropped");
    }
}
