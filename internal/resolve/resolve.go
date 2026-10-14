// Package resolve classifies every name in a core tree and reports scope
// errors (09): block locals become (local n), captured values (captured n),
// a named local fn's own name (self), file globals (global n), prelude
// names (prelude name). It walks once, rewrites name leaves into what the
// compiler needs, and collects every error (08: 의미 에러는 모두).
package resolve

import (
	"fmt"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

// Result is the resolved tree plus what the compiler needs to lay out
// storage: how many block-local slots the main frame needs and the names
// of the file's globals, in index order.
type Result struct {
	Core      *tree.Node
	NumLocals int
	Globals   []string
	Imports   []string      // module keys this file imports, in order
	Exports   []string      // public globals: not private, not hidden, not bound by import (04)
	Warnings  []*diag.Error // W01xx, in the order found
}

type resolver struct {
	prelude     map[string]bool
	preludeList []string
	fn          *fnScope // the function being resolved
	globals     []string
	// fileVarPos records where each file-level variable is assigned, only
	// to explain why a function cannot see it (03: functions copy the
	// variables visible when they are created; top-level fns are created
	// before any variable exists).
	fileVarPos  map[string]diag.Pos
	aliases     map[string][]string
	aliasPos    map[string]diag.Pos
	reported    map[string]bool // type names already reported, to avoid repeats
	opts        Options
	imports     []string
	moduleNames map[string]*ModuleInfo // names bound to modules (unshadowable)
	importBound map[string]bool        // every name an import bound: never re-exported (04)
	errs        []*diag.Error
	warnings    []*diag.Error
}

// Resolve resolves a core tree against the prelude names and the modules
// it may import. The input tree is not modified.
func Resolve(core *tree.Node, prelude []string, opts Options) (Result, []*diag.Error) {
	r := &resolver{
		prelude: map[string]bool{}, preludeList: prelude, fileVarPos: map[string]diag.Pos{},
		aliases: map[string][]string{}, aliasPos: map[string]diag.Pos{}, reported: map[string]bool{},
		opts: opts, moduleNames: map[string]*ModuleInfo{}, importBound: map[string]bool{},
	}
	for _, p := range prelude {
		r.prelude[p] = true
	}
	root := newFn(nil, "", true)
	r.fn = root
	out := r.rootBlock(core)
	sortWarnings(r.warnings)
	var exports []string
	for _, g := range r.globals {
		if !strings.HasPrefix(g, "_") && !strings.HasPrefix(g, "$") && !r.importBound[g] {
			exports = append(exports, g)
		}
	}
	return Result{Core: out, NumLocals: root.maxSlot, Globals: r.globals, Imports: r.imports, Exports: exports, Warnings: r.warnings}, r.errs
}

func newFn(parent *fnScope, self string, file bool) *fnScope {
	return &fnScope{parent: parent, self: self, file: file, capIndex: map[string]int{}, closed: map[string]bool{}}
}

// fnStmt reports whether a node is a named fn definition statement.
func fnStmt(n *tree.Node) (*tree.FnInfo, bool) {
	if n.Head != "fn" {
		return nil, false
	}
	info := n.Lit.(*tree.FnInfo)
	return info, info.Name != ""
}

// rootBlock resolves the file scope. Imports and top-level fn statements
// are bound first, in source order, as constant globals and emitted first
// (03: 최상위 fn은 순서와 무관하게 서로 참조; 04: import는 최상위), then
// the remaining statements in order. In a library file those statements
// may only be assignments (04, E0306).
func (r *resolver) rootBlock(n *tree.Node) *tree.Node {
	sc := r.pushBlock(n, true)
	// REPL: the globals of earlier lines keep their indexes (08).
	for i, g := range r.opts.Globals {
		sc.names[g] = binding{global: true, index: i}
		sc.used[g] = true
		r.globals = append(r.globals, g)
	}
	r.declareAliases(n)
	var imports, fns, rest []*tree.Node
	for _, k := range n.Kids {
		if k.Head == "typealias" {
			continue // consumed by declareAliases
		}
		if k.Head == "import" {
			imports = append(imports, r.importStmt(k, sc))
			continue
		}
		info, ok := fnStmt(k)
		if !ok {
			rest = append(rest, k)
			continue
		}
		if b, dup := sc.names[info.Name]; dup {
			if r.opts.Redefine && b.module == nil {
				sc.names[info.Name] = binding{global: true, index: b.index, constant: true}
				fns = append(fns, k)
				continue
			}
			if b.module != nil {
				r.errs = append(r.errs, shadowError(info.Name, k.Pos))
			} else {
				r.errs = append(r.errs, diag.Errorf("E0210", k.Pos, info.Name).Span(runeLen(info.Name)))
			}
			continue
		}
		r.warnShadow(info.Name, k.Pos)
		sc.names[info.Name] = binding{global: true, index: len(r.globals), constant: true}
		sc.pos[info.Name] = k.Pos
		r.globals = append(r.globals, info.Name)
		fns = append(fns, k)
	}
	for _, k := range rest {
		if k.Head == "let" && k.Kids[0].Head == "name" {
			name := k.Kids[0].Lit.(string)
			if _, ok := r.fileVarPos[name]; !ok {
				r.fileVarPos[name] = k.Kids[0].Pos
			}
		}
	}
	out := &tree.Node{Head: "do", Pos: n.Pos}
	out.Kids = append(out.Kids, imports...)
	for _, k := range fns {
		b := sc.names[k.Lit.(*tree.FnInfo).Name]
		out.Kids = append(out.Kids, tree.New("let", k.Pos, ref(k.Pos, b), r.function(k)))
	}
	for _, k := range rest {
		if r.opts.Library && k.Head != "let" && k.Head != "test" {
			r.errs = append(r.errs, diag.Errorf("E0306", k.Pos))
			continue
		}
		out.Kids = append(out.Kids, r.walk(k))
	}
	r.warnUnusedImports(sc)
	r.popBlock(true)
	return out
}

// pushBlock opens a block scope in the current function and notes which
// local fns the block declares later (for E0208).
func (r *resolver) pushBlock(n *tree.Node, file bool) *scope {
	sc := newScope(file, r.fn.nextSlot)
	r.fn.blocks = append(r.fn.blocks, sc)
	later := map[string]bool{}
	if !file {
		for _, k := range n.Kids {
			if info, ok := fnStmt(k); ok {
				later[info.Name] = true
			}
		}
	}
	r.fn.later = append(r.fn.later, later)
	return sc
}

// popBlock closes a block: its names become "closed" for E0206 and its
// slots are released for reuse (03).
func (r *resolver) popBlock(file bool) {
	f := r.fn
	sc := f.blocks[len(f.blocks)-1]
	f.blocks = f.blocks[:len(f.blocks)-1]
	f.later = f.later[:len(f.later)-1]
	if !file {
		r.warnUnused(sc)
		for name := range sc.names {
			f.closed[name] = true
		}
		f.nextSlot = sc.base
	}
}

// walk resolves one node; a rewritten node inherits the source span of
// the node it came from (assert and dbg show source text, 08).
func (r *resolver) walk(n *tree.Node) *tree.Node {
	out := r.walkNode(n)
	if out != nil && out.End == 0 {
		out.Off, out.End = n.Off, n.End
	}
	return out
}

func (r *resolver) walkNode(n *tree.Node) *tree.Node {
	switch n.Head {
	case "do":
		return r.block(n)
	case "let":
		r.warnBlockValue(n.Kids[1])
		return r.let(n)
	case "call":
		for _, k := range n.Kids[1:] {
			r.warnBlockValue(k)
		}
	case "name":
		return r.name(n)
	case "fn":
		if info, ok := fnStmt(n); ok {
			// A named fn inside a block: resolve the body first (inside it
			// the name is (self)), then bind the name as a local.
			fnNode := r.function(n)
			b := r.bindNew(info.Name, n.Pos)
			if b == nil {
				return fnNode
			}
			return tree.New("let", n.Pos, ref(n.Pos, *b), fnNode)
		}
		return r.function(n)
	case "return":
		if len(n.Kids) > 0 {
			r.warnBlockValue(n.Kids[0])
		}
		if r.fn.parent == nil {
			r.errs = append(r.errs, diag.Errorf("E0211", n.Pos))
		} else if r.fn.forBody {
			r.errs = append(r.errs, diag.Errorf("E0213", n.Pos))
		}
	case "match":
		return r.matchExpr(n)
	case "test":
		// The test body is a function made where it stands (08), not a
		// named local fn: nothing binds its name.
		return tree.WithLit("test", n.Pos, n.Lit, r.function(n.Kids[0]))
	case "unwrap":
		if n.Lit == "?" {
			if r.fn.parent == nil {
				r.errs = append(r.errs, diag.Errorf("E0214", n.Pos))
			} else if r.fn.forBody {
				// The for body is a lambda, so `?` would return from it and
				// drop an Err into the list: the silent bug E0213 prevents.
				r.errs = append(r.errs, diag.Errorf("E0220", n.Pos))
			}
		}
	case "hole":
		r.errs = append(r.errs, diag.Errorf("E0215", n.Pos))
		return n
	case "typealias":
		r.errs = append(r.errs, diag.Errorf("E0219", n.Pos))
		return tree.New("nil", n.Pos)
	case "import":
		// rootBlock consumes top-level imports; any other is nested.
		r.errs = append(r.errs, diag.Errorf("E0311", n.Pos))
		return tree.New("nil", n.Pos)
	case "field":
		r.checkMember(n.Kids[0], n.Lit.(string), n.Pos)
	}
	out := &tree.Node{Head: n.Head, Pos: n.Pos, Lit: n.Lit}
	for _, k := range n.Kids {
		out.Kids = append(out.Kids, r.walk(k))
	}
	return out
}

func (r *resolver) block(n *tree.Node) *tree.Node {
	r.pushBlock(n, false)
	out := &tree.Node{Head: "do", Pos: n.Pos}
	for _, k := range n.Kids {
		out.Kids = append(out.Kids, r.stmt(k))
	}
	r.popBlock(false)
	return out
}

// stmt resolves one statement of the current block, marking a local fn
// as no longer "later" once its statement is reached.
func (r *resolver) stmt(k *tree.Node) *tree.Node {
	if info, ok := fnStmt(k); ok {
		delete(r.fn.later[len(r.fn.later)-1], info.Name)
	}
	return r.walk(k)
}

// bindNew binds a name in the current block for an assignment: rebinding
// in the same block reuses the storage; a name from an enclosing scope
// is E0205 (01: 블록 안에서 바깥 변수 재할당 금지); a hoisted fn is E0209.
// It returns nil when the assignment is an error.
func (r *resolver) bindNew(name string, pos diag.Pos) *binding {
	f := r.fn
	cur := f.blocks[len(f.blocks)-1]
	if b, ok := cur.names[name]; ok {
		if b.module != nil && !r.opts.Redefine {
			r.errs = append(r.errs, shadowError(name, pos))
			return nil
		}
		if b.constant && !r.opts.Redefine {
			r.errs = append(r.errs, diag.Errorf("E0209", pos, name).Span(runeLen(name)))
			return nil
		}
		if b.constant || b.module != nil {
			// REPL: a plain variable replaces the fn or module (08).
			b = binding{global: b.global, index: b.index}
			cur.names[name] = b
			delete(r.moduleNames, name)
		}
		return &b
	}
	if !r.checkShadow(name, pos) {
		return nil
	}
	r.warnShadow(name, pos)
	if r.definedOutside(name) {
		r.errs = append(r.errs, diag.Errorf("E0205", pos, name).Span(runeLen(name)))
		return nil
	}
	var b binding
	if cur.file {
		b = binding{global: true, index: len(r.globals)}
		r.globals = append(r.globals, name)
	} else {
		b = binding{index: f.nextSlot}
		f.nextSlot++
		if f.nextSlot > f.maxSlot {
			f.maxSlot = f.nextSlot
		}
	}
	cur.names[name] = b
	cur.pos[name] = pos
	return &b
}

// definedOutside reports whether name is bound in an enclosing block of
// this function, in any enclosing function, or as a function's own name.
// Only names bound so far count: scopes are read top-down, and a hoisted
// top-level fn is resolved before any file variable exists.
func (r *resolver) definedOutside(name string) bool {
	for f := r.fn; f != nil; f = f.parent {
		start := len(f.blocks) - 1
		if f == r.fn {
			start = len(f.blocks) - 2
		}
		for i := start; i >= 0; i-- {
			if _, ok := f.blocks[i].names[name]; ok {
				return true
			}
		}
		if f.self == name {
			return true
		}
	}
	return false
}

// let resolves the right-hand side first (x = x + 1 reads the old x),
// then binds the name — or, for destructuring, every name of the pattern
// under the same assignment rules (03).
func (r *resolver) let(n *tree.Node) *tree.Node {
	target, rhs := n.Kids[0], r.walk(n.Kids[1])
	if target.Head != "name" {
		// Destructuring assignment binds like assignment; the destructure
		// desugar emits for a `for` pattern binds fresh names (03).
		bind := r.bindNew
		if n.Lit == "fresh" {
			bind = r.bindFresh
		}
		return tree.New("let", n.Pos, r.bindPattern(target, bind), rhs)
	}
	name := target.Lit.(string)
	b := r.bindNew(name, target.Pos)
	if b == nil {
		return tree.New("let", n.Pos, target, rhs)
	}
	return tree.New("let", n.Pos, ref(target.Pos, *b), rhs)
}

// name resolves a read.
func (r *resolver) name(n *tree.Node) *tree.Node {
	name := n.Lit.(string)
	if name == "_" {
		r.errs = append(r.errs, diag.Errorf("E0215", n.Pos))
		return n
	}
	if node := r.lookup(name, r.fn, n.Pos); node != nil {
		return node
	}
	if r.prelude[name] {
		return tree.Leaf("prelude", n.Pos, name)
	}
	if r.fn.closed[name] {
		r.errs = append(r.errs, diag.Errorf("E0206", n.Pos, name).Span(runeLen(name)).
			WithHelp("assign the block's value instead: `"+name+" = if … ;;`"))
		return n
	}
	for f := r.fn; f != nil; f = f.parent {
		for _, later := range f.later {
			if later[name] {
				r.errs = append(r.errs, diag.Errorf("E0208", n.Pos, name).Span(runeLen(name)))
				return n
			}
		}
	}
	e := diag.Errorf("E0204", n.Pos, name).Span(runeLen(name))
	if p, ok := r.fileVarPos[name]; ok && r.fn.parent != nil {
		e.WithHelp(fmt.Sprintf("`%s` is a file-level variable (assigned on line %d) that this function cannot see: functions copy only the variables that exist when they are created, and top-level fns are created first; pass it as an argument", name, p.Line))
	} else if s := similar(name, r.visible()); s != "" {
		e.WithHelp("a similar name exists: `" + s + "`")
	}
	r.errs = append(r.errs, e)
	return n
}

// lookup finds name in function f: its blocks, its own name, then the
// enclosing functions (captured through every function in between). A
// top-level fn (a constant) is referenced directly; a file variable is
// copied like any other captured value (03).
func (r *resolver) lookup(name string, f *fnScope, pos diag.Pos) *tree.Node {
	for i := len(f.blocks) - 1; i >= 0; i-- {
		if b, ok := f.blocks[i].names[name]; ok {
			f.blocks[i].used[name] = true
			return ref(pos, b)
		}
	}
	if f.self == name {
		return tree.New("self", pos)
	}
	if f.parent == nil {
		return nil
	}
	c, ok := r.captureFrom(name, f)
	if !ok {
		return nil
	}
	if c.Kind == captureGlobal {
		return tree.Leaf("global", pos, int64(c.Index))
	}
	return tree.Leaf("captured", pos, int64(c.Index))
}

// captureGlobal marks "found as a file global": not a real capture.
const captureGlobal tree.CaptureKind = -1

// captureFrom makes name available inside f by capturing it from f's
// parent (recursively through the chain). It returns the capture's index
// in f, or the global's index with Kind captureGlobal.
func (r *resolver) captureFrom(name string, f *fnScope) (tree.Capture, bool) {
	if idx, ok := f.capIndex[name]; ok {
		return tree.Capture{Kind: tree.CaptureCaptured, Index: idx}, true
	}
	p := f.parent
	var src tree.Capture
	found := false
	for i := len(p.blocks) - 1; i >= 0 && !found; i-- {
		if b, ok := p.blocks[i].names[name]; ok {
			p.blocks[i].used[name] = true
			switch {
			case b.global && b.constant:
				// Another top-level fn: it never changes, so a direct
				// reference is indistinguishable from a copy.
				return tree.Capture{Kind: captureGlobal, Index: b.index}, true
			case b.global:
				src, found = tree.Capture{Kind: tree.CaptureGlobal, Index: b.index}, true
			default:
				src, found = tree.Capture{Kind: tree.CaptureLocal, Index: b.index}, true
			}
		}
	}
	if !found && p.self == name {
		src, found = tree.Capture{Kind: tree.CaptureSelf}, true
	}
	if !found && p.parent != nil {
		c, ok := r.captureFrom(name, p)
		if !ok {
			return tree.Capture{}, false
		}
		if c.Kind == captureGlobal {
			return c, true
		}
		src, found = tree.Capture{Kind: tree.CaptureCaptured, Index: c.Index}, true
	}
	if !found {
		return tree.Capture{}, false
	}
	f.captures = append(f.captures, src)
	f.capIndex[name] = len(f.captures) - 1
	return tree.Capture{Kind: tree.CaptureCaptured, Index: len(f.captures) - 1}, true
}

// visible lists every name that could have been meant: all blocks of all
// enclosing functions, closed block names, and the prelude.
func (r *resolver) visible() []string {
	var names []string
	for f := r.fn; f != nil; f = f.parent {
		for _, sc := range f.blocks {
			for name := range sc.names {
				names = append(names, name)
			}
		}
		for name := range f.closed {
			names = append(names, name)
		}
	}
	return append(names, r.preludeList...)
}

func ref(pos diag.Pos, b binding) *tree.Node {
	if b.global {
		return tree.Leaf("global", pos, int64(b.index))
	}
	return tree.Leaf("local", pos, int64(b.index))
}

func runeLen(s string) int { return len([]rune(s)) }
