package vm

import (
	"bytes"
	"testing"

	"github.com/tunarider/fung/internal/prelude"
)

func TestCollectionsRun(t *testing.T) {
	cases := map[string]string{
		"print([1, \"a\", [2]], (1,), (), {name: \"x\", \"a b\": 1}, Circle(3), 0..3, 0..=3)":                                         "[1, \"a\", [2]] (1,) () {name: \"x\", \"a b\": 1} Circle(3) 0..3 0..=3\n",
		"xs = [10, 20, 30]\nprint(xs[0], xs[-1], xs[1..3], xs[..1], xs[-2..], xs[0..0], xs[3..])":                                     "10 30 [20, 30] [10] [20, 30] [] []\n",
		"print(\"참치회\"[1], \"참치회\"[1..], len(\"참치\"), len([1, 2]), len(0..10))":                                                       "치 치회 2 2 10\n",
		"m = {name: \"x\", port: 80}\nprint(m.name, m[\"port\"], len(m))":                                                             "x 80 2\n",
		"print(1 in [1, 2], 3 in [1, 2], \"ell\" in \"hello\", \"k\" in {k: 1}, 5 in 0..10, 10 in 0..10, 10 in 0..=10, 2 not in [1])": "true false true true true false true true\n",
		"print([1] + [2], [1, 2] == [1, 2], [1] == (1,), {a: 1, b: 2} == {b: 2, a: 1}, [1, 2] < [1, 3], Ok(1) == Ok(1))":              "[1, 2] true false true true true\n",
		"print(type_of(Circle(3)), type_of(Red), type_of([]), type_of(0..1))":                                                         "'Circle 'sym 'list 'range\n",
		"fn area(s: Circle | Rect) = 1\nprint(area(Circle(3)))":                                                                       "1\n",
		"xs = [1, 2, 3]\nprint(xs |> len, xs |> map(x => x * 2), xs |> map(x => x + 1) |> len)":                                       "3 [2, 4, 6] 3\n",
		"print(for x in [1, 2]: x * 10 ;;, for i in 0..3: i ;;)":                                                                      "[10, 20] [0, 1, 2]\n",
		"print(for (k, v) in [(1, 2), (3, 4)]: k + v ;;)":                                                                             "[3, 7]\n",
		"fn sum(...xs) = len(xs)\nprint(sum(), sum(1, 2, 3))":                                                                         "0 3\n",
		"fn f(a, ...rest) = rest\nprint(f(1), f(1, 2, 3))":                                                                            "[] [2, 3]\n",
		"add = (a, b) => a + b\ninc = add(1, _)\nprint(inc(5), inc(10))":                                                              "6 11\n",
		"fn join3(a, b, c) = a + b + c\nmid = join3(\"<\", _, \">\")\nprint(mid(\"x\"))":                                              "<x>\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestReviewFixes(t *testing.T) {
	cases := map[string]string{
		"xs = [1, 2, 3]\nprint(xs[1..=2], xs[0..=-1], xs[..=0], \"참치회\"[1..=1])": "[2, 3] [1, 2, 3] [1] 치\n",
		"xs = [1, 2, 3]\ni = nil\nprint(xs[i ?? 0], xs[true then 1 else 2])":     "1 2\n",
		"a = 1\nprint(for (a, b) in [(2, 3)]: a + b ;;, a)":                      "[5] 1\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		"[1, 2, 3][1..=3]": "E0507",
		"Err()!":           "E0603",
		"Err(1, 2)!":       "E0603",
	}
	for src, want := range errs {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
	p := compileWith(t, "Err(1, 2)!", prelude.Test())
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Error() != "error[E0603]: unhandled Err: Err(1, 2)" {
		t.Fatalf("err = %v", err)
	}
}

func TestPartialEvaluatesFixedArgsOnce(t *testing.T) {
	_, out, code := run(t, "fn f(a, b) = a + b\nfn side(): print(\"once\"); 0 ;;\ng = f(side(), _)\ng(1)\ng(2)")
	if code != "" || out != "once\n" {
		t.Fatalf("out = %q code=%s", out, code)
	}
}

func TestResultOperators(t *testing.T) {
	cases := map[string]string{
		"print(Ok(1)!, Ok(2) ?? 0, nil ?? 3, Err(\"e\") ?? 4, 5 ?? 6)":  "1 2 3 4 5\n",
		"fn f(r) = r? + 1\nprint(f(Ok(1)), f(Err(\"bad\")))":            "2 Err(\"bad\")\n",
		"fn f(r) -> Ok | Err = Ok(r? * 2)\nprint(f(Ok(2)), f(Err('x)))": "Ok(4) Err('x)\n",
		"print(nil ?? Err(1) ?? 2)":                                     "2\n",
		"fn f(r) = r?\nprint(f(nil))":                                   "nil\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		"Err({kind: 'x, message: \"boom\"})!": "E0603",
		"nil!":                                "E0604",
		"5!":                                  "E0605",
		"fn f(r) = r?\nf(1)":                  "E0605",
	}
	for src, want := range errs {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
	p := compileWith(t, "Err({kind: 'x, message: \"boom\"})!", prelude.Test())
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Error() != "error[E0603]: unhandled Err: boom" {
		t.Fatalf("err = %v", err)
	}
}

func TestRuntimeErrorsM4(t *testing.T) {
	cases := map[string]string{
		"[1][1]":             "E0507",
		"[1][-2]":            "E0507",
		"[1, 2][0..3]":       "E0507",
		"{a: 1}.b":           "E0508",
		"{a: 1}[\"b\"]":      "E0508",
		"(1, 2)[0]":          "E0509",
		"5[0]":               "E0509",
		"(1,)[0..1]":         "E0511",
		"1 in 1":             "E0512",
		"1 in \"a\"":         "E0501",
		"[1][\"a\"]":         "E0501",
		"\"a\"..\"b\"":       "E0501",
		"match 1: 0 => 1 ;;": "E0601",
		"(a, b) = (1, 2, 3)": "E0602",
		"Ok(x) = Err(1)":     "E0602",
		"map(1, x => x)":     "E0506",
	}
	for src, want := range cases {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}
