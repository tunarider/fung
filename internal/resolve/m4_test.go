package resolve

import (
	"strings"
	"testing"
)

func TestDestructuringBindsLikeAssignment(t *testing.T) {
	r, errs := Resolve(coreOf(t, "(q, r) = f()\nprint(q, r)\n[a, ...rest] = xs\n(q, r) = g()"), []string{"print", "f", "g", "xs"}, Options{})
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	got := r.Core.String()
	if !strings.Contains(got, "(let (ptuple (pbind (global 0)) (pbind (global 1)))") || !strings.Contains(got, "(prest (global 3))") {
		t.Fatalf("got %s", got)
	}
	if r.Globals[0] != "q" || r.Globals[3] != "rest" {
		t.Fatalf("globals = %v", r.Globals)
	}
	_, errs = res(t, "total = 0\nif true: (total, n) = (1, 2) ;;")
	if len(errs) != 1 || errs[0].Code != "E0205" {
		t.Fatalf("destructuring an outer name: %v", errs)
	}
}

func TestMatchArmsShadowAndScope(t *testing.T) {
	r, errs := res(t, "x = 1\nmatch 2:\n  x when x > 1 => x\n  _ => x\n;;\nprint(x)")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	got := r.Core.String()
	// The arm's x is a fresh slot; outside the match, x is still the global.
	if !strings.Contains(got, "(arm (pbind (local 1)) (op > (local 1) 1) (do (local 1)))") || !strings.Contains(got, "(call (prelude print) (global 0))") {
		t.Fatalf("got %s", got)
	}
	if r.NumLocals != 2 { // $m0 and the arm's x
		t.Fatalf("NumLocals = %d", r.NumLocals)
	}
	_, errs = res(t, "match 1: 'a | 'b => 1 ;;\nmatch 1: Ok(x) | Err(y) => 1 ;;")
	if len(errs) != 1 || errs[0].Code != "E0216" {
		t.Fatalf("alt names: %v", errs)
	}
	_, errs = res(t, "match 1: x => 1 ;;\nprint(x)")
	if len(errs) != 1 || errs[0].Code != "E0206" {
		t.Fatalf("arm name leaks: %v", errs)
	}
}

func TestNestedMatchUsesSeparateHiddenSlots(t *testing.T) {
	r, errs := res(t, "match 1:\n  a => match 2:\n    b => a + b\n  ;;\n;;")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if got := r.Core.String(); !strings.Contains(got, "(let (local 0) 1)") || !strings.Contains(got, "(let (local 2) 2)") {
		t.Fatalf("got %s", got)
	}
}

func TestForBodyReturnAndQuestionOutsideFn(t *testing.T) {
	cases := map[string]string{
		"for x in xs: return x ;;":                "E0213",
		"for x in xs: if x: return 1 ;; ;;":       "E0213",
		"for x in xs: (() => return x)() ;;":      "", // a nested fn may return
		"fn f(xs) = for x in xs: return x ;;":     "E0213",
		"fn f(xs) = for x in xs: x? ;;":           "E0220",
		"fn f(xs) = for x in xs: g(x?) ;;":        "E0220",
		"fn f(xs) = for x in xs: (() => x?)() ;;": "", // a nested fn may return
		"a = 1\nfor (a, b) in xs: a ;;":           "", // for variables shadow (03)
		"a = 1\nfor a in xs: a ;;":                "",
		"f()?":                                    "E0214",
		"fn g() = f()?":                           "",
		"print(_)":                                "", // hole: fine
		"_ + 1":                                   "E0215",
		"x = _":                                   "E0215",
	}
	for src, want := range cases {
		_, errs := Resolve(coreOf(t, src), []string{"print", "f", "g", "xs"}, Options{})
		got := ""
		if len(errs) > 0 {
			got = errs[0].Code
		}
		if got != want {
			t.Errorf("%q: got %s (%v), want %s", src, got, errs, want)
		}
	}
}

func TestTypeAliases(t *testing.T) {
	cases := map[string]string{
		"type shape = Circle | Rect\nfn area(s: shape) = s": "",
		"fn area(s: shape) = s\ntype shape = Circle":        "", // hoisted
		"type a = b\ntype b = int\nfn f(x: a) = x":          "",
		"type a = a\nfn f(x: a) = x":                        "E0218",
		"type a = b\ntype b = a\nfn f(x: a) = x":            "E0218",
		"type a = int\ntype a = str":                        "E0217",
		"fn f(): type a = int ;;":                           "E0219",
		"type a = zzz":                                      "E0212",
	}
	for src, want := range cases {
		_, errs := res(t, src)
		got := ""
		if len(errs) > 0 {
			got = errs[0].Code
		}
		if got != want {
			t.Errorf("%q: got %s (%v), want %s", src, got, errs, want)
		}
	}
	r, _ := res(t, "type shape = Circle | Rect\nfn area(s: shape | nil) = s")
	info := fnInfo(t, r.Core, 0, 1)
	if strings.Join(info.Params[0].Type, "|") != "Circle|Rect|nil" {
		t.Fatalf("expanded type = %v", info.Params[0].Type)
	}
}

func TestRestParamIsAllowedNow(t *testing.T) {
	r, errs := res(t, "fn f(a, ...xs) = xs")
	if len(errs) != 0 || !fnInfo(t, r.Core, 0, 1).Params[1].Rest {
		t.Fatalf("errs = %v", errs)
	}
}
