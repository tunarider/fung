package tree

import (
	"testing"
	"time"

	"github.com/tunarider/fung/internal/diag"
)

func TestStringFormatsSExpressions(t *testing.T) {
	p := diag.Pos{}
	one := Leaf("int", p, int64(1))
	two := Leaf("int", p, int64(2))
	three := Leaf("int", p, int64(3))
	mul := WithLit("binary", p, "*", two, three)
	add := WithLit("binary", p, "+", one, mul)
	call := New("call", p, Leaf("name", p, "print"), add)
	if got, want := call.String(), "(call print (binary + 1 (binary * 2 3)))"; got != want {
		t.Fatalf("String() = %q, want %q", got, want)
	}
}

func TestStringFormatsLeaves(t *testing.T) {
	p := diag.Pos{}
	cases := map[*Node]string{
		Leaf("float", p, 2.5):                "2.5",
		Leaf("float", p, 1.0):                "1.0",
		Leaf("bool", p, true):                "true",
		Leaf("prelude", p, "print"):          "(prelude print)",
		Leaf("string", p, "a b"):             `"a b"`,
		New("do", p):                         "(do)",
		New("call", p, Leaf("name", p, "f")): "(call f)",
	}
	for n, want := range cases {
		if got := n.String(); got != want {
			t.Errorf("%+v: String() = %q, want %q", n, got, want)
		}
	}
}

func TestLeavesUseValueFormatting(t *testing.T) {
	p := diag.Pos{}
	cases := map[*Node]string{
		Leaf("float", p, 1e21):              "1e21",
		Leaf("float", p, 1e6):               "1000000.0",
		Leaf("symbol", p, "ok"):             "'ok",
		Leaf("duration", p, 90*time.Minute): "1h30m",
		Leaf("string", p, "tab\t${"):        `"tab\t\${"`,
		New("nil", p):                       "(nil)",
		New("assign", p, Leaf("name", p, "x"), Leaf("int", p, int64(1))): "(assign x 1)",
	}
	for n, want := range cases {
		if got := n.String(); got != want {
			t.Errorf("String() = %q, want %q", got, want)
		}
	}
}

func TestFnInfoString(t *testing.T) {
	cases := map[*FnInfo]string{
		{Name: "add", Params: []Param{{Name: "a"}, {Name: "b", HasDefault: true}}}:                       "add(a, b=?)",
		{Params: []Param{{Name: "x"}}}:                                                                   "(x)",
		{Name: "f", Params: []Param{{Name: "p", Type: []string{"int", "nil"}}}, Return: []string{"str"}}: "f(p:int|nil)->str",
		{Name: "g", Params: []Param{{Name: "xs", Rest: true}}}:                                           "g(...xs)",
		{Name: "h"}: "h()",
	}
	for info, want := range cases {
		if got := info.String(); got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	}
	n := WithLit("fn", diag.Pos{}, &FnInfo{Name: "id", Params: []Param{{Name: "x"}}}, New("do", diag.Pos{}, Leaf("name", diag.Pos{}, "x")))
	if got := n.String(); got != "(fn id(x) (do x))" {
		t.Errorf("node = %s", got)
	}
}
