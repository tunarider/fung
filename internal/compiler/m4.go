package compiler

import (
	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

// kids compiles every child as a non-tail expression.
func (c *compiler) kids(n *tree.Node) *diag.Error {
	for _, k := range n.Kids {
		if err := c.expr(k, false); err != nil {
			return err
		}
	}
	return nil
}

// collection compiles list/map/tuple/tag/range/index/slice/field/in forms.
func (c *compiler) collection(n *tree.Node, tail bool) (bool, *diag.Error) {
	p := c.proto
	switch n.Head {
	case "list":
		if err := c.kids(n); err != nil {
			return true, err
		}
		p.Emit(bytecode.OpList, len(n.Kids), n.Pos)
	case "map":
		for _, entry := range n.Kids {
			p.Emit(bytecode.OpConst, p.Const(value.Str(entry.Lit.(string))), entry.Pos)
			if err := c.expr(entry.Kids[0], false); err != nil {
				return true, err
			}
		}
		p.Emit(bytecode.OpMap, len(n.Kids), n.Pos)
	case "tuple":
		if err := c.kids(n); err != nil {
			return true, err
		}
		p.Emit(bytecode.OpTuple, len(n.Kids), n.Pos)
	case "tag":
		p.Emit(bytecode.OpConst, p.Const(value.Intern(n.Lit.(string))), n.Pos)
		if err := c.kids(n); err != nil {
			return true, err
		}
		p.Emit(bytecode.OpTag, len(n.Kids), n.Pos)
	case "range":
		if err := c.kids(n); err != nil {
			return true, err
		}
		inclusive := 0
		if n.Lit == "..=" {
			inclusive = 1
		}
		p.Emit(bytecode.OpRange, inclusive, n.Pos)
	case "index":
		if err := c.kids(n); err != nil {
			return true, err
		}
		p.Emit(bytecode.OpIndex, 0, n.Pos)
	case "slice":
		if err := c.expr(n.Kids[0], false); err != nil {
			return true, err
		}
		flags := 0
		if n.Kids[1].Head != "nil" {
			flags |= 1
			if err := c.expr(n.Kids[1], false); err != nil {
				return true, err
			}
		}
		if n.Kids[2].Head != "nil" {
			flags |= 2
			if err := c.expr(n.Kids[2], false); err != nil {
				return true, err
			}
		}
		if n.Lit == "..=" {
			flags |= 4
		}
		p.Emit(bytecode.OpSlice, flags, n.Pos)
	case "field":
		if err := c.expr(n.Kids[0], false); err != nil {
			return true, err
		}
		p.Emit(bytecode.OpField, p.Const(value.Str(n.Lit.(string))), n.Pos)
	default:
		return false, nil
	}
	return true, nil
}

// unwrap compiles `!` and `?` (02). `?` shares the function's return
// path: UNWRAPTEST leaves the Err on the stack and jumps to a RETURN.
func (c *compiler) unwrap(n *tree.Node) *diag.Error {
	p := c.proto
	if err := c.expr(n.Kids[0], false); err != nil {
		return err
	}
	if n.Lit == "!" {
		p.Emit(bytecode.OpUnwrapAbort, 0, n.Pos)
		return nil
	}
	toReturn := p.Emit(bytecode.OpUnwrapTest, 0, n.Pos)
	toCont := p.Emit(bytecode.OpJump, 0, n.Pos)
	p.Patch(toReturn, len(p.Code))
	if c.hasRet && p.ReturnType >= 0 {
		p.Emit(bytecode.OpCheckReturn, p.ReturnType, n.Pos)
	}
	p.Emit(bytecode.OpReturn, 0, n.Pos)
	p.Patch(toCont, len(p.Code))
	return nil
}

// coalesce compiles `a ?? b`: COALESCE jumps to b when a is nil or Err.
func (c *compiler) coalesce(n *tree.Node, tail bool) *diag.Error {
	p := c.proto
	if err := c.expr(n.Kids[0], false); err != nil {
		return err
	}
	toRHS := p.Emit(bytecode.OpCoalesce, 0, n.Pos)
	toEnd := p.Emit(bytecode.OpJump, 0, n.Pos)
	p.Patch(toRHS, len(p.Code))
	if err := c.expr(n.Kids[1], tail); err != nil {
		return err
	}
	p.Patch(toEnd, len(p.Code))
	return nil
}

// match compiles (match subject (arm pat guard body)…): each arm loads
// the subject, tries its pattern, checks its guard, and runs its body in
// tail position; falling off the end is E0601 (09).
func (c *compiler) match(n *tree.Node, tail bool) *diag.Error {
	p := c.proto
	subject := n.Kids[0]
	var ends []int
	for _, arm := range n.Kids[1:] {
		pat, guard, body := arm.Kids[0], arm.Kids[1], arm.Kids[2]
		if err := c.expr(subject, false); err != nil {
			return err
		}
		idx, err := c.addPattern(pat)
		if err != nil {
			return err
		}
		p.Emit(bytecode.OpMatch, idx, pat.Pos)
		var fails []int
		fails = append(fails, p.Emit(bytecode.OpJumpIfFalse, 0, pat.Pos))
		if guard.Head != "nil" {
			if err := c.expr(guard, false); err != nil {
				return err
			}
			fails = append(fails, p.Emit(bytecode.OpJumpIfFalse, 0, guard.Pos))
		}
		if err := c.expr(body, tail); err != nil {
			return err
		}
		ends = append(ends, p.Emit(bytecode.OpJump, 0, arm.Pos))
		for _, f := range fails {
			p.Patch(f, len(p.Code))
		}
	}
	if err := c.expr(subject, false); err != nil {
		return err
	}
	p.Emit(bytecode.OpNoMatch, 0, n.Pos)
	for _, e := range ends {
		p.Patch(e, len(p.Code))
	}
	return nil
}
