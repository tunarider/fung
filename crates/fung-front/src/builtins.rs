//! The built-in names the front end needs, read from `spec/builtins.txt`.
//! The file is derived: `fung-vm`'s `spec_file` test builds it from the
//! runtime's own tables and the embedded extras (see there), so the Rust
//! code is the source of truth and the file is what the front end, the
//! manual checks and readers see. Resolve needs the prelude names; the
//! module lists describe the `_native.*` bridges and the parameters of
//! `conc` and `http`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

const SPEC: &str = include_str!("../../../spec/builtins.txt");

/// Built-in names by layer. Every list is sorted.
#[derive(Debug, Default)]
pub struct Builtins {
    /// Native values of the prelude (functions and plain values such as `args`).
    pub prelude_native: Vec<String>,
    /// Public prelude names of the prelude files written in fung
    /// (`extras/prelude.fung`, `extras/agent.fung`).
    pub prelude_fung: Vec<String>,
    /// `_native.x` → its member names; also the standard modules written
    /// in fung that the spec lists (`conc`, `http`) → their public names.
    pub modules: BTreeMap<String, Vec<String>>,
    /// Prelude natives that are functions → their parameter list as the
    /// spec writes it (`xs, sep=?`, `...args`).
    native_params: BTreeMap<String, String>,
    /// `(_native.x, name)` (or `(conc, name)`) → the member's parameter list.
    module_params: BTreeMap<(String, String), String>,
}

impl Builtins {
    /// The parameter list of a prelude native function as recorded in the
    /// spec, without the parentheses: `xs, f`, `code=?`, `...args`. `None`
    /// for an unknown name or a plain value such as `args`.
    pub fn native_signature(&self, name: &str) -> Option<&str> {
        self.native_params.get(name).map(String::as_str)
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
        let mut parts = line.splitn(3, ' ');
        let (kind, layer, rest) = (parts.next(), parts.next(), parts.next());
        match (kind, layer, rest) {
            (Some("prelude"), Some("native"), Some(rest)) => {
                let name = entry_name(rest);
                if let Some(params) = entry_params(rest) {
                    b.native_params.insert(name.clone(), params);
                }
                b.prelude_native.push(name)
            }
            (Some("prelude"), Some("fung"), Some(rest)) => b.prelude_fung.push(entry_name(rest)),
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

    /// Stage 3b: there are no atom natives; the agent functions of
    /// `extras/agent.fung` are prelude names.
    #[test]
    fn agents_are_prelude_names_written_in_fung() {
        let b = builtins();
        for name in ["atom", "swap", "deref"] {
            assert!(!b.prelude_native.iter().any(|n| n == name), "{name}");
            assert_eq!(b.native_signature(name), None, "{name}");
        }
        for name in ["agent", "deref", "swap"] {
            assert!(b.prelude_fung.iter().any(|n| n == name), "{name}");
        }
        let names = b.prelude_names();
        assert!(names.windows(2).all(|w| w[0] < w[1]));
        for name in ["agent", "swap", "deref", "print", "take"] {
            assert!(names.iter().any(|n| n == name), "{name}");
        }
        assert!(!names.iter().any(|n| n == "atom"));
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
        assert_eq!(b.module_signature("conc", "fan_in"), Some("chs"));
        assert_eq!(b.module_signature("conc", "pmap"), Some("xs, f, limit=?"));
        assert_eq!(
            b.module_signature("conc", "retry"),
            Some("f, times=?, delay=?")
        );
    }

    #[test]
    fn module_signature_is_the_spec_parameter_list() {
        let b = builtins();
        assert_eq!(
            b.module_signature("_native.process", "exec"),
            Some("cmd, cwd=?, env=?, stdin=?")
        );
        assert_eq!(b.module_signature("_native.process", "env"), Some("name=?"));
        assert_eq!(
            b.module_signature("_native.conc", "spawn_awaited"),
            Some("f")
        );
        assert_eq!(b.module_signature("_native.fs", "temp_dir"), Some(""));
        assert_eq!(
            b.module_signature("_native.conc", "agent_busy"),
            Some("ch, on, state=?")
        );
        assert!(b.modules["_native.conc"].contains(&"try_call".to_string()));
        assert_eq!(b.module_signature("_native.process", "nope"), None);
        assert_eq!(b.module_signature("_native.nope", "exec"), None);
        assert_eq!(
            b.module_signature("http", "serve"),
            Some("addr, handler, ready=?")
        );
        assert!(!b.modules.contains_key("_native.http"));
    }
}
