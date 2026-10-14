package resolve

import (
	"strings"
	"testing"
)

var fsInfo = ModuleInfo{Key: "fs", Name: "fs", Exports: []string{"read", "write"}}
var libInfo = ModuleInfo{Key: "/abs/lib/x.fung", Name: "x", Exports: []string{"a", "b"}}

func resolveM5(t *testing.T, src string, lib bool) (Result, []string) {
	t.Helper()
	opts := Options{Imports: map[string]ModuleInfo{"fs": fsInfo, "./lib/x": libInfo}, Library: lib}
	r, errs := Resolve(coreOf(t, src), []string{"print", "map"}, opts)
	codes := make([]string, len(errs))
	for i, e := range errs {
		codes[i] = e.Code
	}
	return r, codes
}

func TestImportBindsConstantGlobals(t *testing.T) {
	r, errs := resolveM5(t, "import fs\nimport \"./lib/x\" as y (a)\nprint(fs.read, y.b, a)", false)
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	got := r.Core.String()
	for _, want := range []string{
		`(import "fs" (global 0))`,
		`(import "/abs/lib/x.fung" (global 1) (member a (global 2)))`,
		"(field read (global 0))",
	} {
		if !strings.Contains(got, want) {
			t.Errorf("missing %s in %s", want, got)
		}
	}
	if len(r.Imports) != 2 || r.Imports[0] != "fs" || r.Imports[1] != "/abs/lib/x.fung" {
		t.Errorf("Imports = %v", r.Imports)
	}
	if len(r.Globals) != 3 || r.Globals[0] != "fs" || r.Globals[1] != "y" || r.Globals[2] != "a" {
		t.Errorf("Globals = %v", r.Globals)
	}
	r, _ = resolveM5(t, "import fs\nimport \"./lib/x\" (a)\n_p = 1\nfn f() = 1\nq = 2", false)
	if len(r.Exports) != 2 || r.Exports[0] != "f" || r.Exports[1] != "q" {
		t.Errorf("Exports = %v (imports and private names must be excluded)", r.Exports)
	}
}

func TestImportErrors(t *testing.T) {
	cases := map[string]string{
		"import nope":                         "E0301",
		"import fs\nprint(fs.nope)":           "E0303",
		"import \"./lib/x\" (nope)":           "E0303",
		"import fs\nprint(fs._secret)":        "E0304",
		"import fs\nimport fs":                "E0309",
		"import fs\nimport \"./lib/x\" as fs": "E0309",
		"import fs\nfs = 1":                   "E0308",
		"import fs\nfn fs() = 1":              "E0308",
		"import fs\nfn f(fs) = fs":            "E0308",
		"import fs\nfor fs in [1]: fs ;;":     "E0308",
		"import fs\nmatch 1: fs => fs ;;":     "E0308",
		"fn f():\n  import fs\n  1\n;;":       "E0311",
		"import \"./lib/x\" (a)\na = 2":       "E0209", // listed names are constants (plan decision 4)
		"import \"./lib/x\" (a)\nfn g() = a":  "",      // and hoisted fns see them
		"import \"./lib/x\" (a)\nprint(a, x)": "E0204", // the module name is not bound when only names are listed
		"fn map(xs) = xs\nprint(map(1))":      "",      // prelude names may be shadowed (04)
	}
	for src, want := range cases {
		_, errs := resolveM5(t, src, false)
		got := ""
		if len(errs) > 0 {
			got = errs[0]
		}
		if got != want {
			t.Errorf("%q: %v, want %s", src, errs, want)
		}
	}
}

func TestLibraryTopLevelRestriction(t *testing.T) {
	cases := map[string]string{
		"import fs\nfn f() = 1\ntype t = int\nx = print(1)\n(a, b) = (1, 2)": "",
		"print(1)":        "E0306",
		"fn f() = 1\nf()": "E0306",
		"1 + 1":           "E0306",
		"if true: 1 ;;":   "E0306",
	}
	for src, want := range cases {
		_, errs := resolveM5(t, src, true)
		got := ""
		if len(errs) > 0 {
			got = errs[0]
		}
		if got != want {
			t.Errorf("%q: %v, want %s", src, errs, want)
		}
	}
	if _, errs := resolveM5(t, "print(1)", false); len(errs) != 0 {
		t.Error("the entry file may have statements")
	}
}

func TestDuplicatePatternNames(t *testing.T) {
	cases := map[string]string{
		"[a, a] = [1, 2]":                     "E0221",
		"match 1: (a, a) => a ;;":             "E0221",
		"match 1: {x: a, y: a} => a ;;":       "E0221",
		"match 1: [a, ...a] => a ;;":          "E0221",
		"match 1: Ok(a) | Err(a) => a ;;":     "", // the same name once per alternative is fine
		"match 1: [a, b] when a == b => a ;;": "",
		"_ = print(1)":                        "",
	}
	for src, want := range cases {
		_, errs := resolveM5(t, src, false)
		got := ""
		if len(errs) > 0 {
			got = errs[0]
		}
		if got != want {
			t.Errorf("%q: %v, want %s", src, errs, want)
		}
	}
}
