package resolve

import (
	"unicode"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

// builtinTypes are the type names of 07; tag names (uppercase) are also
// accepted and get their values in M4.
var builtinTypes = map[string]bool{
	"int": true, "float": true, "num": true, "str": true, "bool": true, "nil": true, "sym": true,
	"dur": true, "time": true, "list": true, "map": true, "tuple": true, "range": true, "fn": true,
	"module": true, "task": true, "channel": true, "atom": true, "any": true,
}

// function resolves an (fn …) node in a fresh function scope: params are
// slots 0..n-1, each default sees only the parameters before it (03), the
// body shares the parameters' block.
func (r *resolver) function(n *tree.Node) *tree.Node {
	decl := n.Lit.(*tree.FnInfo)
	info := &tree.FnInfo{Name: decl.Name, Params: append([]tree.Param(nil), decl.Params...), Return: decl.Return}
	f := newFn(r.fn, decl.Name, false)
	f.forBody = decl.ForBody
	r.fn = f
	defer func() { r.fn = f.parent }()

	r.checkTypes(info, n.Pos)
	body := n.Kids[len(n.Kids)-1]
	sc := r.pushBlock(body, false)
	var kids []*tree.Node
	di := 0
	for i, prm := range decl.Params {
		if prm.HasDefault {
			kids = append(kids, r.walk(n.Kids[di]))
			di++
		}
		r.checkShadow(prm.Name, prm.Pos)
		r.warnShadow(prm.Name, prm.Pos)
		sc.names[prm.Name] = binding{index: i}
		sc.pos[prm.Name] = prm.Pos
		sc.param[prm.Name] = true
		f.nextSlot = i + 1
	}
	if f.nextSlot > f.maxSlot {
		f.maxSlot = f.nextSlot
	}
	out := &tree.Node{Head: "do", Pos: body.Pos}
	for _, k := range body.Kids {
		out.Kids = append(out.Kids, r.stmt(k))
	}
	r.popBlock(false)
	info.NumLocals = f.maxSlot
	info.Captures = f.captures
	return tree.WithLit("fn", n.Pos, info, append(kids, out)...)
}

// checkTypes expands aliases in a declaration's annotations in place and
// reports unknown type names (E0212) and alias cycles (E0218).
func (r *resolver) checkTypes(info *tree.FnInfo, pos diag.Pos) {
	for i, p := range info.Params {
		if p.Type != nil {
			info.Params[i].Type = r.expandTypes(p.Type, p.Pos)
		}
	}
	if info.Return != nil {
		info.Return = r.expandTypes(info.Return, pos)
	}
}

func isTagName(s string) bool {
	for _, c := range s {
		return unicode.IsUpper(c)
	}
	return false
}
