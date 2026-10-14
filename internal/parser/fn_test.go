package parser

import "testing"

func TestFnDefinitions(t *testing.T) {
	cases := map[string]string{
		"fn add(a, b) = a + b":                            "(program (fn add(a, b) (block (binary + a b))))",
		"fn f():\n  1\n  2\n;;":                           "(program (fn f() (block 1 2)))",
		"fn g(a, b = 1, c = a + b) = c":                   "(program (fn g(a, b=?, c=?) 1 (binary + a b) (block c)))",
		"fn h(n: int, s: str | nil = nil) -> bool = true": "(program (fn h(n:int, s:str|nil=?)->bool (nil) (block true)))",
		"fn k(xs: list[int]) = xs":                        "(program (fn k(xs:list) (block xs)))",
		"fn v(...rest) = rest":                            "(program (fn v(...rest) (block rest)))",
		"fn t(s: Circle) = s":                             "(program (fn t(s:Circle) (block s)))",
		"fn f(a,) = a":                                    "(program (fn f(a) (block a)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestLambdasAndAnonymousFns(t *testing.T) {
	cases := map[string]string{
		"f = x => x * 2":             "(program (assign f (fn (x) (block (binary * x 2)))))",
		"f = (a, b) => a + b":        "(program (assign f (fn (a, b) (block (binary + a b)))))",
		"f = () => 1":                "(program (assign f (fn () (block 1))))",
		"x => y => x + y":            "(program (fn (x) (block (fn (y) (block (binary + x y))))))",
		"x => c then 1 else 2":       "(program (fn (x) (block (then c 1 2))))",
		"g(xs, x => x + 1, 3)":       "(program (call g xs (fn (x) (block (binary + x 1))) 3))",
		"f = fn(a):\n  a\n;;":        "(program (assign f (fn (a) (block a))))",
		"f = fn(a) -> int:\n  a\n;;": "(program (assign f (fn (a)->int (block a))))",
		"(1 + 2) * 3":                "(program (binary * (binary + 1 2) 3))", // still a grouping
		"(x)":                        "(program x)",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestReturnAndNamedArgs(t *testing.T) {
	cases := map[string]string{
		"fn f() = return 1":                   "(program (fn f() (block (return 1))))",
		"fn f():\n  if c: return ;;\n  2\n;;": "(program (fn f() (block (if c (block (return (nil))) (nil)) 2)))",
		"serve(8080, dir: \"./pub\")":         `(program (call serve 8080 (named dir "./pub")))`,
		"f(a: 1, b: 2)":                       "(program (call f (named a 1) (named b 2)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	errs := map[string]string{
		"f(a: 1, 2)":         "E0110", // positional after named
		"fn (a) = a":         "E0110", // anonymous fn only has the block form
		"fn f(a = 1, b) = a": "E0110", // required after optional
		"fn f(a: ) = a":      "E0110",
		"fn f(a, a) = a":     "E0110", // duplicate parameter
		"x =>":               "E0110",
		"fn f() -> = 1":      "E0110",
		"fn f(...a, b) = a":  "E0110", // rest must be last
	}
	for src, want := range errs {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: %s, want %s", src, got, want)
		}
	}
}

func TestLambdaParamsRequireCommas(t *testing.T) {
	if got := parseErr(t, "(a b) => a + b"); got != "E0110" {
		t.Fatalf("got %s, want E0110", got)
	}
}
