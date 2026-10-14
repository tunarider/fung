package resolve

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/tree"
)

func fnInfo(t *testing.T, core *tree.Node, path ...int) *tree.FnInfo {
	t.Helper()
	n := core
	for _, i := range path {
		n = n.Kids[i]
	}
	info, ok := n.Lit.(*tree.FnInfo)
	if !ok {
		t.Fatalf("node %s is not a fn", n)
	}
	return info
}

func TestTopLevelFnsAreHoistedGlobals(t *testing.T) {
	r, errs := res(t, "print(f(1))\nfn f(n) = g(n)\nfn g(n) = n + 1")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (let (global 0) (fn f(n) (do (call (global 1) (local 0))))) (let (global 1) (fn g(n) (do (op + (local 0) 1)))) (call (prelude print) (call (global 0) 1)))"
	if got := r.Core.String(); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if r.Globals[0] != "f" || r.Globals[1] != "g" || r.NumLocals != 0 {
		t.Fatalf("globals=%v locals=%d", r.Globals, r.NumLocals)
	}
	if info := fnInfo(t, r.Core, 0, 1); info.NumLocals != 1 || len(info.Captures) != 0 {
		t.Fatalf("f info = %+v", info)
	}
}

func TestParamsLocalsAndDefaults(t *testing.T) {
	r, errs := res(t, "fn f(a, b = a + 1):\n  c = a * b\n  if c > 0: d = c ;;\n  c\n;;")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (let (global 0) (fn f(a, b=?) (op + (local 0) 1) (do (let (local 2) (op * (local 0) (local 1))) (if (op > (local 2) 0) (do (let (local 3) (local 2))) (nil)) (local 2)))))"
	if got := r.Core.String(); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	if info := fnInfo(t, r.Core, 0, 1); info.NumLocals != 4 {
		t.Fatalf("NumLocals = %d", info.NumLocals)
	}
}

func TestClosuresCaptureByChain(t *testing.T) {
	// y is a local of the outer fn; the middle lambda captures it from a
	// slot, the inner lambda captures it from the middle closure.
	r, errs := res(t, "fn outer(y) = x => z => x + y + z")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	want := "(do (let (global 0) (fn outer(y) (do (fn (x) (do (fn (z) (do (op + (op + (captured 0) (captured 1)) (local 0))))))))))"
	if got := r.Core.String(); got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	middle := fnInfo(t, r.Core, 0, 1, 0, 0)
	if len(middle.Captures) != 1 || middle.Captures[0] != (tree.Capture{Kind: tree.CaptureLocal, Index: 0}) {
		t.Fatalf("middle captures = %+v", middle.Captures)
	}
	inner := fnInfo(t, r.Core, 0, 1, 0, 0, 0, 0)
	if len(inner.Captures) != 2 || inner.Captures[0] != (tree.Capture{Kind: tree.CaptureLocal, Index: 0}) || inner.Captures[1] != (tree.Capture{Kind: tree.CaptureCaptured, Index: 0}) {
		t.Fatalf("inner captures = %+v", inner.Captures)
	}
}

func TestTopLevelVariablesAreCopiedByLambdasAndInvisibleToHoistedFns(t *testing.T) {
	// A lambda created after x exists copies x's value (CaptureGlobal);
	// other top-level fns are constants and stay direct (global n) refs.
	r, errs := res(t, "fn h() = 1\nx = 1\ng = () => x + h()")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if got := r.Core.String(); !strings.Contains(got, "(fn () (do (op + (captured 0) (call (global 0)))))") {
		t.Fatalf("got %s", got)
	}
	g := fnInfo(t, r.Core, 2, 1)
	if len(g.Captures) != 1 || g.Captures[0] != (tree.Capture{Kind: tree.CaptureGlobal, Index: 1}) {
		t.Fatalf("captures = %+v", g.Captures)
	}
	// A top-level fn is created before any variable exists, so it cannot
	// see x, whether x is assigned before or after it (03, model C).
	for _, src := range []string{"x = 1\nfn f() = x", "fn f() = x\nx = 1"} {
		_, errs := res(t, src)
		if len(errs) != 1 || errs[0].Code != "E0204" || !strings.Contains(errs[0].Help, "pass it as an argument") {
			t.Fatalf("%q: errs = %+v", src, errs)
		}
	}
}

func TestLocalNamedFnUsesSelfAndRejectsLaterSiblings(t *testing.T) {
	r, errs := res(t, "fn outer():\n  fn loop(n) = n == 0 then 0 else loop(n - 1)\n  loop(3)\n;;")
	if len(errs) != 0 {
		t.Fatal(errs)
	}
	if got := r.Core.String(); !strings.Contains(got, "(call (self) (op - (local 0) 1))") || !strings.Contains(got, "(call (local 0) 3)") {
		t.Fatalf("got %s", got)
	}
	// A lambda inside the named fn reaches self through a capture.
	r, _ = res(t, "fn outer():\n  fn f(n) = (() => f(n))()\n  f(1)\n;;")
	if got := r.Core.String(); !strings.Contains(got, "(fn () (do (call (captured 0) (captured 1))))") {
		t.Fatalf("got %s", got)
	}
	_, errs = res(t, "fn outer():\n  fn a() = b()\n  fn b() = 1\n  a()\n;;")
	if len(errs) != 1 || errs[0].Code != "E0208" || errs[0].Args[0] != "b" {
		t.Fatalf("errs = %v", errs)
	}
}

func TestFnScopeErrors(t *testing.T) {
	cases := map[string]string{
		"fn f() = 1\nfn f() = 2":                "E0210",
		"fn f() = 1\nf = 2":                     "E0209",
		"x = 1\nfn f(): x = 2 ;;":               "",      // f cannot see x: this is a new local
		"x = 1\nf = fn(): x = 2 ;;":             "E0205", // an anonymous fn created after x sees it
		"fn f(a): g = () => a; a = 2 ;;":        "",      // same scope rebinding is fine
		"return 1":                              "E0211",
		"if true: return 1 ;;":                  "E0211",
		"fn f(a: integer) = a":                  "E0212",
		"fn f(a) -> strng = a":                  "E0212",
		"fn f(a: Circle | int) = a":             "",
		"fn f() = y\ny = 1":                     "E0204", // created before y exists
		"f = () => f()":                         "E0204", // f is not bound yet when the lambda is created
		"fn outer(): f = () => f(); f() ;;":     "E0204", // a block local: only named fns see themselves
		"fn f():\n  if true: v = 1 ;;\n  v\n;;": "E0206",
		"fn f(): fn g() = 1 ;;\nfn h() = g()":   "E0204", // local fn of f is not visible in h
	}
	for src, want := range cases {
		_, errs := res(t, src)
		got := ""
		if len(errs) > 0 {
			got = errs[0].Code
		}
		if want == "" && len(errs) != 0 {
			t.Errorf("%q: unexpected %v", src, errs)
		} else if want != "" && got != want {
			t.Errorf("%q: got %s (%v), want %s", src, got, errs, want)
		}
	}
}

func TestPerFunctionClosedSets(t *testing.T) {
	// A name closed in f's block must not turn an unrelated typo in g into E0206.
	_, errs := res(t, "fn f(): if true: tmp = 1 ;; ;;\nfn g() = tmp")
	if len(errs) != 1 || errs[0].Code != "E0204" {
		t.Fatalf("errs = %v", errs)
	}
}
