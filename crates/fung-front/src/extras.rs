//! The built-in extras (06): the modules written in fung that ship inside
//! the binary. `include_str!` embeds `extras/*.fung`.

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
        // Stage 4d: written in fung over `_native.net`.
        "http" => include_str!("../../../extras/http.fung"),
        // Stage 6a: `fan_in`, `pmap` and `retry`.
        "conc" => include_str!("../../../extras/conc.fung"),
        _ => return None,
    })
}

/// Where a built-in file's source lives, as call paths show it:
/// `extras/<name>.fung`.
pub fn path(name: &str) -> String {
    format!("extras/{name}.fung")
}

/// `extras/agent.fung`: `agent`, `swap` and `deref` (stage 3b). The
/// boot reads it after the prelude file and adds its public names to the
/// prelude; it is not a module a program can import (`source` does not
/// know it).
pub const AGENT: &str = include_str!("../../../extras/agent.fung");

/// The prelude files written in fung, by name, in the order the boot
/// runs them (each sees the public names of the ones before it). Their
/// public names are prelude names.
pub const PRELUDE_FILES: &[(&str, &str)] = &[
    ("prelude", include_str!("../../../extras/prelude.fung")),
    ("agent", AGENT),
];

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
        assert_eq!(path("http"), "extras/http.fung");
        assert!(source("agent").is_none());
        assert!(source("conc").unwrap().contains("fn pmap("));
        assert_eq!(path("conc"), "extras/conc.fung");
        assert_eq!(path("agent"), "extras/agent.fung");
        assert_eq!(PRELUDE_FILES[0], ("prelude", source("prelude").unwrap()));
        assert!(
            source("prelude")
                .unwrap()
                .contains("import _native.process")
        );
    }
}
