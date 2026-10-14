package compiler

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/desugar"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/resolve"
)

var fsImports = map[string]resolve.ModuleInfo{"fs": {Key: "fs", Name: "fs", Exports: []string{"read"}}}

func compileWithImports(t *testing.T, src string, imports map[string]resolve.ModuleInfo) string {
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
	r, errs := resolve.Resolve(c, []string{"print"}, resolve.Options{Imports: imports})
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	p, cerr := Compile(r, prelude, Options{})
	if cerr != nil {
		t.Fatal(cerr)
	}
	return ops(p)
}

func TestCompileImport(t *testing.T) {
	got := compileWithImports(t, "import fs\nprint(fs.read)", fsImports)
	want := "IMPORT(0) STOREGLOBAL(0) CONST(1) LOADGLOBAL(0) FIELD(2) CALL(1) RETURN"
	if got != want {
		t.Errorf("import fs:\n got %s\nwant %s", got, want)
	}
	got = compileWithImports(t, "import fs (read)\nread", fsImports)
	want = "IMPORT(0) FIELD(1) STOREGLOBAL(0) LOADGLOBAL(0) RETURN"
	if got != want {
		t.Errorf("import fs (read):\n got %s\nwant %s", got, want)
	}
	got = compileWithImports(t, "import fs", fsImports)
	if !strings.HasSuffix(got, "STOREGLOBAL(0) CONST(1) RETURN") {
		t.Errorf("import as last statement leaves nil: %s", got)
	}
}
