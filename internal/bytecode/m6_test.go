package bytecode

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

func TestAssertAndTestOpcodes(t *testing.T) {
	for _, op := range []Op{OpAssert, OpTest} {
		if !op.HasArg() {
			t.Errorf("%s needs an argument", op)
		}
	}
	if OpAssert.String() != "ASSERT" || OpTest.String() != "TEST" {
		t.Fatal("names")
	}
	p := &Proto{}
	p.Asserts = append(p.Asserts, Assert{Text: "a == b", Left: "a", Right: "b", Op: "=="})
	p.Emit(OpAssert, 0, diag.Pos{Line: 1, Col: 1})
	p.Emit(OpTest, p.Const(value.Str("name")), diag.Pos{Line: 1, Col: 1})
	s := Disassemble(p)
	if !strings.Contains(s, "ASSERT       0    ; a == b") || !strings.Contains(s, "TEST         0    ; \"name\"") {
		t.Fatalf("disasm:\n%s", s)
	}
}
