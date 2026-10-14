package compiler

import (
	"strconv"
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/value"
)

func TestCompileFunctionProtoAndClosure(t *testing.T) {
	p := compile(t, "fn add(a, b) = a + b\nprint(add(1, 2))")
	if got, want := ops(p), "CLOSURE(0) STOREGLOBAL(0) CONST(0) LOADGLOBAL(0) CONST(1) CONST(2) CALL(2) CALL(1) RETURN"; got != want {
		t.Fatalf("main:\n got %s\nwant %s", got, want)
	}
	add := p.Protos[0]
	if add.Name != "add" || add.NumLocals != 2 || len(add.Params) != 2 || add.ReturnType != -1 {
		t.Fatalf("add proto = %+v", add)
	}
	if got, want := ops(add), "LOADLOCAL(0) LOADLOCAL(1) ADD RETURN"; got != want {
		t.Fatalf("add:\n got %s\nwant %s", got, want)
	}
}

func TestCompileDefaultsAndTypeChecks(t *testing.T) {
	p := compile(t, "fn f(a: int, b = a * 2) -> num = a + b")
	f := p.Protos[0]
	// JUMPIFSET skips the default expression and its store (index 6).
	want := "LOADLOCAL(1) JUMPIFSET(6) LOADLOCAL(0) CONST(0) MUL STORELOCAL(1) CHECKTYPE(0) LOADLOCAL(0) LOADLOCAL(1) ADD CHECKRETURN(1) RETURN"
	if got := ops(f); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if f.Types[0][0] != "int" || f.Types[1][0] != "num" || f.ReturnType != 1 || f.Params[0].Type != 0 || f.Params[1].Type != -1 {
		t.Fatalf("types = %v return = %d params = %+v", f.Types, f.ReturnType, f.Params)
	}
}

func TestCompileTailCalls(t *testing.T) {
	p := compile(t, "fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)")
	if got := ops(p.Protos[0]); !strings.Contains(got, "TAILCALL(2)") || strings.Contains(got, " CALL(") {
		t.Fatalf("expected a tail call: %s", got)
	}
	// Not tail: operand of +, with a return type.
	for _, src := range []string{"fn f(n) = 1 + f(n)", "fn f(n) -> int = f(n)"} {
		p := compile(t, src)
		if got := ops(p.Protos[0]); strings.Contains(got, "TAILCALL") {
			t.Errorf("%q compiled a tail call: %s", src, got)
		}
	}
	// An argument is not in tail position, but the outer call is.
	p = compile(t, "fn f(n) = print(f(n))")
	if got := ops(p.Protos[0]); !strings.Contains(got, "LOADSELF LOADLOCAL(0) CALL(1) TAILCALL(1)") {
		t.Fatalf("got %s", got)
	}
	// return of a call is a tail call; the branches of if are tail positions.
	p = compile(t, "fn f(n):\n  if n > 0: return f(n - 1) ;;\n  if n < 0: f(0) else: 0 ;;\n;;")
	if got := ops(p.Protos[0]); strings.Count(got, "TAILCALL(1)") != 2 {
		t.Fatalf("got %s", got)
	}
}

func TestCompileCapturesSelfAndNamedArgs(t *testing.T) {
	p := compile(t, "fn outer(y):\n  fn loop(n) = n == 0 then y else loop(n - 1)\n  loop(2)\n;;\nouter(1)")
	outer := p.Protos[0]
	loop := outer.Protos[0]
	if len(loop.Captures) != 1 || loop.Captures[0].Index != 0 {
		t.Fatalf("loop captures = %+v", loop.Captures)
	}
	if got := ops(loop); !strings.Contains(got, "LOADCAPTURED(0)") || !strings.Contains(got, "LOADSELF") {
		t.Fatalf("loop = %s", got)
	}
	p = compile(t, "fn f(a, b = 2) = a\nf(1, b: 3)")
	// Stack at the call: f, 1, 'b, 3 — the name symbol is constant 1.
	if got := ops(p); !strings.Contains(got, "CONST(1) CONST(2) CALL("+strconv.Itoa(bytecode.EncodeCall(1, 1))+")") || p.Consts[1] != value.Intern("b") {
		t.Fatalf("named call: %s consts %v", got, p.Consts)
	}
}

func TestTooManyArgumentsOrParametersIsACompileError(t *testing.T) {
	args := strings.TrimSuffix(strings.Repeat("1, ", 4096), ", ")
	if _, err := compileErr(t, "print("+args+")"); err == nil || err.Code != "E0806" {
		t.Fatalf("4096 args: err = %v, want E0806", err)
	}
	var names []string
	for i := 0; i < 4096; i++ {
		names = append(names, "p"+strconv.Itoa(i))
	}
	if _, err := compileErr(t, "fn f("+strings.Join(names, ", ")+") = 1"); err == nil || err.Code != "E0806" {
		t.Fatalf("4096 params: err = %v, want E0806", err)
	}
	ok := strings.TrimSuffix(strings.Repeat("1, ", 4095), ", ")
	if _, err := compileErr(t, "print("+ok+")"); err != nil {
		t.Fatalf("4095 args should compile: %v", err)
	}
}

func TestCheckTypeCarriesTheParameterPosition(t *testing.T) {
	p := compile(t, "fn f(a,\n  b: int) = b")
	f := p.Protos[0]
	for i, in := range f.Code {
		if in.Op() == bytecode.OpCheckType {
			if f.Pos[i].Line != 2 || f.Pos[i].Col != 3 {
				t.Fatalf("CHECKTYPE pos = %v, want 2:3 (the parameter)", f.Pos[i])
			}
			return
		}
	}
	t.Fatal("no CHECKTYPE emitted")
}
