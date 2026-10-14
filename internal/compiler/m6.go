package compiler

import (
	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

var comparisons = map[string]bool{"==": true, "!=": true, "<": true, ">": true, "<=": true, ">=": true}

var literalHeads = map[string]bool{"int": true, "float": true, "bool": true, "nil": true, "string": true, "symbol": true, "duration": true}

// assertStmt compiles (assert e). A top-level comparison is split so the
// failure report can show each side's value (08); a literal side is not
// reported (its value is its text). Each side is evaluated once.
func (c *compiler) assertStmt(n *tree.Node) *diag.Error {
	e := n.Kids[0]
	a := bytecode.Assert{Text: e.Text(c.opts.Source)}
	if op, ok := e.Lit.(string); e.Head == "op" && ok && comparisons[op] && len(e.Kids) == 2 {
		a.Op = op
		for i, side := range e.Kids {
			if err := c.expr(side, false); err != nil {
				return err
			}
			if !literalHeads[side.Head] {
				if i == 0 {
					a.Left = side.Text(c.opts.Source)
				} else {
					a.Right = side.Text(c.opts.Source)
				}
			}
		}
	} else if err := c.expr(e, false); err != nil {
		return err
	}
	c.proto.Asserts = append(c.proto.Asserts, a)
	c.proto.Emit(bytecode.OpAssert, len(c.proto.Asserts)-1, n.Pos)
	return nil
}

// testStmt compiles (test name fn): with Tests the function is made and
// registered on the runtime; without, the block compiles to nothing (08).
func (c *compiler) testStmt(n *tree.Node) *diag.Error {
	if !c.opts.Tests {
		return nil
	}
	idx, err := c.function(n.Kids[0])
	if err != nil {
		return err
	}
	c.proto.Emit(bytecode.OpClosure, idx, n.Pos)
	c.proto.Emit(bytecode.OpTest, c.proto.Const(value.Str(n.Lit.(string))), n.Pos)
	return nil
}

// isDbgCall reports a direct `dbg(e)`: the prelude dbg with one
// positional argument and nothing else. Aliases and named arguments get
// the plain call.
func (c *compiler) isDbgCall(n *tree.Node) bool {
	callee := n.Kids[0]
	if callee.Head != "prelude" || callee.Lit != "dbg" || len(n.Kids) != 2 {
		return false
	}
	k := n.Kids[1].Head
	return k != "named" && k != "hole"
}
