//! The built-in extras (06): the modules written in fung that ship inside
//! the binary. `include_str!` embeds `extras/*.fung`; the names are the
//! Go `extras.Source` names.

/// The fung source of a built-in module by name (`prelude`, `fs`, ...).
pub fn source(name: &str) -> Option<&'static str> {
    Some(match name {
        "prelude" => include_str!("../../../extras/prelude.fung"),
        "fs" => include_str!("../../../extras/fs.fung"),
        "path" => include_str!("../../../extras/path.fung"),
        "cli" => include_str!("../../../extras/cli.fung"),
        "time" => include_str!("../../../extras/time.fung"),
        "json" => include_str!("../../../extras/json.fung"),
        "regex" => include_str!("../../../extras/regex.fung"),
        "http" => include_str!("../../../extras/http.fung"),
        _ => return None,
    })
}

/// `extras/agent.fung`: `agent`, `swap` and `deref` (stage 3b). Only
/// the Rust boot reads it and adds its public names to the prelude; it
/// is not a module a program can import (`source` does not know it), and
/// the Go implementation, which keeps its atoms, never reads it.
pub const AGENT: &str = include_str!("../../../extras/agent.fung");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_returns_embedded_modules() {
        for name in [
            "prelude", "fs", "path", "cli", "time", "json", "regex", "http",
        ] {
            assert!(!source(name).unwrap().is_empty(), "{name}");
        }
        assert!(source("nope").is_none());
        assert!(source("agent").is_none());
        assert!(
            source("prelude")
                .unwrap()
                .contains("import _native.process")
        );
    }
}
