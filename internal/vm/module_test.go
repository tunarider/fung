package vm

import (
	"bytes"
	"fmt"
	"testing"

	"github.com/tunarider/fung/internal/frontend"
	"github.com/tunarider/fung/internal/prelude"
	"github.com/tunarider/fung/internal/value"
)

func runProgram(t *testing.T, files map[string]string, entry string) (string, string) {
	t.Helper()
	l := &frontend.Loader{
		Prelude: prelude.Test(),
		Natives: map[string]*value.Module{},
		Stdlib:  func(string) (string, bool) { return "", false },
		ReadFile: func(p string) (string, error) {
			if s, ok := files[p]; ok {
				return s, nil
			}
			return "", fmt.Errorf("no file %s", p)
		},
	}
	prog, errs := l.Load(entry, files[entry])
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	for _, m := range prog.Modules {
		if m.Native != nil {
			rt.RegisterNative(m.Key, m.Native)
		} else {
			rt.Register(m.Key, m.Proto)
		}
	}
	_, err := rt.Run(prog.Entry)
	code := ""
	if err != nil {
		code = err.Code
	}
	return out.String(), code
}

func TestModulesKeepTheirOwnGlobals(t *testing.T) {
	files := map[string]string{
		"/p/a.fung":    "fn helper() = \"a\"\nfn run() = helper()",
		"/p/b.fung":    "import \"./a\"\nfn helper() = \"b\"\nfn run() = helper() + a.run()",
		"/p/main.fung": "import \"./a\"\nimport \"./b\"\nprint(b.run(), a.run())\nf = b.run\nprint(f())",
	}
	out, code := runProgram(t, files, "/p/main.fung")
	if code != "" || out != "ba a\nba\n" {
		t.Fatalf("out=%q code=%s", out, code)
	}
}

func TestModuleTopLevelRunsOnce(t *testing.T) {
	files := map[string]string{
		"/p/m.fung":    "x = print(\"init\")\ny = 1",
		"/p/a.fung":    "import \"./m\"\nv = m.y",
		"/p/main.fung": "import \"./m\"\nimport \"./a\"\nprint(m.y, a.v)",
	}
	out, _ := runProgram(t, files, "/p/main.fung")
	if out != "init\n1 1\n" {
		t.Fatalf("out=%q", out)
	}
}

func TestModuleValueAndErrors(t *testing.T) {
	files := map[string]string{
		"/p/m.fung":    "fn _hidden() = 1\nfn shown() = 2\nz = 3",
		"/p/main.fung": "import \"./m\"\nprint(m, type_of(m), m.shown(), m.z, m == m)\nv = m\nprint(v._hidden)",
	}
	out, code := runProgram(t, files, "/p/main.fung")
	if out != "<module m> 'module 2 3 true\n" || code != "E0304" {
		t.Fatalf("out=%q code=%s", out, code)
	}
	files["/p/main.fung"] = "import \"./m\"\nv = m\nprint(v.nope)"
	if _, code := runProgram(t, files, "/p/main.fung"); code != "E0303" {
		t.Fatalf("dynamic missing member: %s", code)
	}
	files["/p/boom.fung"] = "x = 1 / 0"
	files["/p/main.fung"] = "import \"./boom\""
	if _, code := runProgram(t, files, "/p/main.fung"); code != "E0502" {
		t.Fatalf("error during module init: %s", code)
	}
}

func TestImportedNamesAreNotReExported(t *testing.T) {
	files := map[string]string{
		"/p/a.fung":    "x = 1",
		"/p/b.fung":    "import \"./a\" (x)\ny = x + 1",
		"/p/main.fung": "import \"./b\"\nm = b\nprint(m.y)\nprint(m.x)",
	}
	out, code := runProgram(t, files, "/p/main.fung")
	if out != "2\n" || code != "E0303" {
		t.Fatalf("out=%q code=%s", out, code)
	}
}

func TestMemberImportAndClosureGlobals(t *testing.T) {
	files := map[string]string{
		"/p/m.fung":    "fn base() = 10\nfn add(x) = x + base()",
		"/p/main.fung": "import \"./m\" (add)\nfn base() = 1\nprint(add(5), map([1, 2], add), base())",
	}
	out, code := runProgram(t, files, "/p/main.fung")
	if code != "" || out != "15 [11, 12] 1\n" {
		t.Fatalf("out=%q code=%s", out, code)
	}
}
