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
        // Stage 4d: the Rust `http` is written in fung over `_native.net`
        // (Go's `extras/http.fung` bridges Go's net/http instead).
        "http" => include_str!("../../../extras/rust/http.fung"),
        // Stage 6a, Rust only: `fan_in`, `pmap` and `retry`, which Go
        // keeps in its prelude.
        "conc" => include_str!("../../../extras/rust/conc.fung"),
        _ => return None,
    })
}

/// Where a built-in module's source lives, as call paths show it:
/// `extras/<name>.fung`, but `extras/rust/<name>.fung` for the Rust-only
/// `http` (stage 4d) and `conc` (stage 6a).
pub fn path(name: &str) -> String {
    match name {
        "http" | "conc" => format!("extras/rust/{name}.fung"),
        _ => format!("extras/{name}.fung"),
    }
}

/// `extras/rust/agent.fung`: `agent`, `swap` and `deref` (stage 3b).
/// Only the Rust boot reads it and adds its public names to the
/// prelude; it is not a module a program can import (`source` does not
/// know it). The Go implementation, which keeps its atoms, never reads
/// it: its own directory keeps it out of Go's `//go:embed *.fung`.
pub const AGENT: &str = include_str!("../../../extras/rust/agent.fung");

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
        assert_eq!(path("json"), "extras/json.fung");
        assert_eq!(path("http"), "extras/rust/http.fung");
        assert!(source("agent").is_none());
        // Stage 6a, Rust only: `fan_in`, `pmap` and `retry`.
        assert!(source("conc").unwrap().contains("fn pmap("));
        assert_eq!(path("conc"), "extras/rust/conc.fung");
        assert!(
            source("prelude")
                .unwrap()
                .contains("import _native.process")
        );
    }
}
