package resolve

import (
	"sort"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

// binder binds one pattern name and returns where it lives, or nil when
// the binding was an error (already reported).
type binder func(name string, pos diag.Pos) *binding

// bindFresh always binds a new name in the current block (or reuses the
// block's own binding of that name): pattern names shadow outer ones (04).
func (r *resolver) bindFresh(name string, pos diag.Pos) *binding {
	if !r.checkShadow(name, pos) {
		return nil
	}
	f := r.fn
	cur := f.blocks[len(f.blocks)-1]
	if b, ok := cur.names[name]; ok && !b.constant {
		return &b
	}
	r.warnShadow(name, pos)
	cur.pos[name] = pos
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
	return &b
}

// bindPattern rewrites the names of a pattern into (local n)/(global n)
// references using bind. Alternatives must bind the same names (03,
// E0216); the first alternative binds and the others reuse its slots. A
// name bound twice in one pattern is E0221 (03).
func (r *resolver) bindPattern(n *tree.Node, bind binder) *tree.Node {
	return r.bindPat(n, bind, map[string]bool{})
}

func (r *resolver) bindPat(n *tree.Node, bind binder, seen map[string]bool) *tree.Node {
	switch n.Head {
	case "pbind", "prest":
		if n.Lit == nil {
			return n
		}
		name := n.Lit.(string)
		if seen[name] {
			r.errs = append(r.errs, diag.Errorf("E0221", n.Pos, name).Span(runeLen(name)))
			return n
		}
		seen[name] = true
		b := bind(name, n.Pos)
		if b == nil {
			return n
		}
		return tree.New(n.Head, n.Pos, ref(n.Pos, *b))
	case "palt":
		first := patternNames(n.Kids[0])
		for _, alt := range n.Kids[1:] {
			if !sameNames(first, patternNames(alt)) {
				r.errs = append(r.errs, diag.Errorf("E0216", alt.Pos))
				return n
			}
		}
		out := tree.New("palt", n.Pos)
		for _, alt := range n.Kids {
			// Each alternative binds the same names once; start every
			// alternative from the names seen before the `|`.
			altSeen := map[string]bool{}
			for k, v := range seen {
				altSeen[k] = v
			}
			out.Kids = append(out.Kids, r.bindPat(alt, bind, altSeen))
		}
		for _, name := range first {
			seen[name] = true
		}
		return out
	}
	out := tree.WithLit(n.Head, n.Pos, n.Lit)
	for _, k := range n.Kids {
		out.Kids = append(out.Kids, r.bindPat(k, bind, seen))
	}
	return out
}

// patternNames lists the names a pattern binds, sorted.
func patternNames(n *tree.Node) []string {
	var names []string
	var walk func(*tree.Node)
	walk = func(n *tree.Node) {
		if (n.Head == "pbind" || n.Head == "prest") && n.Lit != nil {
			names = append(names, n.Lit.(string))
		}
		for _, k := range n.Kids {
			walk(k)
		}
	}
	walk(n)
	sort.Strings(names)
	return names
}

func sameNames(a, b []string) bool {
	return strings.Join(a, "\x00") == strings.Join(b, "\x00")
}

// matchExpr resolves (match subject (arm pat guard body)…): each arm is
// its own block scope whose pattern names are fresh bindings.
func (r *resolver) matchExpr(n *tree.Node) *tree.Node {
	out := tree.New("match", n.Pos, r.walk(n.Kids[0]))
	for _, arm := range n.Kids[1:] {
		r.pushBlock(arm, false)
		pat := r.bindPattern(arm.Kids[0], r.bindFresh)
		guard := r.walk(arm.Kids[1])
		body := r.walk(arm.Kids[2])
		r.popBlock(false)
		out.Kids = append(out.Kids, tree.New("arm", arm.Pos, pat, guard, body))
	}
	return out
}
