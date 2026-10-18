//! The module loader (04: load time): compiles an entry file and its whole
//! import graph up front, so cycles, private access, top-level
//! restrictions and unknown names are compile errors.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use crate::builtins::{Builtins, builtins};
use crate::bytecode::Proto;
use crate::compiler;
use crate::desugar::desugar;
use crate::diag::{Arg, Diag, error};
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{self, ModuleInfo};
use crate::source::{FileId, Pos, SourceMap, decode_lossy};
use crate::tree::{Head, Import, Lit, Node};
use crate::unicode;

/// How far `compile` should go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Lex and parse only.
    Ast,
    /// Through desugar and resolve.
    Core,
    /// Through the compiler.
    All,
}

/// One module of a program, under the key IMPORT uses at run time.
#[derive(Debug)]
pub struct Loaded {
    pub key: String,
    pub name: String,
    /// `None` for a native (`_native.*`) module.
    pub proto: Option<Proto>,
}

/// Whatever stages ran. Fields past the requested stage, or past the
/// first failing stage, are `None`. `sources` is filled even when errors
/// are returned, so the caller can excerpt them.
#[derive(Debug, Default)]
pub struct Build {
    pub surface: Option<Node>,
    pub core: Option<Node>,
    pub proto: Option<Proto>,
    pub sources: SourceMap,
    /// The imported modules in dependency order (a module comes after
    /// what it imports).
    pub modules: Vec<Loaded>,
    /// W01xx warnings of every module that resolved, in dependency order
    /// (a module's come after those of what it imports), without the
    /// embedded stdlib's: users cannot act on them. Only `check` and
    /// `test` show them (08).
    pub warnings: Vec<Diag>,
}

/// Compiles an entry file and everything it imports.
pub struct Loader {
    /// Reads an imported file. By default it reads the bytes and decodes
    /// them (one U+FFFD per invalid sequence, see `decode_lossy`).
    pub read_file: fn(&str) -> std::io::Result<String>,
    /// Keep test blocks of the entry (`fung test` / `check`), see 08.
    pub tests: bool,
    /// The entry is an embedded extras file: it may import `_native.*`.
    pub internal: bool,
    /// Names added to the prelude (VM tests use it for natives that exist only in tests).
    pub extra_prelude: Vec<String>,
    /// REPL (08): the globals earlier inputs defined, for the entry file
    /// only.
    pub globals: Vec<String>,
    /// REPL: the entry file may define fns and imports again.
    pub redefine: bool,
    /// The built-in names to compile against; `None` (the default) is
    /// `spec/builtins.txt`. The generator of that file sets its own, built
    /// from the runtime's tables, so a new `_native.*` bridge or prelude
    /// native needs no hand edit of the file first.
    pub spec: Option<&'static Builtins>,
}

impl Default for Loader {
    fn default() -> Loader {
        Loader {
            read_file: |p| std::fs::read(p).map(|b| decode_lossy(&b)),
            tests: false,
            internal: false,
            extra_prelude: Vec::new(),
            globals: Vec::new(),
            redefine: false,
            spec: None,
        }
    }
}

struct State<'a> {
    l: &'a Loader,
    entry_key: String,
    done: HashMap<String, usize>,
    loading: HashSet<String>,
    /// The DFS path (key, shown path), for E0302 messages.
    stack: Vec<(String, String)>,
    modules: Vec<Loaded>,
    sources: SourceMap,
    prelude: Vec<String>,
    prelude_set: BTreeSet<String>,
    warnings: Vec<Diag>,
}

type Errs = Vec<Diag>;

