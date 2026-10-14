package bytecode

import (
	"strconv"
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

func TestEncodeRoundTrip(t *testing.T) {
	for _, c := range []struct {
		op  Op
		arg int
	}{{OpConst, 0}, {OpJump, 1<<24 - 1}, {OpCall, 3}, {OpAdd, 0}} {
		in := Encode(c.op, c.arg)
		if in.Op() != c.op || in.Arg() != c.arg {
			t.Errorf("Encode(%v, %d) round-trips to (%v, %d)", c.op, c.arg, in.Op(), in.Arg())
		}
	}
}

func TestProtoEmitPatchAndConstDedup(t *testing.T) {
	p := &Proto{Name: "<main>"}
	pos := diag.Pos{File: "t.fung", Line: 1, Col: 1}
	i0 := p.Const(value.Int(1))
	i1 := p.Const(value.Int(1))
	i2 := p.Const(value.Float(1))
	if i0 != 0 || i1 != 0 || i2 != 1 {
		t.Fatalf("Const indexes = %d %d %d", i0, i1, i2)
	}
	at := p.Emit(OpJumpIfFalse, 0, pos)
	p.Emit(OpConst, i0, pos)
	p.Patch(at, 2)
	if p.Code[0].Arg() != 2 || len(p.Pos) != 2 || p.Pos[1] != pos {
		t.Fatalf("code = %v, pos = %v", p.Code, p.Pos)
	}
}

func TestDisassemble(t *testing.T) {
	p := &Proto{Name: "<main>"}
	pos := diag.Pos{File: "t.fung", Line: 1, Col: 7}
	p.Emit(OpConst, p.Const(value.Int(1)), pos)
	p.Emit(OpConst, p.Const(value.Int(2)), diag.Pos{File: "t.fung", Line: 1, Col: 11})
	p.Emit(OpAdd, 0, diag.Pos{File: "t.fung", Line: 1, Col: 9})
	p.Emit(OpReturn, 0, pos)
	want := "== <main> ==\n" +
		"0000  1:7   CONST        0    ; 1\n" +
		"0001  1:11  CONST        1    ; 2\n" +
		"0002  1:9   ADD\n" +
		"0003  1:7   RETURN\n"
	if got := Disassemble(p); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}

func TestNewOpsHaveArgsAndNames(t *testing.T) {
	for _, op := range []Op{OpLoadLocal, OpStoreLocal, OpLoadGlobal, OpStoreGlobal, OpConcat} {
		if !op.HasArg() || strings.HasPrefix(op.String(), "Op(") {
			t.Errorf("%v: HasArg=%v name=%q", op, op.HasArg(), op.String())
		}
	}
}

func TestDisassembleShowsGlobalNames(t *testing.T) {
	p := &Proto{Name: "<main>", GlobalNames: []string{"x"}}
	pos := diag.Pos{Line: 1, Col: 1}
	p.Emit(OpConst, p.Const(value.Int(1)), pos)
	p.Emit(OpStoreGlobal, 0, pos)
	p.Emit(OpLoadGlobal, 0, pos)
	p.Emit(OpLoadLocal, 2, pos)
	want := "== <main> ==\n" +
		"0000  1:1   CONST        0    ; 1\n" +
		"0001  1:1   STOREGLOBAL  0    ; x\n" +
		"0002  1:1   LOADGLOBAL   0    ; x\n" +
		"0003  1:1   LOADLOCAL    2\n"
	if got := Disassemble(p); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}

func TestCallEncoding(t *testing.T) {
	for _, c := range [][2]int{{0, 0}, {3, 0}, {2, 1}, {4095, 4095}} {
		p, n := DecodeCall(EncodeCall(c[0], c[1]))
		if p != c[0] || n != c[1] {
			t.Errorf("EncodeCall(%d, %d) → (%d, %d)", c[0], c[1], p, n)
		}
	}
}

func TestClosureIsAFnValue(t *testing.T) {
	c := &Closure{Proto: &Proto{Name: "add"}}
	var v value.Value = c
	if v.TypeName() != "fn" || value.Format(v) != "<fn add>" || !value.Equal(v, v) || value.Equal(v, &Closure{Proto: c.Proto}) {
		t.Fatalf("closure value behaviour: %s", value.Format(v))
	}
	if value.Format(&Closure{Proto: &Proto{}}) != "<fn>" {
		t.Error("anonymous")
	}
}

func TestDisassembleFunctions(t *testing.T) {
	child := &Proto{Name: "add", NumLocals: 2, Params: []Param{{Name: "a", Type: 0}, {Name: "b", Type: -1}}, Types: [][]string{{"int", "nil"}}, ReturnType: -1}
	pos := diag.Pos{Line: 1, Col: 1}
	child.Emit(OpCheckType, 0|0<<12, pos)
	child.Emit(OpLoadLocal, 0, pos)
	child.Emit(OpReturn, 0, pos)
	p := &Proto{Name: "<main>", Protos: []*Proto{child}, ReturnType: -1}
	p.Emit(OpClosure, 0, pos)
	p.Emit(OpConst, p.Const(value.Int(1)), pos)
	p.Emit(OpCall, EncodeCall(1, 0), pos)
	p.Emit(OpTailCall, EncodeCall(0, 2), pos)
	p.Emit(OpLoadSelf, 0, pos)
	want := "== <main> ==\n" +
		"0000  1:1   CLOSURE      0    ; add\n" +
		"0001  1:1   CONST        0    ; 1\n" +
		"0002  1:1   CALL         1    ; 1 positional, 0 named\n" +
		"0003  1:1   TAILCALL     " + strconv.Itoa(EncodeCall(0, 2)) + "    ; 0 positional, 2 named\n" +
		"0004  1:1   LOADSELF\n" +
		"\n== add ==\n" +
		"0000  1:1   CHECKTYPE    0    ; a: int | nil\n" +
		"0001  1:1   LOADLOCAL    0\n" +
		"0002  1:1   RETURN\n"
	if got := Disassemble(p); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}
