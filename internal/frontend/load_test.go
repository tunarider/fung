package frontend

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/prelude"
	"github.com/tunarider/fung/internal/value"
)

// fileNotFound is the error memLoader returns for unknown paths.
type fileNotFound struct{ path string }

func (e *fileNotFound) Error() string { return "no file " + e.path }

// memLoader serves files from a map so tests need no disk.
func memLoader(files map[string]string, stdlib map[string]string) *Loader {
	nfs := value.NewModule("fs", "_native.fs")
	nfs.Set("read", &value.Native{Name: "read"})
	return &Loader{
		Prelude: prelude.Test(),
		Natives: map[string]*value.Module{"_native.fs": nfs},
		Stdlib:  func(name string) (string, bool) { s, ok := stdlib[name]; return s, ok },
		ReadFile: func(path string) (string, error) {
			if s, ok := files[path]; ok {
				return s, nil
			}
			return "", &fileNotFound{path}
		},
	}
}

func TestLoadGraphInDependencyOrder(t *testing.T) {
	files := map[string]string{
		"/p/main.fung":  "import \"./lib/a\"\nimport \"./lib/b\" as bb\nprint(a.x, bb.y)",
		"/p/lib/a.fung": "import \"../lib/b\"\nx = b.y + 1",
		"/p/lib/b.fung": "y = 1",
	}
	l := memLoader(files, nil)
	prog, errs := l.Load("/p/main.fung", files["/p/main.fung"])
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if len(prog.Modules) != 2 || prog.Modules[0].Key != "/p/lib/b.fung" || prog.Modules[1].Key != "/p/lib/a.fung" {
		t.Fatalf("modules = %+v", prog.Modules)
	}
	if prog.Entry == nil || prog.Sources["/p/lib/a.fung"] == "" {
		t.Fatal("entry or sources missing")
	}
	if prog.Modules[1].Proto.Name != "<module a>" {
		t.Errorf("module proto name = %q", prog.Modules[1].Proto.Name)
	}
}

func TestLoadStdlibAndNatives(t *testing.T) {
	stdlib := map[string]string{"fs": "import _native.fs as _n\nread = _n.read"}
	l := memLoader(map[string]string{}, stdlib)
	prog, errs := l.Load("-e", "import fs\nprint(fs.read)")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if len(prog.Modules) != 2 || prog.Modules[0].Key != "_native.fs" || prog.Modules[0].Native == nil || prog.Modules[1].Key != "fs" {
		t.Fatalf("modules = %+v", prog.Modules)
	}
	_, errs = l.Load("-e", "import _native.fs")
	if len(errs) != 1 || errs[0].Code != "E0310" {
		t.Fatalf("user code importing _native: %v", errs)
	}
	stdlib["prelude"] = "fn until(x) = x"
	_, errs = l.Load("-e", "import prelude")
	if len(errs) != 1 || errs[0].Code != "E0301" {
		t.Fatalf("prelude is not a module: %v", errs)
	}
	files := map[string]string{
		"/p/a.fung":    "x = 1",
		"/p/b.fung":    "import \"./a\" (x)\nimport \"./a\" as am\ny = 1",
		"/p/main.fung": "import \"./b\"\nprint(b.y)",
	}
	l = memLoader(files, stdlib)
	prog, errs = l.Load("/p/main.fung", files["/p/main.fung"])
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if got := prog.Modules[1].Proto.Exports; len(got) != 1 || got[0] != "y" {
		t.Errorf("b exports %v, want [y]: imported names are not re-exported (04)", got)
	}
	for _, src := range []string{"import \"./b\"\nprint(b.x)", "import \"./b\"\nprint(b.am)"} {
		files["/p/main.fung"] = src
		if _, errs := l.Load("/p/main.fung", src); len(errs) != 1 || errs[0].Code != "E0303" {
			t.Errorf("%q: %v, want E0303", src, errs)
		}
	}
}

