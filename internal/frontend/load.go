package frontend

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/compiler"
	"github.com/tunarider/fung/internal/desugar"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/resolve"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

// Loaded is one module of a program: a compiled fung file or a native
// module, under the key the IMPORT instruction uses at run time.
type Loaded struct {
	Key    string
	Name   string
	Proto  *bytecode.Proto
	Native *value.Module
}

// Program is a compiled entry file with every module it reaches, in
// dependency order (a module comes after what it imports). Sources maps
// each file name used in positions to its text, for error excerpts.
type Program struct {
	Entry    *bytecode.Proto
	Modules  []Loaded
	Sources  map[string]string
	Warnings []*diag.Error // W01xx from every module, dependency order; check and test show them (08)
}

// Loader compiles an entry file and its whole import graph (04: 적재
// 시점): every module is read and compiled up front, so cycles, private
// access, top-level restrictions and unknown names are compile errors.
type Loader struct {
	Prelude  map[string]value.Value
	Natives  map[string]*value.Module          // "_native.fs" → module
	Stdlib   func(name string) (string, bool)  // fung source of a stdlib module by name
	ReadFile func(path string) (string, error) // os.ReadFile by default
	Tests    bool                              // keep test blocks (fung test / check), see 08
	Resolve  resolve.Options                   // REPL: Globals and Redefine for the entry file
	Internal bool                              // the entry is an embedded extras file (prelude.fung): may import _native.*
}

type loader struct {
	*Loader
	entryKey string
	done     map[string]*Loaded
	loading  map[string]bool
	stack    []stackEntry // the DFS path, for E0302 messages
	order    []Loaded
	sources  map[string]string
	warnings []*diag.Error
}

// stackEntry is one module on the loading path: its key and the path
// shown in messages.
type stackEntry struct{ key, file string }

// Load compiles entryFile (its text given as src; "-e" for inline code)
// and everything it imports. Sources is filled even when errors are
// returned, so the caller can excerpt them.
func (l *Loader) Load(entryFile, src string) (Program, []*diag.Error) {
	b, errs := l.load(entryFile, src, StageAll)
	return b.Program, errs
}

// LoadBuild is Load keeping the entry's surface and core trees too.
func (l *Loader) LoadBuild(entryFile, src string) (Build, []*diag.Error) {
	return l.load(entryFile, src, StageAll)
}

func (l *Loader) load(entryFile, src string, upTo Stage) (Build, []*diag.Error) {
	ld := &loader{Loader: l, done: map[string]*Loaded{}, loading: map[string]bool{}, sources: map[string]string{}}
	if ld.ReadFile == nil {
		ld.ReadFile = func(path string) (string, error) {
			b, err := os.ReadFile(path)
			return string(b), err
		}
	}
	if ld.Stdlib == nil {
		ld.Stdlib = func(string) (string, bool) { return "", false }
	}
	key := entryFile
	if entryFile != "-e" {
		key = absKey(entryFile)
	}
	ld.entryKey = key
	ld.loading[key] = true
	ld.stack = append(ld.stack, stackEntry{key: key, file: entryFile})
	ld.sources[entryFile] = src
	b, errs := ld.compileFile(key, entryFile, src, false, l.Internal, upTo)
	b.Program = Program{Entry: b.Proto, Modules: ld.order, Sources: ld.sources, Warnings: ld.warnings}
	return b, errs
}

// compileFile parses one file, loads what it imports (depth first), then
// resolves and compiles it with the exports of its imports in hand.
func (ld *loader) compileFile(key, file, src string, library, isStdlib bool, upTo Stage) (Build, []*diag.Error) {
	var b Build
	toks, _, err := lexer.Lex(file, src)
	if err != nil {
		return b, []*diag.Error{err}
	}
	surface, perr := parser.Parse(toks)
	if perr != nil {
		return b, []*diag.Error{perr}
	}
	b.Surface = surface
	if upTo == StageAST {
		return b, nil
	}
	imports := map[string]resolve.ModuleInfo{}
	for _, k := range surface.Kids {
		if k.Head != "import" {
			continue // nested imports are E0311 in resolve
		}
		im := k.Lit.(*tree.Import)
		info, errs := ld.loadImport(im, k.Pos, key, isStdlib)
		if len(errs) > 0 {
			return b, errs
		}
		imports[im.Spec] = info
	}
	core, derr := desugar.Desugar(surface)
	if derr != nil {
		return b, []*diag.Error{derr}
	}
	ropts := resolve.Options{Imports: imports, Library: library}
	if key == ld.entryKey {
		ropts.Globals, ropts.Redefine = ld.Resolve.Globals, ld.Resolve.Redefine
	}
	res, rerrs := resolve.Resolve(core, preludeNames(ld.Prelude), ropts)
	if len(rerrs) > 0 {
		return b, rerrs
	}
	b.Core = res.Core
	if !isStdlib {
		ld.warnings = append(ld.warnings, res.Warnings...) // users cannot act on stdlib warnings
	}
	if upTo == StageCore {
		return b, nil
	}
	// A library's tests run only when it is the entry (08).
	proto, cerr := compiler.Compile(res, ld.Prelude, compiler.Options{Source: src, Tests: ld.Tests && key == ld.entryKey})
	if cerr != nil {
		return b, []*diag.Error{cerr}
	}
	if isStdlib {
		markBuiltin(proto)
	}
	b.Proto = proto
	return b, nil
}

