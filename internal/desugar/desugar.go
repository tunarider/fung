// Package desugar rewrites the surface tree the parser builds into the
// smaller core language the compiler understands (09. 코어 언어). Both
// are tree.Node trees; this pass only changes heads and shapes. Sugar
// that needs temporaries (match subjects, partial application) uses
// hidden names such as $m0 that no identifier can spell.
package desugar

import (
	"fmt"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

type desugarer struct {
	counter int
}

func Desugar(n *tree.Node) (*tree.Node, *diag.Error) {
	d := &desugarer{}
	return d.walk(n)
}

// hidden returns a fresh hidden variable name.
func (d *desugarer) hidden(prefix string) string {
	name := fmt.Sprintf("$%s%d", prefix, d.counter)
	d.counter++
	return name
}

// walk lowers one node; a rewritten node inherits the source span of the
// node it came from (assert and dbg show source text, 08).
func (d *desugarer) walk(n *tree.Node) (*tree.Node, *diag.Error) {
	out, err := d.walkNode(n)
	if out != nil && out.End == 0 {
		out.Off, out.End = n.Off, n.End
	}
	return out, err
}

func (d *desugarer) walkNode(n *tree.Node) (*tree.Node, *diag.Error) {
	switch n.Head {
	case "program", "block":
		return d.mapKids("do", n)

	case "assign":
		rhs, err := d.walk(n.Kids[1])
		if err != nil {
			return nil, err
		}
		return tree.New("let", n.Pos, n.Kids[0], rhs), nil

	case "binary":
		a, err := d.walk(n.Kids[0])
		if err != nil {
			return nil, err
		}
		b, err := d.walk(n.Kids[1])
		if err != nil {
			return nil, err
		}
		switch n.Lit {
		case "and":
			// a and b  →  if a then (b must be bool) else false
			return tree.New("if", n.Pos, a, checkBool(b), tree.Leaf("bool", n.Pos, false)), nil
		case "or":
			// a or b  →  if a then true else (b must be bool)
			return tree.New("if", n.Pos, a, tree.Leaf("bool", n.Pos, true), checkBool(b)), nil
		case "not_in":
			return tree.WithLit("op", n.Pos, "not", tree.WithLit("op", n.Pos, "in", a, b)), nil
		}
		return tree.WithLit("op", n.Pos, n.Lit, a, b), nil

	case "unary":
		a, err := d.walk(n.Kids[0])
		if err != nil {
			return nil, err
		}
		return tree.WithLit("op", n.Pos, n.Lit, a), nil

	case "then", "if":
		return d.mapKids("if", n)

	case "interp":
		return d.mapKids("concat", n)

	case "pipe":
		return d.pipe(n)

	case "fn":
		return d.fnNode(n)

	case "test":
		// A test block is a function made where it stands (08): it copies
		// the variables visible at that point and can fail on its own.
		body, err := d.walk(n.Kids[0])
		if err != nil {
			return nil, err
		}
		info := &tree.FnInfo{Name: "test: " + n.Lit.(string)}
		return tree.WithLit("test", n.Pos, n.Lit, tree.WithLit("fn", n.Pos, info, body)), nil

	case "assert":
		e, err := d.walk(n.Kids[0])
		if err != nil {
			return nil, err
		}
		return tree.New("assert", n.Pos, e), nil

	case "call":
		for _, k := range n.Kids[1:] {
			if k.Head == "hole" {
				return d.partial(n)
			}
		}
		return d.mapKids("call", n)

	case "for":
		return d.forLoop(n)

	case "match":
		return d.matchExpr(n)

	case "return", "named", "list", "map", "entry", "tuple", "tag", "range",
		"index", "slice", "field", "unwrap", "coalesce", "arm", "hole":
		return d.mapKids(n.Head, n)

	case "int", "float", "bool", "name", "nil", "string", "symbol", "duration", "typealias", "import",
		"pwild", "pbind", "plit", "pnil", "psym", "ptag", "ptuple", "plist", "prest", "pmap", "pentry", "palt":
		return n, nil
	}
	return nil, diag.Errorf("E0801", n.Pos, "desugar: unknown node "+n.Head)
}

// mapKids builds a node with the given head and the same Lit whose
// children are the desugared children of n.
func (d *desugarer) mapKids(head string, n *tree.Node) (*tree.Node, *diag.Error) {
	out := tree.WithLit(head, n.Pos, n.Lit)
	for _, k := range n.Kids {
		c, err := d.walk(k)
		if err != nil {
			return nil, err
		}
		out.Kids = append(out.Kids, c)
	}
	return out, nil
}

// checkBool wraps an expression whose value must be a bool at runtime.
func checkBool(e *tree.Node) *tree.Node {
	return tree.WithLit("op", e.Pos, "bool", e)
}

// pipe lowers `x |> f(a)` to `f(x, a)` and `x |> f` to `f(x)` (01).
func (d *desugarer) pipe(n *tree.Node) (*tree.Node, *diag.Error) {
	rhs := n.Kids[1]
	if rhs.Head == "call" {
		// Insert the surface operand and walk the rebuilt call once: a
		// pre-walked operand would be walked again (and its core `do`
		// nodes rejected) when pipes chain.
		call := tree.New("call", rhs.Pos, rhs.Kids[0])
		call.Kids = append(call.Kids, n.Kids[0])
		call.Kids = append(call.Kids, rhs.Kids[1:]...)
		return d.walk(call)
	}
	x, err := d.walk(n.Kids[0])
	if err != nil {
		return nil, err
	}
	f, err := d.walk(rhs)
	if err != nil {
		return nil, err
	}
	return tree.New("call", n.Pos, f, x), nil
}

// partial lowers `f(a, _, b)` to a block that evaluates f, a and b once
// into hidden variables and returns a lambda over the holes (03):
//
//	(do (let $p0 f) (let $p1 a) (let $p2 b) (fn (h0) (do (call $p0 $p1 h0 $p2))))
func (d *desugarer) partial(n *tree.Node) (*tree.Node, *diag.Error) {
	block := tree.New("do", n.Pos)
	info := &tree.FnInfo{}
	call := tree.New("call", n.Pos)
	holes := 0
	for _, k := range n.Kids {
		switch k.Head {
		case "hole":
			name := fmt.Sprintf("h%d", holes)
			holes++
			info.Params = append(info.Params, tree.Param{Name: name, Pos: k.Pos})
			call.Kids = append(call.Kids, tree.Leaf("name", k.Pos, name))
		case "named":
			v, err := d.walk(k.Kids[0])
			if err != nil {
				return nil, err
			}
			tmp := d.hidden("p")
			block.Kids = append(block.Kids, tree.New("let", k.Pos, tree.Leaf("name", k.Pos, tmp), v))
			call.Kids = append(call.Kids, tree.WithLit("named", k.Pos, k.Lit, tree.Leaf("name", k.Pos, tmp)))
		default:
			v, err := d.walk(k)
			if err != nil {
				return nil, err
			}
			tmp := d.hidden("p")
			block.Kids = append(block.Kids, tree.New("let", k.Pos, tree.Leaf("name", k.Pos, tmp), v))
			call.Kids = append(call.Kids, tree.Leaf("name", k.Pos, tmp))
		}
	}
	block.Kids = append(block.Kids, tree.WithLit("fn", n.Pos, info, tree.New("do", n.Pos, call)))
	return block, nil
}

// forLoop lowers `for pat in e: body ;;` to `map(e, fn(pat): body ;;)`
// with the prelude's map, whatever the user has named `map` (01, 02).
// The lambda is marked ForBody so a return inside it is E0213 (03).
func (d *desugarer) forLoop(n *tree.Node) (*tree.Node, *diag.Error) {
	pat, iter, body := n.Kids[0], n.Kids[1], n.Kids[2]
	iterC, err := d.walk(iter)
	if err != nil {
		return nil, err
	}
	bodyC, err := d.walk(body)
	if err != nil {
		return nil, err
	}
	info := &tree.FnInfo{ForBody: true}
	if pat.Head == "pbind" {
		info.Params = []tree.Param{{Name: pat.Lit.(string), Pos: pat.Pos}}
	} else {
		tmp := d.hidden("f")
		info.Params = []tree.Param{{Name: tmp, Pos: pat.Pos}}
		// Lit "fresh": the pattern's names shadow instead of assigning (03).
		destructure := tree.WithLit("let", pat.Pos, "fresh", pat, tree.Leaf("name", pat.Pos, tmp))
		bodyC.Kids = append([]*tree.Node{destructure}, bodyC.Kids...)
	}
	lambda := tree.WithLit("fn", n.Pos, info, bodyC)
	return tree.New("call", n.Pos, tree.Leaf("prelude", n.Pos, "map"), iterC, lambda), nil
}

// fnNode lowers a function: defaults and body are walked, and every
// destructuring parameter (03) gets a hidden name `$aN` whose value is
// matched at the top of the body with fresh (shadowing) bindings.
func (d *desugarer) fnNode(n *tree.Node) (*tree.Node, *diag.Error) {
	src := n.Lit.(*tree.FnInfo)
	info := *src
	info.Params = append([]tree.Param(nil), src.Params...)
	kids := make([]*tree.Node, 0, len(n.Kids))
	for _, k := range n.Kids {
		c, err := d.walk(k)
		if err != nil {
			return nil, err
		}
		kids = append(kids, c)
	}
	body := kids[len(kids)-1]
	var prologue []*tree.Node
	for i, prm := range info.Params {
		if prm.Pattern == nil {
			continue
		}
		tmp := d.hidden("a")
		info.Params[i] = tree.Param{Name: tmp, Pos: prm.Pos}
		prologue = append(prologue, tree.WithLit("let", prm.Pos, "fresh", prm.Pattern, tree.Leaf("name", prm.Pos, tmp)))
	}
	if len(prologue) > 0 {
		body = &tree.Node{Head: body.Head, Pos: body.Pos, Lit: body.Lit, Kids: append(prologue, body.Kids...), Off: body.Off, End: body.End}
		kids[len(kids)-1] = body
	}
	return tree.WithLit("fn", n.Pos, &info, kids...), nil
}

// matchExpr evaluates the subject once into a hidden variable:
//
//	(do (let $m0 e) (match $m0 (arm pat guard body)…))
func (d *desugarer) matchExpr(n *tree.Node) (*tree.Node, *diag.Error) {
	subject, err := d.walk(n.Kids[0])
	if err != nil {
		return nil, err
	}
	tmp := d.hidden("m")
	m := tree.New("match", n.Pos, tree.Leaf("name", n.Pos, tmp))
	for _, arm := range n.Kids[1:] {
		armC, err := d.mapKids("arm", arm)
		if err != nil {
			return nil, err
		}
		m.Kids = append(m.Kids, armC)
	}
	return tree.New("do", n.Pos, tree.New("let", n.Pos, tree.Leaf("name", n.Pos, tmp), subject), m), nil
}
