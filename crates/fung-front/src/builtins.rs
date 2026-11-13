//! The built-in names the front end needs, read from `spec/builtins.txt`.
//! The file is derived: `fung-vm`'s `spec_file` test builds it from the
//! runtime's own tables and the embedded extras (see there), so the Rust
//! code is the source of truth and the file is what the front end, the
//! manual checks and readers see. Resolve needs the prelude names; the
//! module lists describe the `_native.*` bridges and the parameters of
//! `conc` and `http`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const SPEC: &str = include_str!("../../../spec/builtins.txt");

/// Built-in names by layer. Every list is sorted.
#[derive(Debug, Default)]
pub struct Builtins {
    /// Native values of the prelude (functions and plain values such as `args`).
    pub prelude_native: Vec<String>,
    /// Public prelude names of the prelude files written in fung
    /// (`extras/prelude.fung`).
    pub prelude_fung: Vec<String>,
    /// `_native.x` → its member names; also the standard modules written
    /// in fung that the spec lists (`conc`, `http`) → their public names.
    pub modules: BTreeMap<String, Vec<String>>,
    /// Prelude natives that are functions → their parameter list as the
    /// spec writes it (`xs, sep=?`, `...args`).
    native_params: BTreeMap<String, String>,
    /// Prelude functions written in fung → their parameter list.
    fung_params: BTreeMap<String, String>,
    /// `(_native.x, name)` (or `(conc, name)`) → the member's parameter list.
    module_params: BTreeMap<(String, String), String>,
    /// `(prelude, name)` and `(module, name)` of the functions known to
    /// have no effect (the spec's `pure` mark): a call of one as a
    /// statement only computes a value to drop it (E0234, D39).
    pure: BTreeSet<(String, String)>,
}

impl Builtins {
    /// The parameter list of a prelude native function as recorded in the
    /// spec, without the parentheses: `xs, f`, `code=?`, `...args`. `None`
    /// for an unknown name or a plain value such as `args`.
    pub fn native_signature(&self, name: &str) -> Option<&str> {
        self.native_params.get(name).map(String::as_str)
    }

    /// The parameter list of a prelude function, native or written in
    /// fung, as the spec writes it. `None` for an unknown name or a value
    /// that is no function.
    pub fn prelude_signature(&self, name: &str) -> Option<&str> {
        self.native_params
            .get(name)
            .or_else(|| self.fung_params.get(name))
            .map(String::as_str)
    }

    /// Whether the prelude function `name` is known to have no effect
    /// (D39).
    pub fn prelude_pure(&self, name: &str) -> bool {
        self.pure
            .contains(&("prelude".to_string(), name.to_string()))
    }

    /// Whether the member `name` of `module` (a `_native.x` bridge, `conc`
    /// or `http`) is known to have no effect (D39).
    pub fn module_pure(&self, module: &str, name: &str) -> bool {
        self.pure.contains(&(module.to_string(), name.to_string()))
    }

    /// The parameter list of a `_native.x` module member as the spec
    /// writes it, like [`Builtins::native_signature`].
    pub fn module_signature(&self, module: &str, name: &str) -> Option<&str> {
        self.module_params
            .get(&(module.to_string(), name.to_string()))
            .map(String::as_str)
    }
}

pub fn builtins() -> &'static Builtins {
    static CELL: OnceLock<Builtins> = OnceLock::new();
    CELL.get_or_init(|| parse(SPEC))
}

impl Builtins {
    /// Every prelude name: the natives, then the public names of the
    /// prelude files written in fung, sorted.
    pub fn prelude_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .prelude_native
            .iter()
            .chain(&self.prelude_fung)
            .cloned()
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

/// `name(params)` or a bare `name` → `name`.
fn entry_name(s: &str) -> String {
    s.split('(').next().unwrap_or(s).trim().to_string()
}

/// `name(params)` → `params`; a bare `name` → `None`.
fn entry_params(s: &str) -> Option<String> {
    let (_, rest) = s.split_once('(')?;
    Some(rest.strip_suffix(')').unwrap_or(rest).to_string())
}

/// Reads a spec in the format of `spec/builtins.txt`. Public for the
/// generator, which builds the spec from the runtime's tables a step at
/// a time and compiles the extras against each step
/// ([`crate::loader::Loader::spec`]).
pub fn parse(spec: &str) -> Builtins {
    let mut b = Builtins::default();
    for line in spec.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // A function known to have no effect ends with ` pure` (D39).
        let (line, pure) = match line.strip_suffix(" pure") {
            Some(l) => (l, true),
            None => (line, false),
        };
        let mut parts = line.splitn(3, ' ');
        let (kind, layer, rest) = (parts.next(), parts.next(), parts.next());
        if pure && let (Some(kind), Some(layer), Some(rest)) = (kind, layer, rest) {
            let place = if kind == "prelude" { "prelude" } else { layer };
            b.pure.insert((place.to_string(), entry_name(rest)));
        }
        match (kind, layer, rest) {
            (Some("prelude"), Some("native"), Some(rest)) => {
                let name = entry_name(rest);
                if let Some(params) = entry_params(rest) {
                    b.native_params.insert(name.clone(), params);
                }
                b.prelude_native.push(name)
            }
            (Some("prelude"), Some("fung"), Some(rest)) => {
                let name = entry_name(rest);
                if let Some(params) = entry_params(rest) {
                    b.fung_params.insert(name.clone(), params);
                }
                b.prelude_fung.push(name)
            }
            (Some("module"), Some(module), Some(rest)) => {
                let name = entry_name(rest);
                if let Some(params) = entry_params(rest) {
                    b.module_params
                        .insert((module.to_string(), name.clone()), params);
                }
                b.modules.entry(module.to_string()).or_default().push(name)
            }
            _ => panic!("spec/builtins.txt: malformed line {line:?}"),
        }
    }
    b.prelude_native.sort();
    b.prelude_fung.sort();
    for names in b.modules.values_mut() {
        names.sort();
    }
    b
}

