package compiler

import (
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/desugar"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/resolve"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

var prelude = map[string]value.Value{"print": &value.Native{Name: "print"}}

func compile(t *testing.T, src string) *bytecode.Proto {
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
	r, errs := resolve.Resolve(c, []string{"print"}, resolve.Options{})
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	p, cerr := Compile(r, prelude, Options{})
	if cerr != nil {
		t.Fatal(cerr)
	}
	return p
}

// compileErr runs the front end and returns the compiler's error, if any.
func compileErr(t *testing.T, src string) (*bytecode.Proto, *diag.Error) {
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
	r, errs := resolve.Resolve(c, []string{"print"}, resolve.Options{})
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	return Compile(r, prelude, Options{})
}

// ops strips positions for shape comparisons: "CONST(0) ADD RETURN".
func ops(p *bytecode.Proto) string {
	var parts []string
	for _, in := range p.Code {
		s := in.Op().String()
		if in.Op().HasArg() {
			s += "(" + strconv.Itoa(in.Arg()) + ")"
		}
		parts = append(parts, s)
	}
	return strings.Join(parts, " ")
}

func TestCompileArithmetic(t *testing.T) {
	p := compile(t, "1 + 2 * 3")
	if got, want := ops(p), "CONST(0) CONST(1) CONST(2) MUL ADD RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
	if len(p.Consts) != 3 || p.Consts[2] != value.Int(3) {
		t.Fatalf("consts = %v", p.Consts)
	}
}

func TestCompileStatementsPopAllButLast(t *testing.T) {
	p := compile(t, "1\n2")
	if got, want := ops(p), "CONST(0) POP CONST(1) RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func TestCompileEmptyProgramReturnsNil(t *testing.T) {
	p := compile(t, "")
	if got, want := ops(p), "CONST(0) RETURN"; got != want || p.Consts[0] != (value.Nil{}) {
		t.Fatalf("got %s consts %v", got, p.Consts)
	}
}

func TestCompileCallAndPrelude(t *testing.T) {
	p := compile(t, "print(1, 2)")
	if got, want := ops(p), "CONST(0) CONST(1) CONST(2) CALL(2) RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
	if p.Consts[0] != prelude["print"] {
		t.Fatalf("callee const = %v", p.Consts[0])
	}
}

func TestCompileIfWithJumps(t *testing.T) {
	p := compile(t, "true and false")
	// (if true (op bool false) false)
	want := "CONST(0) JUMPIFFALSE(5) CONST(1) CHECKBOOL JUMP(6) CONST(1) RETURN"
	if got := ops(p); got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func TestCompileUnaryAndComparison(t *testing.T) {
	p := compile(t, "not (1 < 2)")
	if got, want := ops(p), "CONST(0) CONST(1) LT NOT RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
	p = compile(t, "-1")
	if got, want := ops(p), "CONST(0) NEG RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func TestCompileRecordsPositions(t *testing.T) {
	p := compile(t, "1 + 2")
	// CONST 1 at col 1, CONST 2 at col 5, ADD at col 3 (the operator)
	if p.Pos[0].Col != 1 || p.Pos[1].Col != 5 || p.Pos[2].Col != 3 {
		t.Fatalf("pos = %v", p.Pos)
	}
}

func TestCompileRejectsUnknownCoreOperator(t *testing.T) {
	bad := tree.WithLit("op", diag.Pos{}, "@@", tree.Leaf("int", diag.Pos{}, int64(1)), tree.Leaf("int", diag.Pos{}, int64(2)))
	_, err := Compile(resolve.Result{Core: tree.New("do", diag.Pos{}, bad)}, prelude, Options{})
	if err == nil || err.Code != "E0801" {
		t.Fatalf("err = %v, want E0801", err)
	}
	badUnary := tree.WithLit("op", diag.Pos{}, "@@", tree.Leaf("int", diag.Pos{}, int64(1)))
	_, err = Compile(resolve.Result{Core: tree.New("do", diag.Pos{}, badUnary)}, prelude, Options{})
	if err == nil || err.Code != "E0801" {
		t.Fatalf("unary: err = %v, want E0801", err)
	}
}

func TestCompileRejectsUnknownPreludeName(t *testing.T) {
	core := tree.New("do", diag.Pos{}, tree.Leaf("prelude", diag.Pos{}, "nope"))
	_, err := Compile(resolve.Result{Core: core}, prelude, Options{})
	if err == nil || err.Code != "E0801" {
		t.Fatalf("err = %v, want E0801", err)
	}
}

func TestCompileLetAndLoads(t *testing.T) {
	p := compile(t, "x = 1\ny = x\nif true: a = y; a ;;")
	want := "CONST(0) STOREGLOBAL(0) LOADGLOBAL(0) STOREGLOBAL(1) CONST(1) JUMPIFFALSE(10) LOADGLOBAL(1) STORELOCAL(0) LOADLOCAL(0) JUMP(11) CONST(2) RETURN"
	if got := ops(p); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if p.NumLocals != 1 || len(p.GlobalNames) != 2 || p.GlobalNames[1] != "y" {
		t.Fatalf("locals=%d globals=%v", p.NumLocals, p.GlobalNames)
	}
	if p.Consts[2] != (value.Nil{}) {
		t.Fatalf("expected nil const for the statement-valued program: %v", p.Consts)
	}
}

func TestCompileBlockEndingInStatementYieldsNil(t *testing.T) {
	p := compile(t, "x = 1")
	if got, want := ops(p), "CONST(0) STOREGLOBAL(0) CONST(1) RETURN"; got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func TestCompileLiteralsAndConcat(t *testing.T) {
	p := compile(t, `"a${1}b" + 'ok`)
	want := "CONST(0) CONST(1) CONST(2) CONCAT(3) CONST(3) ADD RETURN"
	if got := ops(p); got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
	if p.Consts[0] != value.Str("a") || p.Consts[3] != value.Intern("ok") {
		t.Fatalf("consts = %v", p.Consts)
	}
	p = compile(t, "nil\n1h")
	if got, want := ops(p), "CONST(0) POP CONST(1) RETURN"; got != want || p.Consts[1] != value.Dur(time.Hour) {
		t.Fatalf("got %s consts %v", got, p.Consts)
	}
}

func TestJumpIfFalseCarriesConditionPosition(t *testing.T) {
	p := compile(t, "1 and true")
	// CONST 1 at col 1; JUMPIFFALSE should point at the condition (col 1), not the operator (col 3)
	if p.Pos[1].Col != 1 {
		t.Fatalf("JUMPIFFALSE pos = %v, want col 1", p.Pos[1])
	}
}
