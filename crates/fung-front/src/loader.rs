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
use crate::resolve::{self, ModuleInfo, Sig};
use crate::source::{FileId, Pos, SourceMap, decode_source};
use crate::tree::{Head, Import, Lit, Node};

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
    /// The parameters of its exported functions (D43).
    pub sigs: BTreeMap<String, Sig>,
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
    /// The parameters of the entry's exported functions (D43).
    pub sigs: BTreeMap<String, Sig>,
}

/// Compiles an entry file and everything it imports.
pub struct Loader {
    /// Reads an imported file. By default it reads the bytes, which must
    /// be UTF-8 (`decode_source`, L4).
    pub read_file: fn(&str) -> std::io::Result<String>,
    /// Which file a path names, when the path's text cannot tell: by
    /// default the (device, inode) of the file it leads to, so a symbolic
    /// link, other letter case on a file system that ignores it, or a
    /// name in another Unicode form is the same module (04, FE5). `None`
    /// (no such file) leaves the path itself as the identity.
    pub file_id: fn(&str) -> Option<String>,
    /// Keep test blocks of the entry (`fung test` / `check`), see 08.
    pub tests: bool,
    /// The entry is an embedded extras file: it may import `_native.*`.
    pub internal: bool,
    /// The entry is compiled as an imported file is (a prelude file: its
    /// public names are the prelude's, its interface, D38).
    pub library: bool,
    /// Names added to the prelude (VM tests use it for natives that exist only in tests).
    pub extra_prelude: Vec<String>,
    /// REPL (08): the globals earlier inputs defined, for the entry file
    /// only.
    pub globals: Vec<String>,
    /// REPL: the entry file may define fns and imports again.
    pub redefine: bool,
    /// The entry's last expression is its value (the REPL prints it, VM
    /// tests read it). Otherwise, and always for an imported file, that
    /// place is discarded like a statement (01: 버려지는 자리).
    pub keep_value: bool,
    /// The built-in names to compile against; `None` (the default) is
    /// `spec/builtins.txt`. The generator of that file sets its own, built
    /// from the runtime's tables, so a new `_native.*` bridge or prelude
    /// native needs no hand edit of the file first.
    pub spec: Option<&'static Builtins>,
}

impl Default for Loader {
    fn default() -> Loader {
        Loader {
            read_file: |p| {
                std::fs::read(p).and_then(|b| {
                    decode_source(b)
                        .map_err(|why| std::io::Error::new(std::io::ErrorKind::InvalidData, why))
                })
            },
            file_id: |p| {
                use std::os::unix::fs::MetadataExt;
                std::fs::metadata(p)
                    .ok()
                    .map(|m| format!("{}:{}", m.dev(), m.ino()))
            },
            tests: false,
            internal: false,
            library: false,
            extra_prelude: Vec::new(),
            globals: Vec::new(),
            redefine: false,
            keep_value: false,
            spec: None,
        }
    }
}

