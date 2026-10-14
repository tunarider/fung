package vm

import (
	"bytes"
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/frontend"
	"github.com/tunarider/fung/internal/prelude"
	"github.com/tunarider/fung/internal/resolve"
	"github.com/tunarider/fung/internal/value"
)

func buildTests(t *testing.T, src string) (*Runtime, frontend.Build, *bytes.Buffer) {
	t.Helper()
	l := &frontend.Loader{Prelude: prelude.Test(), Natives: testRegistry().Natives, Stdlib: testRegistry().Stdlib, Tests: true}
	b, errs := l.LoadBuild("t.fung", src)
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out, Stderr: &out}
	registerModules(rt, b)
	return rt, b, &out
}

func TestAssertReports(t *testing.T) {
	cases := map[string][]string{
		"fn f(x) = x + 1\nassert f(1) == 3":     {"assertion failed: f(1) == 3", "f(1) = 2"},
		"a = 1\nb = 2\nassert a > b":            {"assertion failed: a > b", "a = 1", "b = 2"},
		"assert false":                          {"assertion failed: false"},
		"xs = []\nassert is_empty(xs) == false": {"is_empty(xs) = true"},
	}
	for src, wants := range cases {
		rt := &Runtime{Stdout: &bytes.Buffer{}}
		b := buildWith(t, src, bootTestPrelude(t, rt))
		_, err := rt.Run(b.Proto)
		if err == nil || err.Code != "E0807" {
			t.Errorf("%q: err = %v", src, err)
			continue
		}
		text := err.Error() + "\n" + strings.Join(err.Notes, "\n")
		for _, w := range wants {
			if !strings.Contains(text, w) {
				t.Errorf("%q: %q lacks %q", src, text, w)
			}
		}
	}
	if _, _, code := run(t, "assert 1"); code != "E0505" {
		t.Errorf("non-bool assert: %s", code)
	}
	if _, _, code := run(t, "assert 1 < \"a\""); code != "E0501" {
		t.Errorf("incomparable sides: %s", code)
	}
	if _, out, code := run(t, "fn f(): print(\"f\"); 1 ;;\nfn g(): print(\"g\"); 1 ;;\nassert f() == g()\nprint(\"ok\")"); code != "" || out != "f\ng\nok\n" {
		t.Errorf("sides evaluated once: out=%q code=%s", out, code)
	}
	if _, _, code := run(t, "assert 1 != 1.0"); code != "E0807" {
		t.Errorf("!= uses Equal: %s", code)
	}
}

func TestTestBlocksRegisterOnlyWhenRequested(t *testing.T) {
	src := "x = 1\ntest \"one\": assert x == 1 ;;\ntest \"two\": assert x == 2 ;;\nprint(\"body\")"
	if _, out, code := run(t, src); code != "" || out != "body\n" {
		t.Fatalf("run ignores tests: out=%q code=%s", out, code)
	}
	rt, b, out := buildTests(t, src)
	if _, err := rt.Run(b.Proto); err != nil {
		t.Fatal(err)
	}
	tests := rt.Tests()
	if len(tests) != 2 || tests[0].Name != "one" || tests[1].Name != "two" || out.String() != "body\n" {
		t.Fatalf("tests = %+v out=%q", tests, out.String())
	}
	if err := rt.CallTest(tests[0].Fn); err != nil {
		t.Errorf("test one: %v", err)
	}
	if err := rt.CallTest(tests[1].Fn); err == nil || err.Code != "E0807" {
		t.Errorf("test two: %v", err)
	}
	rt, b, _ = buildTests(t, "x = 1\ntest \"t\": assert x == 1 ;;\nx = 2")
	rt.Run(b.Proto)
	if err := rt.CallTest(rt.Tests()[0].Fn); err != nil {
		t.Errorf("a test sees the variables that existed when it was defined (model C): %v", err)
	}
	rt, b, _ = buildTests(t, "test \"e\": exit(3) ;;")
	rt.Run(b.Proto)
	if err := rt.CallTest(rt.Tests()[0].Fn); err == nil {
		t.Error("exit signal propagates")
	} else if code, ok := err.ExitCode(); !ok || code != 3 {
		t.Errorf("exit signal: %v", err)
	}
}

