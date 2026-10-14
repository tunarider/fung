package compiler

import (
	"strings"
	"testing"
)

func TestCompileCollectionsAndOps(t *testing.T) {
	cases := map[string]string{
		"[1, 2]":           "CONST(0) CONST(1) LIST(2) RETURN",
		"{a: 1}":           "CONST(0) CONST(1) MAP(1) RETURN",
		"(1,)":             "CONST(0) TUPLE(1) RETURN",
		"Ok(1)":            "CONST(0) CONST(1) TAG(1) RETURN",
		"0..=3":            "CONST(0) CONST(1) RANGE(1) RETURN",
		"xs = []\nxs[0]":   "LIST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(0) INDEX RETURN",
		"xs = []\nxs[1..]": "LIST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(0) SLICE(1) RETURN",
		"xs = []\nxs[..2]": "LIST(0) STOREGLOBAL(0) LOADGLOBAL(0) CONST(0) SLICE(2) RETURN",
		"m = {}\nm.k":      "MAP(0) STOREGLOBAL(0) LOADGLOBAL(0) FIELD(0) RETURN",
		"1 in [1]":         "CONST(0) CONST(0) LIST(1) IN RETURN",
		"Ok(1)!":           "CONST(0) CONST(1) TAG(1) UNWRAPABORT RETURN",
		"nil ?? 1":         "CONST(0) COALESCE(3) JUMP(4) CONST(1) RETURN",
	}
	for src, want := range cases {
		if got := ops(compile(t, src)); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestCompileQuestionInsideFunction(t *testing.T) {
	p := compile(t, "fn f(r) = r? + 1")
	// r; UNWRAPTEST L1; JUMP L2; L1: RETURN; L2: CONST 1; ADD; RETURN
	if got, want := ops(p.Protos[0]), "LOADLOCAL(0) UNWRAPTEST(3) JUMP(4) RETURN CONST(0) ADD RETURN"; got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	p = compile(t, "fn f(r) -> int = r?")
	if got := ops(p.Protos[0]); !strings.Contains(got, "UNWRAPTEST(3) JUMP(5) CHECKRETURN(0) RETURN") {
		t.Fatalf("annotated: %s", got)
	}
}

func TestCompileMatchAndDestructuring(t *testing.T) {
	p := compile(t, "match 1:\n  0 => \"zero\"\n  n when n > 0 => \"pos\"\n  _ => \"other\"\n;;")
	got := ops(p)
	// Shape: subject stored; each arm loads it, matches, jumps on failure.
	for _, frag := range []string{
		"CONST(0) STORELOCAL(0) LOADLOCAL(0) MATCH(0) JUMPIFFALSE(",
		"LOADLOCAL(0) MATCH(1) JUMPIFFALSE(",
		"LOADLOCAL(1) CONST(2) GT JUMPIFFALSE(",
		"LOADLOCAL(0) NOMATCH",
	} {
		if !strings.Contains(got, frag) {
			t.Fatalf("missing %q in %s", frag, got)
		}
	}
	if strings.Count(got, "JUMP(") != 3 || len(p.Patterns) != 3 || p.Patterns[1].String() != "$1" || p.Patterns[0].String() != "0" {
		t.Fatalf("got %s patterns %v", got, p.Patterns)
	}
	p = compile(t, "(a, b) = (1, 2)")
	if got, want := ops(p), "CONST(0) CONST(1) TUPLE(2) MATCHORFAIL(0) CONST(2) RETURN"; got != want || p.Patterns[0].String() != "(@0, @1)" {
		t.Fatalf("got %s %v", got, p.Patterns)
	}
}

func TestCompileRestParam(t *testing.T) {
	p := compile(t, "fn f(a, ...xs) = xs")
	if !p.Protos[0].Params[1].Rest {
		t.Fatal("Rest flag not copied")
	}
}