func TestLoadErrors(t *testing.T) {
	files := map[string]string{
		"/p/main.fung":       "import \"./cyc_a\"",
		"/p/cyc_a.fung":      "import \"./cyc_b\"",
		"/p/cyc_b.fung":      "import \"./cyc_a\"",
		"/p/self.fung":       "import \"./self\"",
		"/p/entry.fung":      "import \"./uses_entry\"",
		"/p/uses_entry.fung": "import \"./entry\"",
		"/p/bad-name.fung":   "x = 1",
		"/p/usebad.fung":     "import \"./bad-name\"",
		"/p/usebad2.fung":    "import \"./bad-name\" as ok\nprint(ok.x)",
		"/p/lib_err.fung":    "print(1)",
		"/p/uselib.fung":     "import \"./lib_err\"",
		"/p/synerr.fung":     "x = (",
		"/p/usesyn.fung":     "import \"./synerr\"",
	}
	l := memLoader(files, map[string]string{})
	cases := map[string]string{
		"/p/main.fung":    "E0302",
		"/p/self.fung":    "E0302",
		"/p/entry.fung":   "E0307",
		"/p/usebad.fung":  "E0305",
		"/p/usebad2.fung": "",
		"/p/uselib.fung":  "E0306",
		"/p/usesyn.fung":  "E0110",
	}
	for entry, want := range cases {
		_, errs := l.Load(entry, files[entry])
		got := ""
		if len(errs) > 0 {
			got = errs[0].Code
		}
		if got != want {
			t.Errorf("%s: %v, want %s", entry, errs, want)
		}
	}
	_, errs := l.Load("/p/main.fung", files["/p/main.fung"])
	if msg := errs[0].Error(); !strings.Contains(msg, "cyc_a.fung → /p/cyc_b.fung → /p/cyc_a.fung") {
		t.Errorf("cycle message: %s", msg)
	}
	_, errs = l.Load("-e", "import missing")
	if len(errs) != 1 || errs[0].Code != "E0301" {
		t.Errorf("missing stdlib: %v", errs)
	}
	_, errs = l.Load("/p/x.fung", "import \"./nothing\"")
	if len(errs) != 1 || errs[0].Code != "E0301" {
		t.Errorf("missing file: %v", errs)
	}
	prog, errs := l.Load("/p/usesyn.fung", files["/p/usesyn.fung"])
	if len(errs) == 0 || prog.Sources["/p/synerr.fung"] != "x = (" {
		t.Errorf("sources of failed loads are kept for error excerpts: %v", prog.Sources)
	}
}

func TestLoaderCollectsWarningsAndTests(t *testing.T) {
	files := map[string]string{
		"/p/lib.fung":  "import \"./util\"\nfn f() = 1",
		"/p/util.fung": "x = 1",
		"/p/main.fung": "import \"./lib\"\nfn g():\n  y = 1\n  2\n;;\ntest \"t\": assert lib.f() == 1 ;;\nprint(g())",
	}
	l := memLoader(files, nil)
	l.Tests = true
	prog, errs := l.Load("/p/main.fung", files["/p/main.fung"])
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	var codes []string
	for _, w := range prog.Warnings {
		codes = append(codes, w.Code+":"+w.Pos.File)
	}
	if len(codes) != 2 || codes[0] != "W0101:/p/lib.fung" || codes[1] != "W0102:/p/main.fung" {
		t.Errorf("warnings = %v", codes)
	}
	if !strings.Contains(bytecode.Disassemble(prog.Entry), "TEST") {
		t.Error("tests kept")
	}
	l.Tests = false
	prog, _ = l.Load("/p/main.fung", files["/p/main.fung"])
	if strings.Contains(bytecode.Disassemble(prog.Entry), "TEST") {
		t.Error("tests dropped")
	}
	files["/p/lib.fung"] = "import \"./util\"\nfn f() = 1\ntest \"lib\": assert f() == 1 ;;"
	l.Tests = true
	prog, errs = l.Load("/p/main.fung", files["/p/main.fung"])
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	for _, m := range prog.Modules {
		if m.Proto != nil && strings.Contains(bytecode.Disassemble(m.Proto), "TEST") {
			t.Errorf("%s: a library's tests run only when it is the entry (08)", m.Key)
		}
	}
}

func TestCycleMessageShowsPaths(t *testing.T) {
	files := map[string]string{
		"/p/main.fung":   "import \"./x/util\"",
		"/p/x/util.fung": "import \"../y/util\"",
		"/p/y/util.fung": "import \"../x/util\"",
	}
	l := memLoader(files, nil)
	_, errs := l.Load("/p/main.fung", files["/p/main.fung"])
	if len(errs) != 1 || errs[0].Code != "E0302" {
		t.Fatal(errs)
	}
	msg := errs[0].Error()
	if !strings.Contains(msg, "x/util.fung → ") || !strings.Contains(msg, "y/util.fung → ") || strings.Contains(msg, "util → util") {
		t.Errorf("cycle message: %s", msg)
	}
}

func TestAliasCollisionMessage(t *testing.T) {
	stdlib := map[string]string{"fs": "x = 1"}
	files := map[string]string{"/p/lib/x.fung": "a = 1", "/p/main.fung": "import \"./lib/x\" as fs\nimport fs"}
	l := memLoader(files, stdlib)
	_, errs := l.Load("/p/main.fung", files["/p/main.fung"])
	if len(errs) != 1 || errs[0].Code != "E0309" || !strings.Contains(errs[0].Error(), "already bound to a module") {
		t.Errorf("errs = %v", errs)
	}
}