func TestDbgPrintsToStderrAndReturnsValue(t *testing.T) {
	p := compileWith(t, "xs = [1, 2]\nprint(xs |> map(x => x * 2) |> dbg |> len)\nd = dbg\nprint(d(3))", prelude.Test())
	var out, errOut bytes.Buffer
	if _, err := (&Runtime{Stdout: &out, Stderr: &errOut}).Run(p); err != nil {
		t.Fatal(err)
	}
	if out.String() != "2\n3\n" {
		t.Errorf("stdout = %q", out.String())
	}
	if errOut.String() != "[t.fung:2] xs |> map(x => x * 2) = [2, 4]\ndbg = 3\n" {
		t.Errorf("stderr = %q", errOut.String())
	}
}

func TestDbgInsideInterpolationShowsTheRightText(t *testing.T) {
	p := compileWith(t, "x = 5\nprint(\"v=${dbg(x + 1)} tail\")", prelude.Test())
	var out, errOut bytes.Buffer
	if _, err := (&Runtime{Stdout: &out, Stderr: &errOut}).Run(p); err != nil {
		t.Fatal(err)
	}
	if out.String() != "v=6 tail\n" || errOut.String() != "[t.fung:2] x + 1 = 6\n" {
		t.Fatalf("out=%q err=%q", out.String(), errOut.String())
	}
}

func TestReplGlobalsSurviveGrowth(t *testing.T) {
	rt := &Runtime{Stdout: &bytes.Buffer{}}
	rt.PrepareREPL()
	first := compileWith(t, "fn g() = 1\nfn f() = g()", prelude.Test())
	if _, err := rt.Run(first); err != nil {
		t.Fatal(err)
	}
	l := &frontend.Loader{Prelude: prelude.Test(), Natives: testRegistry().Natives, Stdlib: testRegistry().Stdlib, Resolve: resolve.Options{Globals: []string{"g", "f"}, Redefine: true}}
	b, errs := l.LoadBuild("t.fung", "fn g() = 2\nh = 3\nf()")
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	v, err := rt.Run(b.Proto)
	if err != nil || v != value.Int(2) {
		t.Fatalf("f() after redefining g = %v %v", v, err)
	}
}

func TestParameterPatternsRun(t *testing.T) {
	cases := map[string]string{
		"print(map([(1, 2), (3, 4)], ((a, b)) => a + b))":                              "[3, 7]\n",
		"fn f((a, b), c) = a + b + c\nprint(f((1, 2), 3))":                             "6\n",
		"fn g({name: n}) = n\nprint(g({name: \"x\", y: 1}))":                           "x\n",
		"print(map([Ok(1), Ok(2)], (Ok(x)) => x * 10))":                                "[10, 20]\n",
		"print(map([1, 2], _ => 0), sort_by([(1, \"b\"), (2, \"a\")], ((_, s)) => s))": "[0, 0] [(2, \"a\"), (1, \"b\")]\n",
		"each(pairs({a: 1}), ((k, v)) => print(k, v))":                                 "a 1\n",
		"fn h(x, ...rest) = rest\nfn p([a], y = 2) = a + y\nprint(h(1, 2), p([1]))":    "[2] 3\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	if _, _, code := run(t, "fn f((a, b)) = a\nf((1, 2, 3))"); code != "E0602" {
		t.Errorf("mismatch: %s", code)
	}
	if _, _, code := run(t, "f = ((a, b)) => a\nf(1)"); code != "E0602" {
		t.Errorf("not a tuple: %s", code)
	}
	if _, _, code := run(t, "fn f((a, b)) = a\nf(x: 1)"); code != "E0406" {
		t.Errorf("a pattern parameter has no name to pass by: %s", code)
	}
}
