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

/// The standard modules written in fung (the prelude files aside), as
/// `import` names them. A test keeps it equal to `extras/`.
pub const MODULES: &[&str] = &["fs", "path", "cli", "json", "time", "regex", "http", "conc"];

/// Where a built-in file's source lives, as call paths show it:
/// `extras/<name>.fung`.
pub fn path(name: &str) -> String {
    format!("extras/{name}.fung")
}

/// The prelude files written in fung, by name, in the order the boot
/// runs them (each sees the public names of the ones before it). Their
/// public names are prelude names.
pub const PRELUDE_FILES: &[(&str, &str)] =
    &[("prelude", include_str!("../../../extras/prelude.fung"))];

#[cfg(test)]
mod tests {
    use super::*;

    /// `MODULES` and the prelude files are the `extras/` directory, one
    /// for one: a new file there is a module the lists must name.
    #[test]
    fn the_lists_are_the_extras_directory() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../extras");
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter_map(|n| n.strip_suffix(".fung").map(str::to_string))
            .collect();
        files.sort();
        let mut listed: Vec<String> = MODULES
            .iter()
            .chain(PRELUDE_FILES.iter().map(|(n, _)| n))
            .map(|n| n.to_string())
            .collect();
        listed.sort();
        assert_eq!(files, listed);
        for m in MODULES {
            assert!(source(m).is_some(), "{m}");
        }
    }

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
