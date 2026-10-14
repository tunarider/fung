package bytecode

import (
	"testing"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

func TestPatternString(t *testing.T) {
	p := &Pattern{Kind: PatAlt, Kids: []*Pattern{
		{Kind: PatTag, Name: "Ok", Kids: []*Pattern{{Kind: PatBind, Index: 1}}},
		{Kind: PatList, Kids: []*Pattern{{Kind: PatLit, Lit: value.Int(1)}}, Rest: &Pattern{Kind: PatWild}},
		{Kind: PatMap, Keys: []string{"k"}, Kids: []*Pattern{{Kind: PatBind, Global: true, Index: 2}}},
		{Kind: PatTuple, Kids: []*Pattern{{Kind: PatWild}, {Kind: PatLit, Lit: value.Str("s")}}},
	}}
	if got := p.String(); got != `Ok($1) | [1, ...] | {k: @2} | (_, "s")` {
		t.Fatalf("got %s", got)
	}
}

func TestM4OpsDisassemble(t *testing.T) {
	p := &Proto{Name: "<main>", ReturnType: -1, Patterns: []*Pattern{{Kind: PatBind, Index: 0}}}
	pos := diag.Pos{Line: 1, Col: 1}
	p.Emit(OpList, 2, pos)
	p.Emit(OpTag, 1, pos)
	p.Emit(OpSlice, 3, pos)
	p.Emit(OpField, p.Const(value.Str("name")), pos)
	p.Emit(OpMatch, 0, pos)
	p.Emit(OpRange, 1, pos)
	p.Emit(OpIn, 0, pos)
	want := "== <main> ==\n" +
		"0000  1:1   LIST         2\n" +
		"0001  1:1   TAG          1    ; 1 args\n" +
		"0002  1:1   SLICE        3    ; start, end\n" +
		"0003  1:1   FIELD        0    ; name\n" +
		"0004  1:1   MATCH        0    ; $0\n" +
		"0005  1:1   RANGE        1    ; inclusive\n" +
		"0006  1:1   IN\n"
	if got := Disassemble(p); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}
