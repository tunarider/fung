package compiler

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/desugar"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/resolve"
	"github.com/tunarider/fung/internal/value"
)

var m6Prelude = map[string]value.Value{
	"print": &value.Native{Name: "print"}, "dbg": &value.Native{Name: "dbg"},
	"f": &value.Native{Name: "f"}, "ok": &value.Native{Name: "ok"}, "map": &value.Native{Name: "map"},
}

func compileOpts(t *testing.T, src string, opts Options) *bytecode.Proto {
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
	r, errs := resolve.Resolve(c, []string{"print", "dbg", "f", "ok", "map"}, resolve.Options{})
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	p, cerr := Compile(r, m6Prelude, opts)
	if cerr != nil {
		t.Fatal(cerr)
	}
	return p
}

func strConsts(p *bytecode.Proto) []string {
	var out []string
	for _, c := range p.Consts {
		if s, ok := c.(value.Str); ok {
			out = append(out, string(s))
		}
	}
	return out
}

func hasStr(xs []string, s string) bool {
	for _, x := range xs {
		if x == s {
			return true
		}
	}
	return false
}

func TestCompileAssert(t *testing.T) {
	src := "assert f(1) == 2"
	p := compileOpts(t, src, Options{Source: src})
	if got := ops(p); got != "CONST(0) CONST(1) CALL(1) CONST(2) ASSERT(0) CONST(3) RETURN" {
		t.Errorf("ops = %s", got)
	}
	a := p.Asserts[0]
	if a.Text != "f(1) == 2" || a.Left != "f(1)" || a.Right != "" || a.Op != "==" {
		t.Errorf("assert = %+v (literal sides are blank)", a)
	}
	src = "assert ok(1)"
	p = compileOpts(t, src, Options{Source: src})
	if a := p.Asserts[0]; a.Text != "ok(1)" || a.Op != "" {
		t.Errorf("unary assert = %+v", a)
	}
	src = "fn g(x):\n  assert x > 0\n  x\n;;"
	p = compileOpts(t, src, Options{Source: src})
	if a := p.Protos[0].Asserts[0]; a.Text != "x > 0" || a.Left != "x" || a.Op != ">" {
		t.Errorf("assert inside a fn uses that proto's table: %+v", a)
	}
}

func TestCompileTestBlocks(t *testing.T) {
	src := "test \"a\": assert true ;;\nprint(1)"
	p := compileOpts(t, src, Options{Source: src, Tests: true})
	if got := ops(p); !strings.HasPrefix(got, "CLOSURE(0) TEST(0) ") {
		t.Errorf("with Tests: %s", got)
	}
	if p.Protos[0].Name != "test: a" {
		t.Errorf("test proto name = %q", p.Protos[0].Name)
	}
	p = compileOpts(t, src, Options{Source: src})
	if got := ops(p); strings.Contains(got, "TEST") || strings.Contains(got, "CLOSURE") {
		t.Errorf("without Tests the block compiles to nothing: %s", got)
	}
	src = "test \"only\": assert true ;;"
	p = compileOpts(t, src, Options{Source: src})
	if got := ops(p); got != "CONST(0) RETURN" {
		t.Errorf("a file of only tests is an empty program: %s", got)
	}
}

func TestCompileDbgAppendsTextAndPlace(t *testing.T) {
	src := "xs = [1]\nxs |> map(x => x) |> dbg"
	p := compileOpts(t, src, Options{Source: src})
	if got := ops(p); !strings.Contains(got, "CALL(3) ") {
		t.Errorf("dbg gets three positional args: %s", got)
	}
	if texts := strConsts(p); !hasStr(texts, "xs |> map(x => x)") || !hasStr(texts, "t.fung:2") {
		t.Errorf("consts = %v", texts)
	}
	src = "d = dbg\nd(1)"
	p = compileOpts(t, src, Options{Source: src})
	if strings.Contains(ops(p), "CALL(3)") {
		t.Error("an aliased dbg is a plain call")
	}
	src = "dbg(1, text: \"t\")"
	p = compileOpts(t, src, Options{Source: src})
	if strings.Contains(ops(p), "CALL(3)") {
		t.Error("named arguments disable the rewrite")
	}
}
