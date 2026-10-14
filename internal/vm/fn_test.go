package vm

import (
	"bytes"
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/prelude"
	"github.com/tunarider/fung/internal/value"
)

func TestFunctionsRun(t *testing.T) {
	cases := map[string]string{
		"fn add(a, b) = a + b\nprint(add(1, 2))":                                                                                  "3\n",
		"fn fib(n) = n < 2 then n else fib(n - 1) + fib(n - 2)\nprint(fib(20))":                                                   "6765\n",
		"print(even(10), odd(10))\nfn even(n) = n == 0 then true else odd(n - 1)\nfn odd(n) = n == 0 then false else even(n - 1)": "true false\n",
		"f = x => x * 2\nprint(f(21))":                                                                                            "42\n",
		"print((fn(a):\n  a + 1\n;;)(1))":                                                                                         "2\n",
		"fn f():\n  x = 1\n  y = 2\n  x + y\n;;\nprint(f())":                                                                      "3\n",
		"fn f() = return 1\nprint(f())":                                                                                           "1\n",
		"fn f(n):\n  if n > 0: return \"pos\" ;;\n  \"other\"\n;;\nprint(f(1), f(0))":                                             "pos other\n",
		"fn f() = return\nprint(f())":                                                                                             "nil\n",
		"fn f(): x = 1 ;;\nprint(f())":                                                                                            "nil\n",
		"fn add(a, b) = a + b\nprint(add, x => x)":                                                                                "<fn add> <fn>\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestClosuresCaptureByValue(t *testing.T) {
	cases := map[string]string{
		"x = 1\nf = () => x\nx = 2\nprint(f())":                                                                        "1\n", // copied when f was made, like a local
		"fn mk():\n  x = 1\n  f = () => x\n  x = 2\n  f()\n;;\nprint(mk())":                                            "1\n", // a local: copied when f was made
		"fn adder(n) = x => x + n\nadd5 = adder(5)\nprint(add5(1), adder(10)(1))":                                      "6 11\n",
		"fn outer(a) = b => c => a + b + c\nprint(outer(1)(2)(3))":                                                     "6\n",
		"fn outer():\n  fn loop(n, acc) = n == 0 then acc else loop(n - 1, acc + n)\n  loop(4, 0)\n;;\nprint(outer())": "10\n",
		"fn outer(y):\n  fn f(n) = n == 0 then y else (() => f(n - 1))()\n  f(3)\n;;\nprint(outer(7))":                 "7\n", // self through a lambda
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestArgumentsDefaultsAndNames(t *testing.T) {
	cases := map[string]string{
		"fn f(a, b = a * 2) = a + b\nprint(f(1), f(1, 5))":                                             "3 6\n",
		"fn f(a, b = 1, c = 2) = a + b * 10 + c * 100\nprint(f(1, c: 3), f(c: 3, a: 2))":               "311 312\n",
		"fn f(greeting = \"hi\", name = \"you\") = greeting + \", \" + name\nprint(f(name: \"fung\"))": "hi, fung\n",
		"f = (a, b) => a - b\nprint(f(b: 1, a: 3))":                                                    "2\n",
		"fn f(a = print(\"eval\")) = a\nf(1)\nf()":                                                     "eval\n", // default evaluated only when needed
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestTailCallsDoNotGrowTheStack(t *testing.T) {
	_, out, code := run(t, "fn count(n, acc) = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(1000000, 0))")
	if code != "" || out != "1000000\n" {
		t.Fatalf("out=%q code=%s", out, code)
	}
	_, out, code = run(t, "fn even(n) = n == 0 then true else odd(n - 1)\nfn odd(n) = n == 0 then false else even(n - 1)\nprint(even(300001))")
	if code != "" || out != "false\n" {
		t.Fatalf("mutual: out=%q code=%s", out, code)
	}
}

func TestNonTailRecursionHitsTheFrameLimit(t *testing.T) {
	_, _, code := run(t, "fn f(n) = n == 0 then 0 else 1 + f(n - 1)\nprint(f(50000))")
	if code != "" {
		t.Fatalf("50000 deep: %s", code)
	}
	_, _, code = run(t, "fn f(n) = n == 0 then 0 else 1 + f(n - 1)\nprint(f(200000))")
	if code != "E0804" {
		t.Fatalf("200000 deep: %s, want E0804", code)
	}
	// A return type annotation disables TCO (03), so this one overflows too.
	_, _, code = run(t, "fn count(n, acc) -> int = n == 0 then acc else count(n - 1, acc + 1)\nprint(count(200000, 0))")
	if code != "E0804" {
		t.Fatalf("annotated: %s, want E0804", code)
	}
}

func TestTypeAnnotationsAreChecked(t *testing.T) {
	cases := map[string]string{
		"fn f(n: int, s: str = \"x\") -> str = s\nprint(f(1), f(2, \"y\"))": "x y\n",
		"fn f(n: num) = n\nprint(f(1), f(1.5))":                             "1 1.5\n",
		"fn f(v: str | nil) = v\nprint(f(nil), f(\"a\"))":                   "nil a\n",
		"fn f(v: any) = v\nprint(f('s))":                                    "'s\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		"fn f(n: int) = n\nf(\"a\")":         "E0403",
		"fn f(n: int, s: str = 1) = n\nf(1)": "E0403", // default is checked too
		"fn f() -> str = 1\nf()":             "E0404",
		"fn f(a, b) = a\nf(1, 2, 3)":         "E0402",
		"fn f(a) = a\nf(1, a: 2)":            "E0405",
		"fn f(a) = a\nf(b: 1)":               "E0406",
		"fn f(a, b) = a\nf(1)":               "E0407",
		"print(x: 1)":                        "E0406",
		"f = 1\nf()":                         "E0401",
	}
	for src, want := range errs {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}

func TestRuntimeErrorsCarryTheCallPath(t *testing.T) {
	// g's call is not a tail call (+ 0), so g keeps its own frame.
	p := compileWith(t, "fn f(x) = x / 0\nfn g() = f(1) + 0\ng()", prelude.Test())
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Code != "E0502" {
		t.Fatalf("err = %v", err)
	}
	names := []string{}
	for _, c := range err.CallPath {
		names = append(names, c.Fn)
	}
	if got := strings.Join(names, " "); got != "f g <main>" {
		t.Fatalf("call path = %v", err.CallPath)
	}
	if err.CallPath[0].Line != 1 || err.CallPath[1].Line != 2 || err.CallPath[2].Line != 3 {
		t.Fatalf("lines = %+v", err.CallPath)
	}
	p = compileWith(t, "fn loop(n) = n == 0 then 1 / 0 else loop(n - 1)\nloop(5)", prelude.Test())
	_, err = (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.CallPath[0].Elided != 5 {
		t.Fatalf("elided = %+v", err.CallPath)
	}
}

func TestAtExitRunsHandlersInReverseAfterTheProgram(t *testing.T) {
	p := compileWith(t, "at_exit(() => print(\"first registered\"))\nat_exit(() => print(\"second registered\"))\nprint(\"body\")", prelude.Test())
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	if errs := rt.RunExitHandlers(); len(errs) != 0 {
		t.Fatal(errs)
	}
	if out.String() != "body\nsecond registered\nfirst registered\n" {
		t.Fatalf("out = %q", out.String())
	}
}

func TestAtExitHandlerErrorsAreCollectedAndOthersStillRun(t *testing.T) {
	p := compileWith(t, "at_exit(() => print(\"ok\"))\nat_exit(() => fail(\"boom\"))", prelude.Test())
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	rt.Run(p)
	errs := rt.RunExitHandlers()
	if len(errs) != 1 || errs[0].Code != "E0803" || out.String() != "ok\n" {
		t.Fatalf("errs = %v out = %q", errs, out.String())
	}
	if _, _, code := run(t, "at_exit(1)"); code != "E0403" {
		t.Fatalf("at_exit(1) = %s", code)
	}
}

func TestGoPanicInExitHandlerBecomesInternalError(t *testing.T) {
	prelude := prelude.Test()
	prelude["boom"] = &value.Native{Name: "boom", Fn: func(value.Context, []value.Value) (value.Value, error) { panic("x") }}
	p := compileWith(t, "at_exit(boom)", prelude)
	rt := &Runtime{Stdout: &bytes.Buffer{}}
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	errs := rt.RunExitHandlers()
	if len(errs) != 1 || errs[0].Code != "E0801" {
		t.Fatalf("errs = %v, want E0801", errs)
	}
}

func TestAtExitRegisteredDuringExitStillRuns(t *testing.T) {
	p := compileWith(t, "at_exit(() => at_exit(() => print(\"nested\")))\nat_exit(() => print(\"outer\"))", prelude.Test())
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	rt.Run(p)
	if errs := rt.RunExitHandlers(); len(errs) != 0 || out.String() != "outer\nnested\n" {
		t.Fatalf("errs = %v out = %q", errs, out.String())
	}
	// A handler that keeps re-registering itself is stopped with a limit error.
	p = compileWith(t, "fn again() = at_exit(again)\nat_exit(again)", prelude.Test())
	rt = &Runtime{Stdout: &bytes.Buffer{}}
	rt.Run(p)
	if errs := rt.RunExitHandlers(); len(errs) != 1 || errs[0].Code != "E0806" {
		t.Fatalf("errs = %v, want E0806", errs)
	}
}