impl Loader {
    /// The built-in names this loader compiles against ([`Loader::spec`]).
    pub fn builtins(&self) -> &'static Builtins {
        self.spec.unwrap_or_else(builtins)
    }

    /// Runs the front end on one entry file ("-e" for inline code) up to
    /// a stage. Syntax errors stop at the first; name errors are all
    /// collected (08).
    pub fn compile(&self, file: &str, src: &str, up_to: Stage) -> (Build, Vec<Diag>) {
        self.compile_after(SourceMap::new(), file, src, up_to)
    }

    /// `compile` with the files of `prior` first: the new files' ids come
    /// after them, and `Build::sources` holds both. The REPL compiles each
    /// input this way, so the positions of every input and module name
    /// one file of the session's runtime (08).
    pub fn compile_after(
        &self,
        prior: SourceMap,
        file: &str,
        src: &str,
        up_to: Stage,
    ) -> (Build, Vec<Diag>) {
        let mut prelude: Vec<String> = self.builtins().prelude_names();
        prelude.extend(self.extra_prelude.iter().cloned());
        prelude.sort();
        prelude.dedup();
        let key = if file == "-e" {
            file.to_string()
        } else {
            abs_key(file)
        };
        let mut st = State {
            l: self,
            entry_key: key.clone(),
            done: HashMap::new(),
            loading: HashSet::new(),
            stack: vec![(key.clone(), file.to_string())],
            modules: Vec::new(),
            sources: prior,
            prelude_set: prelude.iter().cloned().collect(),
            prelude,
            warnings: Vec::new(),
        };
        st.loading.insert(key.clone());
        let id = st.sources.add(file, src);
        let (mut b, errs) = st.compile_file(&key, file, id, src, false, self.internal, up_to);
        b.sources = st.sources;
        b.modules = st.modules;
        b.warnings = st.warnings;
        (b, errs)
    }
}

