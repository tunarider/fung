package compiler

import (
	"fmt"
	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

// maxArgs is what the 12-bit CALL operand fields and CHECKTYPE packing hold.
const maxArgs = 4095

// call compiles (call f pos… (named k e)…): callee, positional arguments,
// then each named argument as its symbol followed by its value, then
// CALL — or TAILCALL in tail position (09. 호출 규약).
func (c *compiler) call(n *tree.Node, tail bool) *diag.Error {
	p := c.proto
	if err := c.expr(n.Kids[0], false); err != nil {
		return err
	}
	positional, named := 0, 0
	if c.isDbgCall(n) {
		// dbg(e) also receives e's source text and place (08).
		arg := n.Kids[1]
		if err := c.expr(arg, false); err != nil {
			return err
		}
		p.Emit(bytecode.OpConst, p.Const(value.Str(arg.Text(c.opts.Source))), arg.Pos)
		p.Emit(bytecode.OpConst, p.Const(value.Str(fmt.Sprintf("%s:%d", n.Pos.File, n.Pos.Line))), n.Pos)
		p.Emit(bytecode.OpCall, bytecode.EncodeCall(3, 0), n.Pos)
		return nil
	}
	for _, k := range n.Kids[1:] {
		if k.Head == "named" {
			named++
			p.Emit(bytecode.OpConst, p.Const(value.Intern(k.Lit.(string))), k.Pos)
			if err := c.expr(k.Kids[0], false); err != nil {
				return err
			}
			continue
		}
		positional++
		if err := c.expr(k, false); err != nil {
			return err
		}
	}
	if positional > maxArgs || named > maxArgs {
		return diag.Errorf("E0806", n.Pos, "arguments", maxArgs)
	}
	op := bytecode.OpCall
	if tail {
		op = bytecode.OpTailCall
	}
	p.Emit(op, bytecode.EncodeCall(positional, named), n.Pos)
	return nil
}

// function compiles an (fn …) node into a child proto and returns its
// index in the current proto's Protos. The prologue fills defaults for
// unassigned slots (JUMPIFSET sees the Go nil the VM leaves there) and
// checks parameter types; the body's last value is checked against the
// return type when there is one.
func (c *compiler) function(n *tree.Node) (int, *diag.Error) {
	info := n.Lit.(*tree.FnInfo)
	if len(info.Params) > maxArgs {
		return 0, diag.Errorf("E0806", n.Pos, "parameters", maxArgs)
	}
	child := &bytecode.Proto{Name: info.Name, NumLocals: info.NumLocals, Captures: info.Captures, ReturnType: -1}
	cc := &compiler{proto: child, prelude: c.prelude, opts: c.opts}
	for _, prm := range info.Params {
		cp := bytecode.Param{Name: prm.Name, HasDefault: prm.HasDefault, Rest: prm.Rest, Type: -1}
		if prm.Type != nil {
			cp.Type = child.AddType(prm.Type)
		}
		child.Params = append(child.Params, cp)
	}
	if info.Return != nil {
		child.ReturnType = child.AddType(info.Return)
	}
	cc.hasRet = child.ReturnType >= 0

	// Prologue: defaults, then type checks.
	di := 0
	for i, prm := range child.Params {
		if !prm.HasDefault {
			continue
		}
		child.Emit(bytecode.OpLoadLocal, i, n.Pos)
		skip := child.Emit(bytecode.OpJumpIfSet, 0, n.Pos)
		if err := cc.expr(n.Kids[di], false); err != nil {
			return 0, err
		}
		di++
		child.Emit(bytecode.OpStoreLocal, i, n.Pos)
		child.Patch(skip, len(child.Code))
	}
	for i, prm := range child.Params {
		if prm.Type >= 0 {
			child.Emit(bytecode.OpCheckType, i|prm.Type<<12, info.Params[i].Pos)
		}
	}

	body := n.Kids[len(n.Kids)-1]
	if err := cc.block(body, !cc.hasRet); err != nil {
		return 0, err
	}
	if cc.hasRet {
		child.Emit(bytecode.OpCheckReturn, child.ReturnType, body.Pos)
	}
	child.Emit(bytecode.OpReturn, 0, body.Pos)
	c.proto.Protos = append(c.proto.Protos, child)
	return len(c.proto.Protos) - 1, nil
}
