package resolve

import "testing"

func warningsOf(t *testing.T, src string, opts Options) []string {
	t.Helper()
	r, errs := Resolve(coreOf(t, src), []string{"print", "map", "len"}, opts)
	if len(errs) > 0 {
		t.Fatalf("%q: %v", src, errs)
	}
	var codes []string
	for _, w := range r.Warnings {
		codes = append(codes, w.Code+":"+w.Args[0].(string))
	}
	return codes
}

func TestWarnings(t *testing.T) {
	opts := Options{Imports: map[string]ModuleInfo{"fs": {Key: "fs", Name: "fs", Exports: []string{"read"}}}}
	cases := map[string][]string{
		"import fs":                                          {"W0101:fs"},
		"import fs as f\nprint(f.read)":                      nil,
		"import fs (read)":                                   {"W0101:read"},
		"import fs (read)\nprint(read)":                      nil,
		"fn f():\n  x = 1\n  2\n;;":                          {"W0102:x"},
		"fn f():\n  _x = 1\n  2\n;;":                         nil,
		"fn f(a) = 1":                                        nil, // parameters never warn
		"x = 1":                                              nil, // file globals never warn (they are the module's API)
		"fn f():\n  (a, b) = (1, 2)\n  a\n;;":                {"W0102:b"},
		"fn f(v) = match v: (a, _) => 1 ;;":                  {"W0102:a"},
		"fn f(v) = match v: x when x > 0 => 1; _ => 0 ;;":    nil,
		"fn map(xs) = xs":                                    {"W0103:map"},
		"len = 1":                                            {"W0103:len"},
		"fn f(len) = len":                                    {"W0103:len"},
		"fn f(v) = match v: map => map ;;":                   {"W0103:map"},
		"fn f():\n  x = 1\n  fn g() = x\n  g()\n;;":          nil,          // read by a nested fn counts
		"fn f():\n  x = 1\n  x = 2\n  x\n;;":                 nil,          // reassigned then read
		"fn f():\n  x = if true: _y = 1 ;;\n  x\n;;":         {"W0104:_y"}, // a block used as a value ends with an assignment
		"fn f():\n  x = if true: 1 ;;\n  x\n;;":              nil,
		"print(if true: _y = 1 ;;)":                          {"W0104:_y"},
		"fn f():\n  x = if true: 1 else: _z = 2 ;;\n  x\n;;": {"W0104:_z"}, // any branch
		"fn f() = if true: _z = 2 ;;":                        nil,          // fn bodies are not checked
		"fn f():\n  return match 1: 1 => _y = 1 ;;\n;;":      {"W0104:_y"},
	}
	libOpts := Options{Library: true}
	if got := warningsOf(t, "fn map(xs) = xs\nlen = 1", libOpts); len(got) != 2 || got[0] != "W0103:map" || got[1] != "W0103:len" {
		t.Errorf("the rule does not depend on how a file is loaded (08): %v", got)
	}
	for src, want := range cases {
		got := warningsOf(t, src, opts)
		if len(got) != len(want) {
			t.Errorf("%q: %v, want %v", src, got, want)
			continue
		}
		for i := range want {
			if got[i] != want[i] {
				t.Errorf("%q: %v, want %v", src, got, want)
			}
		}
	}
}

func TestReplOptions(t *testing.T) {
	r, errs := Resolve(coreOf(t, "fn g() = 2\nprint(x, g())"), []string{"print"}, Options{Globals: []string{"x", "g"}, Redefine: true})
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if len(r.Globals) != 2 || r.Globals[0] != "x" || r.Globals[1] != "g" {
		t.Errorf("Globals = %v (existing names keep their indexes)", r.Globals)
	}
	if got := r.Core.String(); got != "(do (let (global 1) (fn g() (do 2))) (call (prelude print) (global 0) (call (global 1))))" {
		t.Errorf("core = %s", got)
	}
	_, errs = Resolve(coreOf(t, "fn g() = 2"), []string{"print"}, Options{Globals: []string{"g"}})
	if len(errs) != 1 || errs[0].Code != "E0210" {
		t.Errorf("without Redefine a known fn is a duplicate: %v", errs)
	}
	r, errs = Resolve(coreOf(t, "y = 1"), []string{"print"}, Options{Globals: []string{"x"}})
	if len(errs) != 0 || len(r.Globals) != 2 || r.Globals[1] != "y" {
		t.Errorf("new globals append: %v %v", r.Globals, errs)
	}
	r, errs = Resolve(coreOf(t, "x = 5"), []string{"print"}, Options{Globals: []string{"x"}, Redefine: true})
	if len(errs) != 0 || len(r.Globals) != 1 {
		t.Errorf("reassigning a known global: %v %v", r.Globals, errs)
	}
}