// markBuiltin flags a proto tree as compiled from an embedded extras
// file, so runtime errors inside it point at the user's call (06).
func markBuiltin(p *bytecode.Proto) {
	p.Builtin = true
	for _, child := range p.Protos {
		markBuiltin(child)
	}
}

// loadImport resolves one import spec to a module key, loads it if it
// is new, and returns what resolve needs. Cycles (E0302), the entry file
// (E0307), internal modules from user code (E0310), bad file names
// (E0305) and missing modules (E0301) are reported at the import.
func (ld *loader) loadImport(im *tree.Import, pos diag.Pos, fromKey string, fromStdlib bool) (resolve.ModuleInfo, []*diag.Error) {
	switch {
	case im.IsPath && (strings.HasPrefix(im.Spec, "./") || strings.HasPrefix(im.Spec, "../") || strings.HasPrefix(im.Spec, "/")):
		return ld.loadFile(im, pos, fromKey)
	case im.IsPath:
		// Bare path strings are reserved for packages (04); none exist yet.
		return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0301", pos, im.Spec)}
	case strings.HasPrefix(im.Spec, "_native."):
		if !fromStdlib {
			public := strings.TrimPrefix(im.Spec, "_native.")
			return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0310", pos, im.Spec, public)}
		}
		m, ok := ld.Natives[im.Spec]
		if !ok {
			return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0301", pos, im.Spec)}
		}
		if ld.done[im.Spec] == nil {
			l := &Loaded{Key: im.Spec, Name: m.Name, Native: m}
			ld.done[im.Spec] = l
			ld.order = append(ld.order, *l)
		}
		return resolve.ModuleInfo{Key: im.Spec, Name: m.Name, Exports: m.Names()}, nil
	default:
		// prelude.fung is bootstrapped into the prelude table, not a module.
		src, ok := ld.Stdlib(im.Spec)
		if !ok || im.Spec == "prelude" {
			return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0301", pos, im.Spec)}
		}
		return ld.loadModule(im.Spec, im.Spec, "extras/"+im.Spec+".fung", pos, true, func() (string, error) { return src, nil })
	}
}

// loadFile resolves a relative or absolute path against the importing
// file's directory (04: import하는 파일의 위치 기준), or the working
// directory for -e.
func (ld *loader) loadFile(im *tree.Import, pos diag.Pos, fromKey string) (resolve.ModuleInfo, []*diag.Error) {
	path := im.Spec
	if !strings.HasSuffix(path, ".fung") {
		path += ".fung"
	}
	var key string
	if filepath.IsAbs(path) {
		key = filepath.Clean(path)
	} else {
		dir := "."
		if filepath.IsAbs(fromKey) {
			dir = filepath.Dir(fromKey)
		}
		key = absKey(filepath.Join(dir, path))
	}
	if key == ld.entryKey && fromKey != key {
		return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0307", pos)}
	}
	name := moduleStem(key)
	if !value.IsName(name) && im.Alias == "" {
		return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0305", pos, name)}
	}
	return ld.loadModule(key, name, displayPath(key), pos, false, func() (string, error) { return ld.ReadFile(key) })
}

// loadModule compiles a module once (library rules apply) and records it
// in dependency order. Re-entering a module that is still loading is a
// cycle; the message shows the whole path.
func (ld *loader) loadModule(key, name, file string, pos diag.Pos, isStdlib bool, read func() (string, error)) (resolve.ModuleInfo, []*diag.Error) {
	if l, ok := ld.done[key]; ok {
		return resolve.ModuleInfo{Key: key, Name: name, Exports: exportsOf(l.Proto)}, nil
	}
	if ld.loading[key] {
		var path []string
		for i, e := range ld.stack {
			if e.key == key {
				for _, f := range ld.stack[i:] {
					path = append(path, f.file)
				}
				break
			}
		}
		path = append(path, file)
		return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0302", pos, strings.Join(path, " → "))}
	}
	src, err := read()
	if err != nil {
		return resolve.ModuleInfo{}, []*diag.Error{diag.Errorf("E0301", pos, key)}
	}
	ld.sources[file] = src
	ld.loading[key] = true
	ld.stack = append(ld.stack, stackEntry{key: key, file: file})
	b, errs := ld.compileFile(key, file, src, true, isStdlib, StageAll)
	ld.stack = ld.stack[:len(ld.stack)-1]
	delete(ld.loading, key)
	if len(errs) > 0 {
		return resolve.ModuleInfo{}, errs
	}
	b.Proto.Name = "<module " + name + ">"
	l := &Loaded{Key: key, Name: name, Proto: b.Proto}
	ld.done[key] = l
	ld.order = append(ld.order, *l)
	return resolve.ModuleInfo{Key: key, Name: name, Exports: exportsOf(b.Proto)}, nil
}

// exportsOf lists a module's public globals as resolve decided them
// (04: private and import-bound names are not exported). The VM builds
// the module value from the same list.
func exportsOf(p *bytecode.Proto) []string { return p.Exports }

func absKey(path string) string {
	if abs, err := filepath.Abs(path); err == nil {
		return abs
	}
	return filepath.Clean(path)
}

// displayPath is how an imported file appears in positions: relative to
// the working directory when it lies below it, absolute otherwise.
func displayPath(key string) string {
	if wd, err := os.Getwd(); err == nil {
		if rel, err := filepath.Rel(wd, key); err == nil && !strings.HasPrefix(rel, "..") {
			return rel
		}
	}
	return key
}

func moduleStem(key string) string {
	return strings.TrimSuffix(filepath.Base(key), ".fung")
}