struct State<'a> {
    l: &'a Loader,
    entry_key: String,
    /// The key of each file loaded so far, by its identity
    /// (`Loader::file_id`): the first path that reached it.
    keys: HashMap<String, String>,
    done: HashMap<String, usize>,
    loading: HashSet<String>,
    /// The DFS path (key, shown path), for E0302 messages.
    stack: Vec<(String, String)>,
    modules: Vec<Loaded>,
    sources: SourceMap,
    prelude: Vec<String>,
    prelude_set: BTreeSet<String>,
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
        let mut keys = HashMap::new();
        if file != "-e" {
            keys.insert(
                (self.file_id)(&key).unwrap_or_else(|| key.clone()),
                key.clone(),
            );
        }
        let mut st = State {
            l: self,
            entry_key: key.clone(),
            keys,
            done: HashMap::new(),
            loading: HashSet::new(),
            stack: vec![(key.clone(), file.to_string())],
            modules: Vec::new(),
            sources: prior,
            prelude_set: prelude.iter().cloned().collect(),
            prelude,
        };
        st.loading.insert(key.clone());
        let id = st.sources.add(file, src);
        let (mut b, errs) =
            st.compile_file(&key, file, id, src, self.library, self.internal, up_to);
        b.sources = st.sources;
        b.modules = st.modules;
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
        // The trees are kept only for `dump`, and the surface for the
        // REPL, which prints the value of an input that ends in an
        // expression (FE39).
        if up_to == Stage::Ast {
            b.surface = Some(surface);
            return (b, Vec::new());
        }
        if self.l.keep_value && key == self.entry_key {
            b.surface = Some(surface.clone());
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
        let keep_value = self.l.keep_value && key == self.entry_key;
        let core = match desugar(&surface, keep_value) {
            Ok(c) => c,
            Err(e) => return (b, vec![e]),
        };
        let mut ropts = resolve::Options {
            imports,
            library,
            spec: Some(self.l.builtins()),
            ..resolve::Options::default()
        };
        if key == self.entry_key {
            ropts.globals.clone_from(&self.l.globals);
            ropts.redefine = self.l.redefine;
            ropts.keep_value = keep_value;
        }
        let mut res = match resolve::resolve(&core, &self.prelude, &ropts) {
            Ok(r) => r,
            Err(errs) => return (b, errs),
        };
        b.sigs = std::mem::take(&mut res.sigs);
        if up_to == Stage::Core {
            b.core = Some(res.core);
            return (b, Vec::new());
        }
        // A library's tests run only when it is the entry (08).
        let copts = compiler::Options {
            source: src,
            file,
            tests: self.l.tests && key == self.entry_key,
            keep_value,
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
                // What wraps the bridge: its module, or the prelude.
                let hint = match public {
                    "net" => "import `http` instead".to_string(),
                    "process" | "range" => "its functions are in the prelude".to_string(),
                    "arg" => "it checks the arguments of the built-in modules".to_string(),
                    m => format!("import `{m}` instead"),
                };
                return Err(e("E0310", vec![s(spec), s(&hint)]));
            }
            let Some(names) = self.l.builtins().modules.get(spec) else {
                return Err(e("E0301", vec![s(spec)]));
            };
            let sigs: BTreeMap<String, Sig> = names
                .iter()
                .filter_map(|n| {
                    let list = self.l.builtins().module_signature(spec, n)?;
                    let pure = self.l.builtins().module_pure(spec, n);
                    Some((n.clone(), Sig::parse_marked(list, pure)))
                })
                .collect();
            if !self.done.contains_key(spec) {
                self.done.insert(spec.to_string(), self.modules.len());
                self.modules.push(Loaded {
                    key: spec.to_string(),
                    name: public.to_string(),
                    proto: None,
                    sigs: sigs.clone(),
                });
            }
            return Ok(ModuleInfo {
                key: spec.to_string(),
                name: public.to_string(),
                exports: names.clone(),
                sigs,
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
        // The binding is a name, so it is NFC like the names that use it;
        // it comes from the path as written here.
        let name = crate::unicode::nfc(&module_stem(&key)).into_owned();
        // One file is one module, whatever path reaches it (FE5): a file
        // seen before keeps the key it was first loaded under.
        let id = (self.l.file_id)(&key).unwrap_or_else(|| key.clone());
        let key = self.keys.entry(id).or_insert(key).clone();
        if key == self.entry_key && from_key != key {
            return Err(vec![error("E0307", pos, vec![])]);
        }
        if !crate::lexer::is_name(&name) && im.alias.is_empty() && im.names.is_none() {
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
            let m = &self.modules[i];
            let exports = m
                .proto
                .as_ref()
                .map(|p| p.exports.clone())
                .unwrap_or_default();
            return Ok(ModuleInfo {
                key: key.to_string(),
                name: name.to_string(),
                exports,
                sigs: m.sigs.clone(),
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
        let src = match read() {
            Ok(src) => src,
            Err(e) => return Err(vec![read_error(&e, file, pos)]),
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
            sigs: b.sigs.clone(),
        });
        Ok(ModuleInfo {
            key: key.to_string(),
            name: name.to_string(),
            exports,
            sigs: b.sigs,
        })
    }
}

/// Why a module's file could not be read, its path shown as positions
/// show it (FE29): E0301 when there is no such file, else E0312 with the
/// reason. Text that is not UTF-8 (L4) gets the help that fung source is.
fn read_error(e: &std::io::Error, file: &str, pos: Pos) -> Diag {
    use std::io::ErrorKind as K;
    let path = Arg::Str(file.to_string());
    match e.kind() {
        K::NotFound => error("E0301", pos, vec![path]),
        K::InvalidData => error("E0312", pos, vec![path, Arg::Str(e.to_string())])
            .with_help("fung source is UTF-8 text"),
        _ => error("E0312", pos, vec![path, Arg::Str(crate::oserr::reason(e))]),
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
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
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

    /// Whether the build kept a file `name` (with `text`, if given).
    fn has_source(b: &Build, name: &str, text: Option<&str>) -> bool {
        (0..b.sources.len() as u32)
            .map(FileId)
            .any(|id| b.sources.name(id) == name && text.is_none_or(|t| b.sources.text(id) == t))
    }

    /// A scratch directory removed when the test ends, failed or not.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            let dir =
                std::env::temp_dir().join(format!("fung-loader-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
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

    /// A module that is not UTF-8 is refused where it is imported (L4),
    /// not read with U+FFFD in it.
    #[test]
    fn default_reader_refuses_invalid_utf8() {
        let dir = Scratch::new("utf8");
        std::fs::write(dir.0.join("m.fung"), b"x = \"\xeb\x8b\"\n").unwrap();
        let entry = dir.0.join("main.fung").to_string_lossy().into_owned();
        let (_, errs) = Loader::default().compile(&entry, "import \"./m\"\nprint(m.x)", Stage::All);
        assert_eq!(codes(&errs), ["E0312"]);
        let msg = crate::diag::message(&errs[0].code, &errs[0].args);
        assert!(
            msg.ends_with("m.fung`: not valid UTF-8 at line 1, column 6"),
            "{msg}"
        );
    }

    /// A module is one file, however a path names it (04, FE5): through
    /// a symbolic link, or with other letter case on a file system that
    /// ignores it, it is loaded once and shares its state. Each import
    /// keeps the name its own path gives.
    #[test]
    fn one_file_under_several_paths_is_one_module() {
        let dir = Scratch::new("same-file");
        std::fs::write(dir.0.join("lib.fung"), "n = 1\n").unwrap();
        std::os::unix::fs::symlink("lib.fung", dir.0.join("lib2.fung")).unwrap();
        let folds_case = dir.0.join("LIB.fung").exists();
        let entry = dir.0.join("main.fung").to_string_lossy().into_owned();
        let mut src = String::from("import \"./lib\"\nimport \"./lib2\"\n");
        if folds_case {
            src.push_str("import \"./Lib\" as _l3\n");
        }
        src.push_str("print(lib.n, lib2.n)\n");
        let (b, errs) = Loader::default().compile(&entry, &src, Stage::All);
        assert!(errs.is_empty(), "{:?}", codes(&errs));
        let users: Vec<_> = b
            .modules
            .iter()
            .filter(|m| m.key.starts_with('/'))
            .collect();
        assert_eq!(
            users.len(),
            1,
            "{:?}",
            b.modules.iter().map(|m| &m.key).collect::<Vec<_>>()
        );
        let p = b.proto.unwrap();
        assert!(
            p.global_names.iter().any(|g| g == "lib2"),
            "{:?}",
            p.global_names
        );
        // The entry file through a link is still the entry (E0307).
        std::fs::write(dir.0.join("main.fung"), &src).unwrap();
        std::os::unix::fs::symlink("main.fung", dir.0.join("again.fung")).unwrap();
        std::fs::write(dir.0.join("other.fung"), "import \"./again\"\n").unwrap();
        let (_, errs) =
            Loader::default().compile(&entry, "import \"./other\"\nprint(other)", Stage::All);
        assert_eq!(codes(&errs), ["E0307"]);
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
            keep_value: true,
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
        // the hint names what wraps the bridge, not a module that is not there
        for (spec, want) in [
            (
                "_native.fs",
                "`_native.fs` is internal; import `fs` instead",
            ),
            (
                "_native.net",
                "`_native.net` is internal; import `http` instead",
            ),
            (
                "_native.process",
                "`_native.process` is internal; its functions are in the prelude",
            ),
            (
                "_native.range",
                "`_native.range` is internal; its functions are in the prelude",
            ),
        ] {
            let (_, errs) = l.compile("-e", &format!("import {spec}"), Stage::All);
            let shown = crate::diag::message(&errs[0].code, &errs[0].args);
            assert_eq!(shown, want, "{spec}");
        }
        let (_, errs) = l.compile("-e", "import prelude", Stage::All);
        assert_eq!(codes(&errs), ["E0301"], "prelude is not a module");

        let main = "import \"./b\"\nprint(b.y)";
        set_files(&[
            ("/p/a.fung", "x = 1"),
            (
                "/p/b.fung",
                "import \"./a\" (x)\nimport \"./a\" as am\ny = x + am.x",
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

    /// FE29: only a missing file is "cannot find"; one that is there but
    /// cannot be read says why. Both show the path the way positions do.
    #[test]
    fn unreadable_module_says_why() {
        let dir = Scratch::new("unreadable");
        std::fs::create_dir(dir.0.join("dir.fung")).unwrap();
        let noperm = dir.0.join("noperm.fung");
        std::fs::write(&noperm, "x = 1\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&noperm, std::fs::Permissions::from_mode(0o000)).unwrap();
        let root_reads_anyway = std::fs::read(&noperm).is_ok();
        let entry = dir.0.join("main.fung").to_string_lossy().into_owned();
        let msg = |src: &str| {
            let (_, errs) = Loader::default().compile(&entry, src, Stage::All);
            assert_eq!(errs.len(), 1, "{src}: {errs:?}");
            format!(
                "{} {}",
                errs[0].code,
                diag::message(&errs[0].code, &errs[0].args)
            )
        };
        let shown = display_path(&dir.0.to_string_lossy());
        assert_eq!(
            msg("import \"./dir\"\nprint(dir.x)"),
            format!("E0312 cannot read module `{shown}/dir.fung`: is a directory")
        );
        if !root_reads_anyway {
            assert_eq!(
                msg("import \"./noperm\"\nprint(noperm.x)"),
                format!("E0312 cannot read module `{shown}/noperm.fung`: permission denied")
            );
        }
        let (_, errs) =
            Loader::default().compile(&entry, "import \"./dir\"\nprint(dir.x)", Stage::All);
        assert_eq!(
            errs[0].help, None,
            "the UTF-8 help is for text that is not UTF-8"
        );
        assert_eq!(
            msg("import \"./nothing\"\nprint(nothing.x)"),
            format!("E0301 cannot find module `{shown}/nothing.fung`")
        );
        // below the working directory, the path is relative like positions
        let (b, errs) = Loader::default().compile("-e", "import \"./no_such_mod\"", Stage::All);
        assert!(
            first_msg(&b, &errs).starts_with("error[E0301]: cannot find module `no_such_mod.fung`"),
            "{}",
            first_msg(&b, &errs)
        );
    }

    /// FE29: E0306 points at the start of the statement, not at a call's
    /// `(`.
    #[test]
    fn library_statement_error_points_at_the_statement() {
        set_files(&[("/p/lib_side.fung", "x = 1\nprint(\"side\")\n(print)(x)\n")]);
        let (_, errs) = mem_loader().compile("/p/m.fung", "import \"./lib_side\"", Stage::All);
        let at: Vec<(&str, u32, u32)> = errs
            .iter()
            .map(|e| (e.code.as_str(), e.pos.line, e.pos.col))
            .collect();
        assert_eq!(at, [("E0306", 2, 1), ("E0306", 3, 2)]);
    }

    /// FE30: a module name from a file name follows the lexer's rule for
    /// a name: a reserved word or a tag needs `as`.
    #[test]
    fn module_name_from_a_file_is_a_name_the_lexer_reads() {
        set_files(&[
            ("/p/if.fung", "x = 1"),
            ("/p/Big.fung", "x = 1"),
            ("/p/nil.fung", "x = 1"),
            ("/p/ok_1.fung", "x = 1"),
        ]);
        for (src, want) in [
            ("import \"./if\"", vec!["E0305"]),
            ("import \"./nil\"", vec!["E0305"]),
            ("import \"./Big\"", vec!["E0305"]),
            ("import \"./if\" as m\nprint(m.x)", vec![]),
            ("import \"./ok_1\"\nprint(ok_1.x)", vec![]),
        ] {
            let (_, errs) = mem_loader().compile("/p/main.fung", src, Stage::All);
            assert_eq!(codes(&errs), want, "{src}");
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
        let main = "import \"./lib\"\nfn g():\n  _y = 1\n  2\n;;\ntest \"t\": assert lib.f() == 1 ;;\nprint(g())";
        set_files(&[
            ("/p/lib.fung", "import \"./util\"\nfn f() = util.x"),
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
                "import \"./util\"\nfn f() = util.x\ntest \"lib\": assert f() == 1 ;;",
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
            library: true,
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
        let (b, errs) = Loader::default().compile("-e", "import conc\nprint(conc)", Stage::All);
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
            let (_, errs) = Loader::default().compile(
                "-e",
                &format!("import {name}\nprint({name})"),
                Stage::All,
            );
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
        use crate::lexer::is_name;
        assert!(is_name("x_1") && !is_name("bad-name") && !is_name("Up") && !is_name(""));
        assert!(is_name("_") && !is_name("if") && !is_name("true") && is_name("iff"));
        // UAX #31: So and No are not name characters, Ll and Nd are.
        assert!(!is_name("mⓐ") && !is_name("x²") && is_name("ß1") && is_name("x٣"));
    }

    #[test]
    fn an_imported_files_unused_names_are_errors_too() {
        // D38: the same rules for every file, the embedded stdlib's too;
        // an imported file's public names are its interface.
        let main = "import \"./lib\"\nprint(lib.v)";
        set_files(&[("/p/lib.fung", "fn g():\n  dead = 1\n  2\n;;\nv = 1")]);
        let (b, errs) = mem_loader().compile("/p/main.fung", main, Stage::All);
        let got: Vec<String> = errs
            .iter()
            .map(|e| format!("{} {}:{}", e.code, b.sources.name(e.pos.file), e.pos.line))
            .collect();
        assert_eq!(got, ["E0239 /p/lib.fung:2"]);
    }
}