impl State<'_> {
    /// Parses one file, loads what it imports (depth first), then
    /// resolves and compiles it with the exports of its imports in hand.
    #[allow(clippy::too_many_arguments)]
    fn compile_file(
        &mut self,
        key: &str,
        file: &str,
        id: FileId,
        src: &str,
        library: bool,
        is_stdlib: bool,
        up_to: Stage,
    ) -> (Build, Errs) {
        let mut b = Build::default();
        let toks = match lex(id, src) {
            Ok((t, _)) => t,
            Err(e) => return (b, vec![e]),
        };
        let surface = match parse(&toks) {
            Ok(n) => n,
            Err(e) => return (b, vec![e]),
        };
        b.surface = Some(surface.clone());
        if up_to == Stage::Ast {
            return (b, Vec::new());
        }
        let mut imports = BTreeMap::new();
        for k in &surface.kids {
            if k.head != Head::Import {
                continue; // nested imports are E0311 in resolve
            }
            let Lit::Import(im) = &k.lit else { continue };
            match self.load_import(im, k.pos, key, is_stdlib) {
                Ok(info) => {
                    imports.insert(im.spec.clone(), info);
                }
                Err(errs) => return (b, errs),
            }
        }
        let core = match desugar(&surface) {
            Ok(c) => c,
            Err(e) => return (b, vec![e]),
        };
        let mut ropts = resolve::Options {
            imports,
            library,
            ..resolve::Options::default()
        };
        if key == self.entry_key {
            ropts.globals.clone_from(&self.l.globals);
            ropts.redefine = self.l.redefine;
        }
        let mut res = match resolve::resolve(&core, &self.prelude, &ropts) {
            Ok(r) => r,
            Err(errs) => return (b, errs),
        };
        b.core = Some(res.core.clone());
        if !is_stdlib {
            self.warnings.append(&mut res.warnings);
        }
        if up_to == Stage::Core {
            return (b, Vec::new());
        }
        // A library's tests run only when it is the entry (08).
        let copts = compiler::Options {
            source: src.to_string(),
            file: file.to_string(),
            tests: self.l.tests && key == self.entry_key,
        };
        match compiler::compile(&res, &self.prelude_set, &copts) {
            Ok(mut p) => {
                if is_stdlib {
                    mark_builtin(&mut p);
                }
                b.proto = Some(p);
                (b, Vec::new())
            }
            Err(e) => (b, vec![e]),
        }
    }

    /// Resolves one import spec to a module key, loads it if it is new,
    /// and returns what resolve needs. Cycles (E0302), the entry file
    /// (E0307), internal modules from user code (E0310), bad file names
    /// (E0305) and missing modules (E0301) are reported at the import.
    fn load_import(
        &mut self,
        im: &Import,
        pos: Pos,
        from_key: &str,
        from_stdlib: bool,
    ) -> Result<ModuleInfo, Errs> {
        let spec = im.spec.as_str();
        let e = |code: &str, args: Vec<Arg>| vec![error(code, pos, args)];
        let s = |x: &str| Arg::Str(x.to_string());
        if im.is_path
            && (spec.starts_with("./") || spec.starts_with("../") || spec.starts_with('/'))
        {
            return self.load_file(im, pos, from_key);
        }
        if im.is_path {
            // Bare path strings are reserved for packages (04); none exist yet.
            return Err(e("E0301", vec![s(spec)]));
        }
        if let Some(public) = spec.strip_prefix("_native.") {
            if !from_stdlib {
                return Err(e("E0310", vec![s(spec), s(public)]));
            }
            let Some(names) = self.l.builtins().modules.get(spec) else {
                return Err(e("E0301", vec![s(spec)]));
            };
            if !self.done.contains_key(spec) {
                self.done.insert(spec.to_string(), self.modules.len());
                self.modules.push(Loaded {
                    key: spec.to_string(),
                    name: public.to_string(),
                    proto: None,
                });
            }
            return Ok(ModuleInfo {
                key: spec.to_string(),
                name: public.to_string(),
                exports: names.clone(),
            });
        }
        // prelude.fung is bootstrapped into the prelude table, not a module.
        let src = match crate::extras::source(spec) {
            Some(src) if spec != "prelude" => src,
            _ => return Err(e("E0301", vec![s(spec)])),
        };
        let file = crate::extras::path(spec);
        self.load_module(spec, spec, &file, pos, true, &mut || Ok(src.to_string()))
    }

    /// Resolves a relative or absolute path against the importing file's
    /// directory (04), or the working directory for `-e`.
    fn load_file(&mut self, im: &Import, pos: Pos, from_key: &str) -> Result<ModuleInfo, Errs> {
        let mut path = im.spec.clone();
        if !path.ends_with(".fung") {
            path.push_str(".fung");
        }
        let key = if path.starts_with('/') {
            clean(&path)
        } else {
            let dir = if from_key.starts_with('/') {
                dir_of(from_key)
            } else {
                ".".to_string()
            };
            abs_key(&format!("{dir}/{path}"))
        };
        if key == self.entry_key && from_key != key {
            return Err(vec![error("E0307", pos, vec![])]);
        }
        // The binding is a name, so it is NFC like the names that use it.
        let name = crate::unicode::nfc(&module_stem(&key)).into_owned();
        if !is_name(&name) && im.alias.is_empty() {
            return Err(vec![error("E0305", pos, vec![Arg::Str(name)])]);
        }
        let read = self.l.read_file;
        let k = key.clone();
        self.load_module(&key, &name, &display_path(&key), pos, false, &mut || {
            read(&k)
        })
    }

    /// Compiles a module once (library rules apply) and records it in
    /// dependency order. Re-entering a module that is still loading is a
    /// cycle; the message shows the whole path.
    fn load_module(
        &mut self,
        key: &str,
        name: &str,
        file: &str,
        pos: Pos,
        is_stdlib: bool,
        read: &mut dyn FnMut() -> std::io::Result<String>,
    ) -> Result<ModuleInfo, Errs> {
        if let Some(&i) = self.done.get(key) {
            let exports = self.modules[i]
                .proto
                .as_ref()
                .map(|p| p.exports.clone())
                .unwrap_or_default();
            return Ok(ModuleInfo {
                key: key.to_string(),
                name: name.to_string(),
                exports,
            });
        }
        if self.loading.contains(key) {
            let mut path: Vec<&str> = Vec::new();
            if let Some(i) = self.stack.iter().position(|(k, _)| k == key) {
                path.extend(self.stack[i..].iter().map(|(_, f)| f.as_str()));
            }
            path.push(file);
            return Err(vec![error("E0302", pos, vec![Arg::Str(path.join(" → "))])]);
        }
        let Ok(src) = read() else {
            return Err(vec![error("E0301", pos, vec![Arg::Str(key.to_string())])]);
        };
        let id = self.sources.add(file, &src);
        self.loading.insert(key.to_string());
        self.stack.push((key.to_string(), file.to_string()));
        let (b, errs) = self.compile_file(key, file, id, &src, true, is_stdlib, Stage::All);
        self.stack.pop();
        self.loading.remove(key);
        if !errs.is_empty() {
            return Err(errs);
        }
        let mut proto = b.proto.expect("compiled without errors");
        proto.name = format!("<module {name}>");
        let exports = proto.exports.clone();
        self.done.insert(key.to_string(), self.modules.len());
        self.modules.push(Loaded {
            key: key.to_string(),
            name: name.to_string(),
            proto: Some(proto),
        });
        Ok(ModuleInfo {
            key: key.to_string(),
            name: name.to_string(),
            exports,
        })
    }
}

