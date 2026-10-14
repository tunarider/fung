package bytecode

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

// Param is one parameter of a compiled function.
type Param struct {
	Name       string
	HasDefault bool
	Rest       bool // ...name: collects surplus positional arguments
	Type       int  // index into Types, -1 for unchecked
}

// Proto is a compiled function body ("prototype"): code, its constant
// pool, one source position per instruction, how many local slots its
// frame needs, (for the module body) the names of the globals in index
// order, and the function's parameters, child functions, captures and
// type specs.
type Proto struct {
	Name        string
	Code        []Instr
	Consts      []value.Value
	Pos         []diag.Pos
	NumLocals   int
	GlobalNames []string
	Params      []Param
	Protos      []*Proto       // child functions, referenced by CLOSURE
	Captures    []tree.Capture // what CLOSURE copies from the creating frame
	Types       [][]string     // type specs referenced by CHECKTYPE/CHECKRETURN
	ReturnType  int            // index into Types, -1 for unchecked
	Patterns    []*Pattern     // pattern trees referenced by MATCH/MATCHORFAIL
	Exports     []string       // (module body) the public globals, in order (04)
	Asserts     []Assert       // what ASSERT reports (08)
	Builtin     bool           // compiled from an embedded extras file: errors point at the caller (06)
}

// Assert is the source of one assert statement: the whole expression and,
// for a top-level comparison, each side's text ("" for a literal side) and
// the operator.
type Assert struct {
	Text, Left, Right, Op string
}

// AddType registers a type spec and returns its index.
func (p *Proto) AddType(alts []string) int {
	p.Types = append(p.Types, alts)
	return len(p.Types) - 1
}

// Emit appends an instruction and returns its index (for Patch).
func (p *Proto) Emit(op Op, arg int, pos diag.Pos) int {
	p.Code = append(p.Code, Encode(op, arg))
	p.Pos = append(p.Pos, pos)
	return len(p.Code) - 1
}

// Patch rewrites the operand of an earlier instruction (jump targets).
func (p *Proto) Patch(at int, arg int) {
	p.Code[at] = Encode(p.Code[at].Op(), arg)
}

// Const adds a constant, reusing an existing equal one of the same type.
func (p *Proto) Const(v value.Value) int {
	for i, c := range p.Consts {
		if c.TypeName() == v.TypeName() && value.Equal(c, v) {
			return i
		}
	}
	p.Consts = append(p.Consts, v)
	return len(p.Consts) - 1
}
