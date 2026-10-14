// Package compiler turns a resolved core tree into bytecode. It is a
// single recursive walk: each core form knows how to emit itself, every
// emitted instruction carries the position of the node it came from, and
// a `tail` flag follows the walk so calls in tail position become
// TAILCALL (03. 꼬리 위치).
package compiler

import (
	"time"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/resolve"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

type compiler struct {
	proto   *bytecode.Proto
	prelude map[string]value.Value
	opts    Options
	hasRet  bool // a return type annotation disables tail calls (03)
}

// Options configures compilation of one file.
type Options struct {
	Source string // the file's text, for assert and dbg source spans (08)
	Tests  bool   // register test blocks (fung test / check); false drops them (fung run)
}

// Compile produces the <main> proto for a resolved program. The main body
// never uses tail calls: it is the entry frame.
func Compile(res resolve.Result, prelude map[string]value.Value, opts Options) (*bytecode.Proto, *diag.Error) {
	c := &compiler{
		proto:   &bytecode.Proto{Name: "<main>", NumLocals: res.NumLocals, GlobalNames: res.Globals, Exports: res.Exports, ReturnType: -1},
		prelude: prelude,
		opts:    opts,
		hasRet:  true,
	}
	if err := c.block(res.Core, false); err != nil {
		return nil, err
	}
	c.proto.Emit(bytecode.OpReturn, 0, res.Core.Pos)
	return c.proto, nil
}

// block compiles (do e...) leaving the block's value on the stack: the
// last expression's value, or nil when the block ends in a statement
// (01: 블록이 문장으로 끝나면 nil). Only the last expression is in tail
// position.
func (c *compiler) block(n *tree.Node, tail bool) *diag.Error {
	if len(n.Kids) == 0 {
		c.proto.Emit(bytecode.OpConst, c.proto.Const(value.Nil{}), n.Pos)
		return nil
	}
	for i, k := range n.Kids {
		last := i == len(n.Kids)-1
		if k.Head == "let" || k.Head == "import" || k.Head == "assert" || k.Head == "test" {
			var err *diag.Error
			switch k.Head {
			case "let":
				err = c.let(k)
			case "import":
				err = c.importStmt(k)
			case "assert":
				err = c.assertStmt(k)
			case "test":
				err = c.testStmt(k)
			}
			if err != nil {
				return err
			}
			if last {
				c.proto.Emit(bytecode.OpConst, c.proto.Const(value.Nil{}), k.Pos)
			}
			continue
		}
		if err := c.expr(k, tail && last); err != nil {
			return err
		}
		if !last {
			c.proto.Emit(bytecode.OpPop, 0, k.Pos)
		}
	}
	return nil
}

// let compiles an assignment: the value, then a store into the slot or
// global that resolve chose — or a MATCHORFAIL for a destructuring pattern.
func (c *compiler) let(n *tree.Node) *diag.Error {
	if err := c.expr(n.Kids[1], false); err != nil {
		return err
	}
	target := n.Kids[0]
	switch target.Head {
	case "ptuple", "plist", "pmap", "ptag", "palt", "plit", "pnil", "psym", "pwild", "pbind":
		idx, err := c.addPattern(target)
		if err != nil {
			return err
		}
		c.proto.Emit(bytecode.OpMatchOrFail, idx, target.Pos)
		return nil
	}
	return c.storeRef(target)
}

// storeRef emits the store into a resolved (local n) or (global n).
func (c *compiler) storeRef(target *tree.Node) *diag.Error {
	switch target.Head {
	case "local":
		c.proto.Emit(bytecode.OpStoreLocal, int(target.Lit.(int64)), target.Pos)
	case "global":
		c.proto.Emit(bytecode.OpStoreGlobal, int(target.Lit.(int64)), target.Pos)
	default:
		return diag.Errorf("E0801", target.Pos, "compiler: unresolved assignment target "+target.Head)
	}
	return nil
}

// importStmt compiles (import key binding (member name binding)…): the
// module value comes from IMPORT (cached after the first run, 04) and is
// stored into the module name and each listed member.
func (c *compiler) importStmt(n *tree.Node) *diag.Error {
	p := c.proto
	key := p.Const(value.Str(n.Lit.(string)))
	if n.Kids[0].Head != "nil" {
		p.Emit(bytecode.OpImport, key, n.Pos)
		if err := c.storeRef(n.Kids[0]); err != nil {
			return err
		}
	}
	for _, m := range n.Kids[1:] {
		p.Emit(bytecode.OpImport, key, m.Pos)
		p.Emit(bytecode.OpField, p.Const(value.Str(m.Lit.(string))), m.Pos)
		if err := c.storeRef(m.Kids[0]); err != nil {
			return err
		}
	}
	return nil
}

var binaryOps = map[string]bytecode.Op{
	"+": bytecode.OpAdd, "-": bytecode.OpSub, "*": bytecode.OpMul, "/": bytecode.OpDiv,
	"//": bytecode.OpFloorDiv, "%": bytecode.OpMod, "**": bytecode.OpPow,
	"==": bytecode.OpEq, "!=": bytecode.OpNe, "<": bytecode.OpLt, ">": bytecode.OpGt,
	"<=": bytecode.OpLe, ">=": bytecode.OpGe,
}

var unaryOps = map[string]bytecode.Op{
	"-": bytecode.OpNeg, "not": bytecode.OpNot, "bool": bytecode.OpCheckBool,
}

// expr compiles an expression; tail says whether its value is what the
// enclosing function returns.
func (c *compiler) expr(n *tree.Node, tail bool) *diag.Error {
	p := c.proto
	switch n.Head {
	case "do":
		return c.block(n, tail)
	case "int":
		p.Emit(bytecode.OpConst, p.Const(value.Int(n.Lit.(int64))), n.Pos)
	case "float":
		p.Emit(bytecode.OpConst, p.Const(value.Float(n.Lit.(float64))), n.Pos)
	case "bool":
		p.Emit(bytecode.OpConst, p.Const(value.Bool(n.Lit.(bool))), n.Pos)
	case "nil":
		p.Emit(bytecode.OpConst, p.Const(value.Nil{}), n.Pos)
	case "string":
		p.Emit(bytecode.OpConst, p.Const(value.Str(n.Lit.(string))), n.Pos)
	case "symbol":
		p.Emit(bytecode.OpConst, p.Const(value.Intern(n.Lit.(string))), n.Pos)
	case "duration":
		p.Emit(bytecode.OpConst, p.Const(value.Dur(n.Lit.(time.Duration))), n.Pos)
	case "local":
		p.Emit(bytecode.OpLoadLocal, int(n.Lit.(int64)), n.Pos)
	case "global":
		p.Emit(bytecode.OpLoadGlobal, int(n.Lit.(int64)), n.Pos)
	case "captured":
		p.Emit(bytecode.OpLoadCaptured, int(n.Lit.(int64)), n.Pos)
	case "self":
		p.Emit(bytecode.OpLoadSelf, 0, n.Pos)
	case "prelude":
		name := n.Lit.(string)
		fn, ok := c.prelude[name]
		if !ok {
			return diag.Errorf("E0801", n.Pos, "compiler: unknown prelude name "+name)
		}
		p.Emit(bytecode.OpConst, p.Const(fn), n.Pos)
	case "op":
		for _, k := range n.Kids {
			if err := c.expr(k, false); err != nil {
				return err
			}
		}
		// A missing table entry must be an error, not the zero Op (CONST):
		// later milestones add operators to the parser first.
		table := unaryOps
		if len(n.Kids) == 2 {
			table = binaryOps
		}
		if n.Lit == "in" {
			p.Emit(bytecode.OpIn, 0, n.Pos)
			return nil
		}
		op, ok := table[n.Lit.(string)]
		if !ok {
			return diag.Errorf("E0801", n.Pos, "compiler: unknown operator "+n.Lit.(string))
		}
		p.Emit(op, 0, n.Pos)
	case "if":
		if err := c.expr(n.Kids[0], false); err != nil {
			return err
		}
		// The jump carries the condition's position so a non-bool
		// condition is reported where the condition is.
		toElse := p.Emit(bytecode.OpJumpIfFalse, 0, n.Kids[0].Pos)
		if err := c.expr(n.Kids[1], tail); err != nil {
			return err
		}
		toEnd := p.Emit(bytecode.OpJump, 0, n.Pos)
		p.Patch(toElse, len(p.Code))
		if err := c.expr(n.Kids[2], tail); err != nil {
			return err
		}
		p.Patch(toEnd, len(p.Code))
	case "concat":
		for _, k := range n.Kids {
			if err := c.expr(k, false); err != nil {
				return err
			}
		}
		p.Emit(bytecode.OpConcat, len(n.Kids), n.Pos)
	case "list", "map", "tuple", "tag", "range", "index", "slice", "field":
		_, err := c.collection(n, tail)
		return err
	case "unwrap":
		return c.unwrap(n)
	case "coalesce":
		return c.coalesce(n, tail)
	case "match":
		return c.match(n, tail)
	case "call":
		return c.call(n, tail)
	case "fn":
		idx, err := c.function(n)
		if err != nil {
			return err
		}
		p.Emit(bytecode.OpClosure, idx, n.Pos)
	case "return":
		if err := c.expr(n.Kids[0], !c.hasRet); err != nil {
			return err
		}
		if c.hasRet && c.proto.ReturnType >= 0 {
			p.Emit(bytecode.OpCheckReturn, c.proto.ReturnType, n.Pos)
		}
		p.Emit(bytecode.OpReturn, 0, n.Pos)
	case "let":
		// Assignment is a statement (01); it cannot appear in expression position.
		return diag.Errorf("E0801", n.Pos, "compiler: let outside a block")
	default:
		return diag.Errorf("E0801", n.Pos, "compiler: unknown core form "+n.Head)
	}
	return nil
}