/// Flags a proto tree as compiled from an embedded extras file, so
/// runtime errors inside it point at the user's call (06).
fn mark_builtin(p: &mut Proto) {
    p.builtin = true;
    for child in &mut p.protos {
        mark_builtin(child);
    }
}

/// A name (01): `_` or a name-start character that is not uppercase,
/// then name-continue characters (UAX #31).
fn is_name(s: &str) -> bool {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c == '_' || (unicode::is_name_start(c) && !unicode::is_upper(c)) => {}
        _ => return false,
    }
    cs.all(unicode::is_name_continue)
}

/// Lexical path cleaning (Unix): `.` and empty elements dropped, `..`
/// taken back where it can be.
fn clean(path: &str) -> String {
    if path.is_empty() {
        return ".".into();
    }
    let rooted = path.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if out.last().is_some_and(|l| *l != "..") {
                    out.pop();
                } else if !rooted {
                    out.push("..");
                }
            }
            p => out.push(p),
        }
    }
    let joined = out.join("/");
    match (rooted, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".into(),
        (false, false) => joined,
    }
}

fn dir_of(path: &str) -> String {
    match Path::new(path).parent() {
        Some(p) if !p.as_os_str().is_empty() => clean(&p.to_string_lossy()),
        _ => ".".into(),
    }
}

/// The working directory: `$PWD` when it names the current directory (so
/// symlinked paths print as the user typed them), else `getcwd`.
fn cwd() -> Option<String> {
    let real = std::env::current_dir().ok()?;
    if let Ok(pwd) = std::env::var("PWD")
        && pwd.starts_with('/')
        && std::fs::canonicalize(&pwd).ok() == std::fs::canonicalize(&real).ok()
    {
        return Some(pwd);
    }
    Some(real.to_string_lossy().into_owned())
}

/// An absolute, clean path, relative paths taken from
/// the working directory (`$PWD` when it names the same directory).
pub fn abs_path(path: &str) -> String {
    abs_key(path)
}

fn abs_key(path: &str) -> String {
    if path.starts_with('/') {
        return clean(path);
    }
    match cwd() {
        Some(wd) => clean(&format!("{wd}/{path}")),
        None => clean(path),
    }
}

/// How an imported file appears in positions: relative to the working
/// directory when it lies below it, absolute otherwise.
fn display_path(key: &str) -> String {
    if let Some(wd) = cwd() {
        let rel = rel(&clean(&wd), key);
        if !rel.starts_with("..") {
            return rel;
        }
    }
    key.to_string()
}

