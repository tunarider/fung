package resolve

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/desugar"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/tree"
)

func coreOf(t *testing.T, src string) *tree.Node {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatal(err)
	}
	s, perr := parser.Parse(toks)
	if perr != nil {
		t.Fatal(perr)
	}
	c, derr := desugar.Desugar(s)
	if derr != nil {
		t.Fatal(derr)
	}
	return c
}

func res(t *testing.T, src string) (Result, []*diag.Error) {
	t.Helper()
	return Resolve(coreOf(t, src), []string{"print", "fail"}, Options{})
}

func TestResolveMarksPreludeNames(t *testing.T) {
	r, errs := res(t, "print(1)")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if got := r.Core.String(); got != "(do (call (prelude print) 1))" {
		t.Fatalf("got %s", got)
	}
}

func TestResolveCollectsAllUndefinedNames(t *testing.T) {
	_, errs := res(t, "print(fiels)\nprint(x + y)")
	if len(errs) != 3 {
		t.Fatalf("errs = %v, want 3", errs)
	}
	for i, want := range []string{"fiels", "x", "y"} {
		if errs[i].Code != "E0204" || errs[i].Args[0] != want {
			t.Errorf("errs[%d] = %v, want E0204 %s", i, errs[i], want)
		}
	}
	if errs[1].Pos.Line != 2 || errs[1].Pos.Col != 7 {
		t.Errorf("errs[1].Pos = %v", errs[1].Pos)
	}
}

func TestResolveDoesNotMutateInput(t *testing.T) {
	in := coreOf(t, "print(1)")
	before := in.String()
	Resolve(in, []string{"print"}, Options{})
	if in.String() != before {
		t.Fatalf("input mutated: %s", in.String())
	}
}

func TestTopLevelNamesBecomeGlobals(t *testing.T) {
	r, errs := res(t, "x = 1\ny = x + 1\nx = x * 2\nprint(x, y)")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (let (global 0) 1) (let (global 1) (op + (global 0) 1)) (let (global 0) (op * (global 0) 2)) (call (prelude print) (global 0) (global 1)))"
	if got := r.Core.String(); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if r.NumLocals != 0 || len(r.Globals) != 2 || r.Globals[0] != "x" || r.Globals[1] != "y" {
		t.Fatalf("locals=%d globals=%v", r.NumLocals, r.Globals)
	}
}

func TestBlockNamesBecomeSlotsAndAreReused(t *testing.T) {
	r, errs := res(t, "if true: a = 1; b = a ;;\nif true: c = 2 ;;")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (if true (do (let (local 0) 1) (let (local 1) (local 0))) (nil)) (if true (do (let (local 0) 2)) (nil)))"
	if got := r.Core.String(); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if r.NumLocals != 2 {
		t.Fatalf("NumLocals = %d, want 2", r.NumLocals)
	}
}

func TestNestedBlocksStackSlots(t *testing.T) {
	r, _ := res(t, "if true: a = 1; if true: b = a ;; ;;")
	if r.NumLocals != 2 || !strings.Contains(r.Core.String(), "(let (local 1) (local 0))") {
		t.Fatalf("locals=%d core=%s", r.NumLocals, r.Core)
	}
}

func TestReassigningOuterNameInBlockIsE0205(t *testing.T) {
	_, errs := res(t, "total = 0\nif true: total = total + 1 ;;")
	if len(errs) != 1 || errs[0].Code != "E0205" || errs[0].Args[0] != "total" || errs[0].Len != 5 {
		t.Fatalf("errs = %v", errs)
	}
	if errs[0].Pos.Line != 2 || errs[0].Pos.Col != 10 {
		t.Fatalf("pos = %v", errs[0].Pos)
	}
	// Block-local names may be reassigned inside their own block.
	if _, errs := res(t, "if true: n = 1; n = n + 1 ;;"); len(errs) != 0 {
		t.Fatalf("same-scope rebinding: %v", errs)
	}
	// And a local may not be reassigned from a nested block either.
	if _, errs := res(t, "if true: n = 1; if true: n = 2 ;; ;;"); len(errs) != 1 || errs[0].Code != "E0205" {
		t.Fatalf("nested: %v", errs)
	}
}

func TestUsingBlockNameOutsideIsE0206(t *testing.T) {
	_, errs := res(t, "if true: y = 1 ;;\nprint(y)")
	if len(errs) != 1 || errs[0].Code != "E0206" || errs[0].Args[0] != "y" {
		t.Fatalf("errs = %v", errs)
	}
}

func TestUseBeforeAssignmentIsUndefined(t *testing.T) {
	_, errs := res(t, "print(x)\nx = 1")
	if len(errs) != 1 || errs[0].Code != "E0204" {
		t.Fatalf("errs = %v", errs)
	}
	// The right-hand side is resolved before the name is bound.
	if _, errs := res(t, "x = x + 1"); len(errs) != 1 || errs[0].Code != "E0204" {
		t.Fatalf("self-reference: %v", errs)
	}
}

func TestUndefinedNameSuggestsSimilarName(t *testing.T) {
	_, errs := res(t, "files = 1\nprint(fiels)")
	if len(errs) != 1 || errs[0].Help != "a similar name exists: `files`" || errs[0].Len != 5 {
		t.Fatalf("errs = %+v", errs)
	}
	_, errs = res(t, "print(prnt(1))")
	if len(errs) != 1 || errs[0].Help != "a similar name exists: `print`" {
		t.Fatalf("prelude suggestion: %+v", errs)
	}
	_, errs = res(t, "print(zzzzzz)")
	if len(errs) != 1 || errs[0].Help != "" {
		t.Fatalf("no suggestion expected: %+v", errs)
	}
}

func TestSimilarNameThresholdScalesWithLength(t *testing.T) {
	cases := map[string]string{
		"x = 1\nprint(zz)":           "",                               // short names: no guessing
		"ab = 1\nprint(ac)":          "",                               // 2 letters: still none
		"abc = 1\nprint(abd)":        "a similar name exists: `abc`",   // 3-4 letters: one edit
		"abcd = 1\nprint(abxy)":      "",                               // two edits is too far at 4
		"print(fiels)":               "",                               // nothing similar in prelude
		"files = 1\nprint(fiels)":    "a similar name exists: `files`", // transposition counts as one edit
		"counter = 1\nprint(countr)": "a similar name exists: `counter`",
	}
	for src, want := range cases {
		_, errs := res(t, src)
		if len(errs) != 1 || errs[0].Help != want {
			t.Errorf("%q: errs = %+v, want help %q", src, errs, want)
		}
	}
}