#[cfg(test)]
mod tests {
    use super::builtins;

    #[test]
    fn native_signature_is_the_spec_parameter_list() {
        let b = builtins();
        assert_eq!(b.native_signature("fold"), Some("xs, init, f"));
        assert_eq!(b.native_signature("join"), Some("xs, sep=?"));
        assert_eq!(b.native_signature("print"), Some("...args"));
        assert_eq!(b.native_signature("args"), None);
        assert_eq!(b.native_signature("take"), None);
    }

    /// Concurrency v2 L5: the atom functions are prelude natives, and
    /// `agent` is gone.
    #[test]
    fn atoms_are_prelude_natives_and_agent_is_gone() {
        let b = builtins();
        for name in ["atom", "deref", "swap", "reset"] {
            assert!(b.prelude_native.iter().any(|n| n == name), "{name}");
        }
        assert_eq!(b.native_signature("swap"), Some("a, f"));
        let names = b.prelude_names();
        assert!(names.windows(2).all(|w| w[0] < w[1]));
        assert!(!names.iter().any(|n| n == "agent"));
    }

    /// Stage 6a: `fan_in`, `pmap` and `retry` are the fung module `conc`
    /// (`extras/conc.fung`), not prelude names.
    #[test]
    fn the_conc_helpers_are_a_module_not_prelude_names() {
        let b = builtins();
        let names = b.prelude_names();
        for name in ["fan_in", "pmap", "retry"] {
            assert!(!names.iter().any(|n| n == name), "{name}");
            assert!(!b.prelude_fung.iter().any(|n| n == name), "{name}");
        }
        assert_eq!(b.modules["conc"], ["fan_in", "pmap", "retry"]);
        assert_eq!(b.module_signature("conc", "fan_in"), Some("s, rxs"));
        assert_eq!(b.module_signature("conc", "pmap"), Some("xs, f, limit=?"));
        assert_eq!(
            b.module_signature("conc", "retry"),
            Some("f, times=?, delay=?")
        );
    }

    /// D39: the spec marks the functions known to have no effect; those
    /// that take a function, and effects, are not marked.
    #[test]
    fn pure_functions_are_marked() {
        let b = builtins();
        // `take` and `zip` are written in fung: the mark is the same
        // whichever language a builtin is written in (VA47).
        for name in ["put", "sort", "len", "join", "env", "take", "zip"] {
            assert!(b.prelude_pure(name), "{name}");
        }
        // Those that take a function to compute a value with are marked
        // too (D90); `each` runs its function for what it does.
        for name in [
            "map", "filter", "fold", "find", "any", "all", "sort_by", "group_by", "try_map",
        ] {
            assert!(b.prelude_pure(name), "{name}");
        }
        for name in ["each", "print", "fail", "send", "until", "scope", "with"] {
            assert!(!b.prelude_pure(name), "{name}");
        }
        assert!(b.module_pure("_native.path", "join"));
        assert!(!b.module_pure("_native.fs", "read"));
        assert!(!b.module_pure("_native.arg", "count"));
        let s = super::parse("prelude native len(x) pure\nmodule _native.path base(p) pure\n");
        assert!(s.prelude_pure("len"));
        assert_eq!(s.native_signature("len"), Some("x"));
        assert!(s.module_pure("_native.path", "base"));
        assert_eq!(s.module_signature("_native.path", "base"), Some("p"));
    }

    #[test]
    fn module_signature_is_the_spec_parameter_list() {
        let b = builtins();
        assert_eq!(
            b.module_signature("_native.process", "exec"),
            Some("cmd, cwd=?, env=?, stdin=?")
        );
        assert_eq!(b.module_signature("_native.process", "env"), Some("name"));
        assert_eq!(b.module_signature("_native.process", "env_all"), Some(""));
        assert_eq!(b.module_signature("_native.fs", "temp_dir"), Some(""));
        assert!(!b.modules.contains_key("_native.conc"), "gone with R1b");
        assert_eq!(b.module_signature("_native.process", "nope"), None);
        assert_eq!(b.module_signature("_native.nope", "exec"), None);
        assert_eq!(
            b.module_signature("http", "serve"),
            Some("addr, handler, ready=?, drain=?, reuse_port=?")
        );
        assert!(!b.modules.contains_key("_native.http"));
    }
}
