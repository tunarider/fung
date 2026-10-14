// Package bytecode defines the instruction set of the fung VM. Every
// instruction is one 32-bit word: 8 bits of opcode and 24 bits of
// operand (09. 명령 인코딩). Each instruction also has a source position
// in the parallel Proto.Pos slice (08: 모든 명령은 소스 위치를 가진다).
package bytecode

import "fmt"

type Op uint8

const (
	OpConst Op = iota // push Consts[arg]
	OpPop             // discard top
	OpAdd             // a b -- a+b
	OpSub
	OpMul
	OpDiv      // always float
	OpFloorDiv // //
	OpMod
	OpPow
	OpNeg // a -- -a
	OpNot // a -- not a (a must be bool)
	OpEq
	OpNe
	OpLt
	OpGt
	OpLe
	OpGe
	OpCheckBool    // a -- a, error unless a is bool
	OpJump         // ip = arg
	OpJumpIfFalse  // c -- ; if c is false ip = arg; c must be bool
	OpCall         // f a1..an -- result (arg = n)
	OpReturn       // v -- ; return v from the current frame
	OpLoadLocal    // -- slots[base+arg]
	OpStoreLocal   // v -- ; slots[base+arg] = v
	OpLoadGlobal   // -- globals[arg] (E0207 if unassigned)
	OpStoreGlobal  // v -- ; globals[arg] = v
	OpConcat       // s1..sn -- str (arg = n; parts formatted as print would)
	OpClosure      // -- closure                (arg = index into Protos)
	OpLoadCaptured // -- closure.Captured[arg]
	OpLoadSelf     // -- the running closure
	OpTailCall     // like CALL, reusing the current frame
	OpJumpIfSet    // v -- ; if v is assigned (not Go nil) ip = arg
	OpCheckType    // -- ; slot arg&0xFFF must match Types[arg>>12]  (E0403)
	OpCheckReturn  // v -- v ; v must match Types[arg]                (E0404)
	OpList         // a1..an -- list            (arg = n)
	OpMap          // k1 v1 .. kn vn -- map     (arg = n; keys are Str)
	OpTuple        // a1..an -- tuple           (arg = n)
	OpTag          // name a1..an -- tag        (arg = n; name is a Sym)
	OpRange        // start end -- range        (arg = 1 if inclusive)
	OpIndex        // x i -- x[i]
	OpSlice        // x [start] [end] -- slice  (arg bit0 = has start, bit1 = has end)
	OpField        // x -- x.name               (arg = const index of the Str name)
	OpIn           // x c -- bool
	OpUnwrapAbort  // r -- v   (!: Ok(v) -> v; Err -> E0603; nil -> E0604; other -> E0605)
	OpUnwrapTest   // r -- v, or leaves Err and jumps to arg  (?)
	OpCoalesce     // r -- v, or pops nil/Err and jumps to arg  (??)
	OpMatch        // v -- bool  (arg = Patterns index; binds on success)
	OpMatchOrFail  // v --       (E0602 on failure)
	OpNoMatch      // v --       (E0601 with the value)
	OpImport       // -- module  (arg = const index of the Str key; runs the module body on first use)
	OpAssert       // cond -- or l r -- (arg = Asserts index; E0807 with operand values on failure)
	OpTest         // fn --      (arg = const index of the Str name; registers a test on the runtime)
)

// CALL and TAILCALL pack the positional count in the low 12 bits of the
// operand and the named-argument count in the next 12.
func EncodeCall(positional, named int) int { return positional | named<<12 }

// DecodeCall unpacks a CALL operand.
func DecodeCall(arg int) (positional, named int) { return arg & 0xFFF, arg >> 12 }

var opNames = [...]string{
	OpConst: "CONST", OpPop: "POP", OpAdd: "ADD", OpSub: "SUB", OpMul: "MUL", OpDiv: "DIV",
	OpFloorDiv: "FLOORDIV", OpMod: "MOD", OpPow: "POW", OpNeg: "NEG", OpNot: "NOT",
	OpEq: "EQ", OpNe: "NE", OpLt: "LT", OpGt: "GT", OpLe: "LE", OpGe: "GE",
	OpCheckBool: "CHECKBOOL", OpJump: "JUMP", OpJumpIfFalse: "JUMPIFFALSE",
	OpCall: "CALL", OpReturn: "RETURN",
	OpLoadLocal: "LOADLOCAL", OpStoreLocal: "STORELOCAL",
	OpLoadGlobal: "LOADGLOBAL", OpStoreGlobal: "STOREGLOBAL", OpConcat: "CONCAT",
	OpClosure: "CLOSURE", OpLoadCaptured: "LOADCAPTURED", OpLoadSelf: "LOADSELF",
	OpTailCall: "TAILCALL", OpJumpIfSet: "JUMPIFSET", OpCheckType: "CHECKTYPE", OpCheckReturn: "CHECKRETURN",
	OpList: "LIST", OpMap: "MAP", OpTuple: "TUPLE", OpTag: "TAG", OpRange: "RANGE",
	OpIndex: "INDEX", OpSlice: "SLICE", OpField: "FIELD", OpIn: "IN",
	OpUnwrapAbort: "UNWRAPABORT", OpUnwrapTest: "UNWRAPTEST", OpCoalesce: "COALESCE",
	OpMatch: "MATCH", OpMatchOrFail: "MATCHORFAIL", OpNoMatch: "NOMATCH",
	OpImport: "IMPORT", OpAssert: "ASSERT", OpTest: "TEST",
}

func (o Op) String() string {
	if int(o) < len(opNames) && opNames[o] != "" {
		return opNames[o]
	}
	return fmt.Sprintf("Op(%d)", uint8(o))
}

// HasArg reports whether the operand is meaningful for this opcode.
func (o Op) HasArg() bool {
	switch o {
	case OpConst, OpJump, OpJumpIfFalse, OpCall,
		OpLoadLocal, OpStoreLocal, OpLoadGlobal, OpStoreGlobal, OpConcat,
		OpClosure, OpLoadCaptured, OpTailCall, OpJumpIfSet, OpCheckType, OpCheckReturn,
		OpList, OpMap, OpTuple, OpTag, OpRange, OpSlice, OpField, OpUnwrapTest, OpCoalesce, OpMatch, OpMatchOrFail,
		OpImport, OpAssert, OpTest:
		return true
	}
	return false
}

type Instr uint32

const argMask = 1<<24 - 1

func Encode(op Op, arg int) Instr { return Instr(uint32(op)<<24 | uint32(arg)&argMask) }
func (i Instr) Op() Op            { return Op(i >> 24) }
func (i Instr) Arg() int          { return int(i & argMask) }
