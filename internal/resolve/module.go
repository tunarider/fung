package resolve

import (
	"fmt"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

// ModuleInfo is what resolve knows about an imported module: enough to
// check member names at compile time (04: 정적으로 알면 컴파일 에러).
type ModuleInfo struct {
	Key     string
	Name    string
	Exports []string
}

// Options configures a file's resolution.
type Options struct {
	Imports  map[string]ModuleInfo // spec as written → module
	Library  bool                  // imported file: top-level restrictions (04)
	Globals  []string              // REPL: globals that already exist, in index order
	Redefine bool                  // REPL: fns and imports may be redefined
}

// importStmt binds an import's names as constant globals — module names
// marked as modules so they cannot be shadowed (04) — and rewrites the
// statement to (import key (global n)|(nil) (member name (global m))…).
func (r *resolver) importStmt(n *tree.Node, sc *scope) *tree.Node {
	im := n.Lit.(*tree.Import)
	info, ok := r.opts.Imports[im.Spec]
	if !ok {
		r.errs = append(r.errs, diag.Errorf("E0301", n.Pos, im.Spec))
		return tree.New("nil", n.Pos)
	}
	r.imports = append(r.imports, info.Key)
	out := tree.WithLit("import", n.Pos, info.Key)
	modName := im.Alias
	if modName == "" && im.Names == nil {
		modName = info.Name
	}
	if b := r.bindModule(modName, n.Pos, info, sc); b != nil {
		out.Kids = append(out.Kids, ref(n.Pos, *b))
	} else {
		out.Kids = append(out.Kids, tree.New("nil", n.Pos))
	}
	for i, name := range im.Names {
		pos := im.NamePos[i]
		if strings.HasPrefix(name, "_") {
			r.errs = append(r.errs, diag.Errorf("E0304", pos, name, info.Name).Span(runeLen(name)))
			continue
		}
		if !contains(info.Exports, name) {
			r.errs = append(r.errs, diag.Errorf("E0303", pos, info.Name, name).Span(runeLen(name)))
			continue
		}
		b := r.bindConst(name, pos, sc) // a constant global, like a hoisted fn
		if b == nil {
			continue
		}
		out.Kids = append(out.Kids, tree.WithLit("member", pos, name, ref(pos, *b)))
	}
	return out
}

// bindModule binds a module name: constant, unshadowable, once (E0309).
// An empty name (only a name list was written) binds nothing.
func (r *resolver) bindModule(name string, pos diag.Pos, info ModuleInfo, sc *scope) *binding {
	if name == "" {
		return nil
	}
	if b, ok := sc.names[name]; ok {
		if r.opts.Redefine && (b.module != nil || !b.constant) {
			mi := info
			b = binding{global: true, index: b.index, constant: true, module: &mi}
			sc.names[name] = b
			r.moduleNames[name] = &mi
			r.importBound[name] = true
			return &b
		}
		if b.module != nil {
			r.errs = append(r.errs, diag.Errorf("E0309", pos, name).Span(runeLen(name)))
		} else {
			r.errs = append(r.errs, shadowError(name, pos))
		}
		return nil
	}
	r.warnShadow(name, pos)
	mi := info
	b := binding{global: true, index: len(r.globals), constant: true, module: &mi}
	r.globals = append(r.globals, name)
	sc.names[name] = b
	sc.pos[name] = pos
	r.moduleNames[name] = &mi
	r.importBound[name] = true
	return &b
}

// bindConst binds a listed import name as a constant global (plan: 목록의
// 이름도 상수): hoisted fns see it, nothing may reassign it (E0209).
func (r *resolver) bindConst(name string, pos diag.Pos, sc *scope) *binding {
	if b, ok := sc.names[name]; ok {
		if b.module != nil {
			r.errs = append(r.errs, shadowError(name, pos))
		} else {
			r.errs = append(r.errs, diag.Errorf("E0209", pos, name).Span(runeLen(name)))
		}
		return nil
	}
	r.warnShadow(name, pos)
	b := binding{global: true, index: len(r.globals), constant: true}
	r.globals = append(r.globals, name)
	sc.names[name] = b
	sc.pos[name] = pos
	r.importBound[name] = true
	return &b
}

func shadowError(name string, pos diag.Pos) *diag.Error {
	return diag.Errorf("E0308", pos, name).Span(runeLen(name)).
		WithHelp(fmt.Sprintf("import it under another name: `import %s as m`", name))
}

// checkShadow reports E0308 when a new name would hide an imported
// module name (04: import한 모듈 이름을 가리면 에러). Every place that
// creates a name calls it: assignment, fn, params, patterns.
func (r *resolver) checkShadow(name string, pos diag.Pos) bool {
	if _, ok := r.moduleNames[name]; ok {
		r.errs = append(r.errs, shadowError(name, pos))
		return false
	}
	return true
}

// checkMember reports a static member access on a module binding:
// private names are E0304, unknown names E0303 (04).
func (r *resolver) checkMember(target *tree.Node, member string, pos diag.Pos) {
	if target.Head != "name" {
		return
	}
	info, ok := r.moduleNames[target.Lit.(string)]
	if !ok {
		return
	}
	if strings.HasPrefix(member, "_") {
		r.errs = append(r.errs, diag.Errorf("E0304", pos, member, info.Name).Span(runeLen(member)))
		return
	}
	if !contains(info.Exports, member) {
		r.errs = append(r.errs, diag.Errorf("E0303", pos, info.Name, member).Span(runeLen(member)))
	}
}

func contains(xs []string, s string) bool {
	for _, x := range xs {
		if x == s {
			return true
		}
	}
	return false
}