/// `target` relative to `base`, both clean absolute paths.
fn rel(base: &str, target: &str) -> String {
    let b: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    let t: Vec<&str> = target.split('/').filter(|s| !s.is_empty()).collect();
    let common = b.iter().zip(&t).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<&str> = vec![".."; b.len() - common];
    parts.extend(&t[common..]);
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

fn module_stem(key: &str) -> String {
    let base = key.rsplit('/').next().unwrap_or(key);
    base.strip_suffix(".fung").unwrap_or(base).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::disassemble;
    use crate::diag;
    use std::cell::RefCell;

    thread_local! {
        static FILES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    }

    fn set_files(files: &[(&str, &str)]) {
        FILES.with(|f| {
            *f.borrow_mut() = files
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        });
    }

    fn mem_read(path: &str) -> std::io::Result<String> {
        FILES.with(|f| {
            f.borrow()
                .get(path)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("no file {path}")))
        })
    }

    fn mem_loader() -> Loader {
        Loader {
            read_file: mem_read,
            ..Loader::default()
        }
    }

    fn errors(src: &str) -> Vec<String> {
        let (b, errs) = Loader::default().compile("-e", src, Stage::Core);
        errs.iter().map(|e| diag::format(e, &b.sources)).collect()
    }

    fn codes(errs: &[Diag]) -> Vec<&str> {
        errs.iter().map(|e| e.code.as_str()).collect()
    }

    fn first_msg(b: &Build, errs: &[Diag]) -> String {
        diag::format(&errs[0], &b.sources)
    }

    fn has_source(b: &Build, name: &str, text: Option<&str>) -> bool {
        (0..8).map(FileId).any(|id| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                b.sources.name(id) == name && text.is_none_or(|t| b.sources.text(id) == t)
            }))
            .unwrap_or(false)
        })
    }

    /// A module's name from its file is a name like any other: NFC, so
    /// a decomposed path spelling binds the composed name.
    #[test]
    fn module_names_from_paths_are_nfc() {
        set_files(&[("/p/mcafe\u{301}.fung", "x = 1")]);
        let (b, errs) = mem_loader().compile(
            "/p/main.fung",
            "import \"./mcafe\u{301}\"\nprint(mcaf\u{e9}.x)",
            Stage::All,
        );
        assert!(errs.is_empty(), "{:?}", codes(&errs));
        assert!(b.modules.iter().any(|m| m.name == "mcaf\u{e9}"));
    }

    #[test]
    fn default_reader_decodes_invalid_utf8() {
        let dir = std::env::temp_dir().join(format!("fung-loader-utf8-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.fung"), b"x = \"\xeb\x8b\"\n").unwrap();
        let entry = dir.join("main.fung").to_string_lossy().into_owned();
        let (b, errs) = Loader::default().compile(&entry, "import \"./m\"\nprint(m.x)", Stage::All);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(errs.is_empty(), "{errs:?}");
        assert!(b.modules.iter().any(|m| m.name == "m"));
        let text = "x = \"\u{fffd}\"\n";
        assert!((0..8).map(FileId).any(|id| {
            std::panic::catch_unwind(|| b.sources.text(id) == text).unwrap_or(false)
        }));
    }

    /// The REPL (08) compiles each input after the files the runtime
    /// already has, so every position names one file of the session; the
    /// REPL options reach the entry file only.
    #[test]
    fn compile_after_continues_the_file_ids_and_repl_options_reach_the_entry_only() {
        let mut prior = SourceMap::new();
        prior.add("a", "x");
        prior.add("b", "y");
        set_files(&[("/p/lib.fung", "fn g() = 1\nk = g()")]);
        let l = Loader {
            globals: vec!["g".into()],
            redefine: true,
            ..mem_loader()
        };
        let (b, errs) = l.compile_after(
            prior,
            "/p/main.fung",
            "import \"./lib\"\nfn g() = 2",
            Stage::All,
        );
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(b.sources.len(), 4);
        assert_eq!(
            (b.sources.name(FileId(0)), b.sources.text(FileId(1))),
            ("a", "y")
        );
        assert_eq!(b.sources.name(FileId(2)), "/p/main.fung");
        assert_eq!(b.sources.name(FileId(3)), "/p/lib.fung");
        let p = b.proto.unwrap();
        assert_eq!(p.global_names, ["g", "lib"]);
        assert!(p.pos.iter().all(|pos| pos.file == FileId(2)));
        // The library defines its own `g`: not the entry's globals.
        let lib = b.modules[0].proto.as_ref().unwrap();
        assert_eq!(lib.global_names, ["g", "k"]);
        let (_, errs) = l.compile_after(SourceMap::new(), "/p/main.fung", "x = )", Stage::All);
        assert_eq!(errs[0].pos.file, FileId(0));
    }

    // the whole front end

    #[test]
    fn compile_stops_at_requested_stage() {
        let l = Loader::default();
        let (b, errs) = l.compile("t.fung", "print(1)", Stage::Ast);
        assert!(errs.is_empty() && b.surface.is_some() && b.core.is_none() && b.proto.is_none());
        let (b, errs) = l.compile("t.fung", "print(1)", Stage::Core);
        assert!(errs.is_empty() && b.core.is_some() && b.proto.is_none());
        assert_eq!(b.core.unwrap().to_string(), "(do (call (prelude print) 1))");
        let (b, errs) = l.compile("t.fung", "print(1)", Stage::All);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(b.proto.unwrap().name, "<main>");
    }

    #[test]
    fn compile_returns_first_syntax_error_and_all_name_errors() {
        let l = Loader::default();
        let (_, errs) = l.compile("t.fung", "1 +", Stage::All);
        assert_eq!(codes(&errs), ["E0110"]);
        let (_, errs) = l.compile("t.fung", "print(a + b)", Stage::All);
        assert_eq!(codes(&errs), ["E0204", "E0204"]);
    }

    // loading imports

    #[test]
    fn load_graph_in_dependency_order() {
        let main = "import \"./lib/a\"\nimport \"./lib/b\" as bb\nprint(a.x, bb.y)";
        set_files(&[
            ("/p/main.fung", main),
            ("/p/lib/a.fung", "import \"../lib/b\"\nx = b.y + 1"),
            ("/p/lib/b.fung", "y = 1"),
        ]);
        let (b, errs) = mem_loader().compile("/p/main.fung", main, Stage::All);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(b.modules.len(), 2);
        assert_eq!(b.modules[0].key, "/p/lib/b.fung");
        assert_eq!(b.modules[1].key, "/p/lib/a.fung");
        assert!(b.proto.is_some());
        assert!(has_source(&b, "/p/lib/a.fung", None));
        assert_eq!(b.modules[1].proto.as_ref().unwrap().name, "<module a>");
    }

    #[test]
    fn load_stdlib_and_natives() {
        let l = mem_loader();
        set_files(&[]);
        let (b, errs) = l.compile("-e", "import fs\nprint(fs.read)", Stage::All);
        assert!(errs.is_empty(), "{errs:?}");
        let keys: Vec<&str> = b.modules.iter().map(|m| m.key.as_str()).collect();
        assert_eq!(keys[0], "_native.fs");
        assert!(b.modules[0].proto.is_none());
        assert_eq!(*keys.last().unwrap(), "fs");
        assert!(b.modules.last().unwrap().proto.as_ref().unwrap().builtin);

        let (_, errs) = l.compile("-e", "import _native.fs", Stage::All);
        assert_eq!(codes(&errs), ["E0310"]);
        let (_, errs) = l.compile("-e", "import prelude", Stage::All);
        assert_eq!(codes(&errs), ["E0301"], "prelude is not a module");

        let main = "import \"./b\"\nprint(b.y)";
        set_files(&[
            ("/p/a.fung", "x = 1"),
            (
                "/p/b.fung",
                "import \"./a\" (x)\nimport \"./a\" as am\ny = 1",
            ),
            ("/p/main.fung", main),
        ]);
        let (b, errs) = l.compile("/p/main.fung", main, Stage::All);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(
            b.modules[1].proto.as_ref().unwrap().exports,
            ["y"],
            "imported names are not re-exported (04)"
        );
        for src in ["import \"./b\"\nprint(b.x)", "import \"./b\"\nprint(b.am)"] {
            let (_, errs) = l.compile("/p/main.fung", src, Stage::All);
            assert_eq!(codes(&errs), ["E0303"], "{src:?}");
        }
    }

    #[test]
    fn load_errors() {
        set_files(&[
            ("/p/main.fung", "import \"./cyc_a\""),
            ("/p/cyc_a.fung", "import \"./cyc_b\""),
            ("/p/cyc_b.fung", "import \"./cyc_a\""),
            ("/p/self.fung", "import \"./self\""),
            ("/p/entry.fung", "import \"./uses_entry\""),
            ("/p/uses_entry.fung", "import \"./entry\""),
            ("/p/bad-name.fung", "x = 1"),
            ("/p/usebad.fung", "import \"./bad-name\""),
            (
                "/p/usebad2.fung",
                "import \"./bad-name\" as ok\nprint(ok.x)",
            ),
            ("/p/lib_err.fung", "print(1)"),
            ("/p/uselib.fung", "import \"./lib_err\""),
            ("/p/synerr.fung", "x = ("),
            ("/p/usesyn.fung", "import \"./synerr\""),
        ]);
        let l = mem_loader();
        let cases = [
            ("/p/main.fung", "E0302"),
            ("/p/self.fung", "E0302"),
            ("/p/entry.fung", "E0307"),
            ("/p/usebad.fung", "E0305"),
            ("/p/usebad2.fung", ""),
            ("/p/uselib.fung", "E0306"),
            ("/p/usesyn.fung", "E0110"),
        ];
        for (entry, want) in cases {
            let src = mem_read(entry).unwrap();
            let (_, errs) = l.compile(entry, &src, Stage::All);
            let got = errs.first().map(|e| e.code.as_str()).unwrap_or("");
            assert_eq!(got, want, "{entry}: {errs:?}");
        }
        let src = mem_read("/p/main.fung").unwrap();
        let (b, errs) = l.compile("/p/main.fung", &src, Stage::All);
        let msg = first_msg(&b, &errs);
        assert!(
            msg.contains("cyc_a.fung → /p/cyc_b.fung → /p/cyc_a.fung"),
            "{msg}"
        );
        let (_, errs) = l.compile("-e", "import missing", Stage::All);
        assert_eq!(codes(&errs), ["E0301"], "missing stdlib");
        let (_, errs) = l.compile("/p/x.fung", "import \"./nothing\"", Stage::All);
        assert_eq!(codes(&errs), ["E0301"], "missing file");
        let src = mem_read("/p/usesyn.fung").unwrap();
        let (b, errs) = l.compile("/p/usesyn.fung", &src, Stage::All);
        assert!(!errs.is_empty());
        assert!(
            has_source(&b, "/p/synerr.fung", Some("x = (")),
            "sources of failed loads are kept for error excerpts"
        );
    }

    #[test]
    fn loader_collects_tests() {
        let main = "import \"./lib\"\nfn g():\n  y = 1\n  2\n;;\ntest \"t\": assert lib.f() == 1 ;;\nprint(g())";
        set_files(&[
            ("/p/lib.fung", "import \"./util\"\nfn f() = 1"),
            ("/p/util.fung", "x = 1"),
            ("/p/main.fung", main),
        ]);
        let mut l = mem_loader();
        l.tests = true;
        let (b, errs) = l.compile("/p/main.fung", main, Stage::All);
        assert!(errs.is_empty(), "{errs:?}");
        assert!(
            disassemble(b.proto.as_ref().unwrap()).contains("TEST"),
            "tests kept"
        );
        l.tests = false;
        let (b, _) = l.compile("/p/main.fung", main, Stage::All);
        assert!(
            !disassemble(b.proto.as_ref().unwrap()).contains("TEST"),
            "tests dropped"
        );
        set_files(&[
            (
                "/p/lib.fung",
                "import \"./util\"\nfn f() = 1\ntest \"lib\": assert f() == 1 ;;",
            ),
            ("/p/util.fung", "x = 1"),
            ("/p/main.fung", main),
        ]);
        l.tests = true;
        let (b, errs) = l.compile("/p/main.fung", main, Stage::All);
        assert!(errs.is_empty(), "{errs:?}");
        for m in &b.modules {
            if let Some(p) = &m.proto {
                assert!(
                    !disassemble(p).contains("TEST"),
                    "{}: a library's tests run only when it is the entry (08)",
                    m.key
                );
            }
        }
    }

    #[test]
    fn cycle_message_shows_paths() {
        let main = "import \"./x/util\"";
        set_files(&[
            ("/p/main.fung", main),
            ("/p/x/util.fung", "import \"../y/util\""),
            ("/p/y/util.fung", "import \"../x/util\""),
        ]);
        let (b, errs) = mem_loader().compile("/p/main.fung", main, Stage::All);
        assert_eq!(codes(&errs), ["E0302"]);
        let msg = first_msg(&b, &errs);
        assert!(
            msg.contains("x/util.fung → ")
                && msg.contains("y/util.fung → ")
                && !msg.contains("util → util"),
            "{msg}"
        );
    }

    #[test]
    fn alias_collision_message() {
        let main = "import \"./lib/x\" as fs\nimport fs";
        set_files(&[("/p/lib/x.fung", "a = 1"), ("/p/main.fung", main)]);
        let (b, errs) = mem_loader().compile("/p/main.fung", main, Stage::All);
        assert_eq!(codes(&errs), ["E0309"]);
        assert!(first_msg(&b, &errs).contains("already bound to a module"));
    }

    // Brief examples

    #[test]
    fn module_member_and_missing_module() {
        assert_eq!(
            errors("import fs\nprint(fs.nope)"),
            vec![
                "error[E0303]: module `fs` has no member `nope`\n  --> -e:2:9\n  |\n2 | print(fs.nope)\n  |         ^^^^\n  |\n"
            ]
        );
        assert_eq!(
            errors("import nosuch"),
            vec![
                "error[E0301]: cannot find module `nosuch`\n  --> -e:1:1\n  |\n1 | import nosuch\n  | ^\n  |\n help: standard library modules are imported by name (`import fs`); your own files by path (`import \"./lib/x\"`)\n"
            ]
        );
    }

    /// The prelude names written in fung are the public names of the
    /// prelude files (`extras/prelude.fung`, `extras/agent.fung`).
    #[test]
    fn prelude_fung_exports_match_the_spec() {
        let loader = Loader {
            internal: true,
            ..Loader::default()
        };
        let mut exports = Vec::new();
        for (name, src) in crate::extras::PRELUDE_FILES {
            let file = crate::extras::path(name);
            let (b, errs) = loader.compile(&file, src, Stage::All);
            assert!(errs.is_empty(), "{file}: {errs:?}");
            exports.extend(b.proto.unwrap().exports);
        }
        exports.sort();
        assert_eq!(exports, crate::builtins::builtins().prelude_fung);
    }

    /// `extras/conc.fung` (stage 6a) is the module `conc`: it exports
    /// exactly the names the spec lists for it.
    #[test]
    fn conc_module_exports_match_the_list() {
        let (b, errs) = Loader::default().compile("-e", "import conc", Stage::All);
        assert!(errs.is_empty(), "{errs:?}");
        let conc = b.modules.iter().find(|m| m.key == "conc").expect("conc");
        let mut exports = conc.proto.as_ref().unwrap().exports.clone();
        exports.sort();
        assert_eq!(exports, crate::builtins::builtins().modules["conc"]);
        let (_, errs) = Loader::default().compile("-e", "print(pmap)", Stage::All);
        assert_eq!(codes(&errs), ["E0204"], "pmap is not a prelude name");
    }

    #[test]
    fn every_embedded_module_compiles() {
        for name in ["fs", "path", "cli", "time", "json", "regex", "http", "conc"] {
            let (_, errs) = Loader::default().compile("-e", &format!("import {name}"), Stage::All);
            assert!(errs.is_empty(), "{name}: {errs:?}");
        }
    }

    #[test]
    fn path_helpers() {
        assert_eq!(clean("/a/b/../c/./d//"), "/a/c/d");
        assert_eq!(clean("/.."), "/");
        assert_eq!(clean("a/../.."), "..");
        assert_eq!(rel("/a/b", "/a/b/c/d.fung"), "c/d.fung");
        assert_eq!(rel("/a/b", "/a/x.fung"), "../x.fung");
        assert_eq!(module_stem("/p/q/x.fung"), "x");
        assert!(is_name("x_1") && !is_name("bad-name") && !is_name("Up") && !is_name(""));
        // UAX #31: So and No are not name characters, Ll and Nd are.
        assert!(!is_name("mⓐ") && !is_name("x²") && is_name("ß1") && is_name("x٣"));
    }

    #[test]
    fn warnings_come_from_every_user_module_in_dependency_order() {
        // each module's warnings once it resolves,
        // so an import's come before its importer's; the embedded stdlib's
        // are hidden (users cannot act on them).
        let main = "import \"./lib\"\nimport fs\nfn f():\n  unused = 1\n  lib.v\n;;";
        set_files(&[("/p/lib.fung", "fn g():\n  dead = 1\n  2\n;;\nv = 1")]);
        let (b, errs) = mem_loader().compile("/p/main.fung", main, Stage::All);
        assert!(errs.is_empty(), "{errs:?}");
        let got: Vec<String> = b
            .warnings
            .iter()
            .map(|w| format!("{} {}:{}", w.code, b.sources.name(w.pos.file), w.pos.line))
            .collect();
        assert_eq!(
            got,
            [
                "W0102 /p/lib.fung:2",
                "W0101 /p/main.fung:2",
                "W0102 /p/main.fung:4"
            ]
        );
        let (b, errs) = mem_loader().compile("-e", "fn f():\n  x = 1\n  y\n;;", Stage::All);
        assert_eq!(codes(&errs), ["E0204"]);
        assert!(b.warnings.is_empty(), "a file with errors has no warnings");
    }
}
