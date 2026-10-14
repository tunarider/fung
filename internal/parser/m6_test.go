package parser

import (
	"testing"

	"github.com/tunarider/fung/internal/lexer"
)

func TestTestAndAssertStatements(t *testing.T) {
	cases := map[string]string{
		"test \"spaces\":\n  assert f(1) == 2\n;;": `(program (test "spaces" (block (assert (binary == (call f 1) 2)))))`,
		"test \"x\": assert true ;;":               `(program (test "x" (block (assert true))))`,
		"assert x":                                 "(program (assert x))",
		"fn f():\n  assert x > 0\n  x\n;;":         "(program (fn f() (block (assert (binary > x 0)) x)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	errs := map[string]string{
		"test:":                        "E0112",
		"test \"a\" 1":                 "E0112",
		"test \"${x}\": 1 ;;":          "E0112",
		"assert":                       "E0110",
		"fn f(): test \"a\": 1 ;; ;;":  "E0114",
		"if true: test \"a\": 1 ;; ;;": "E0114",
	}
	for src, want := range errs {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: %s, want %s", src, got, want)
		}
	}
}

func TestExpressionSpans(t *testing.T) {
	src := "y = 참치(1, 2) |> f\nassert 참치(1) == 2\nz = (1 + 2) * xs[0]"
	toks, _, _ := lexer.Lex("t.fung", src)
	prog, err := Parse(toks)
	if err != nil {
		t.Fatal(err)
	}
	assign := prog.Kids[0]
	if got := assign.Kids[1].Text(src); got != "참치(1, 2) |> f" {
		t.Errorf("rhs span = %q", got)
	}
	if got := assign.Kids[1].Kids[0].Text(src); got != "참치(1, 2)" {
		t.Errorf("call span = %q", got)
	}
	if got := assign.Text(src); got != "y = 참치(1, 2) |> f" {
		t.Errorf("assignment span = %q", got)
	}
	cmp := prog.Kids[1].Kids[0]
	if cmp.Text(src) != "참치(1) == 2" || cmp.Kids[0].Text(src) != "참치(1)" || cmp.Kids[1].Text(src) != "2" {
		t.Errorf("compare spans = %q / %q / %q", cmp.Text(src), cmp.Kids[0].Text(src), cmp.Kids[1].Text(src))
	}
	if prog.Kids[1].Text(src) != "assert 참치(1) == 2" {
		t.Errorf("statement span = %q", prog.Kids[1].Text(src))
	}
	mul := prog.Kids[2].Kids[1]
	if mul.Text(src) != "(1 + 2) * xs[0]" || mul.Kids[0].Text(src) != "(1 + 2)" || mul.Kids[1].Text(src) != "xs[0]" {
		t.Errorf("paren/index spans = %q / %q / %q", mul.Text(src), mul.Kids[0].Text(src), mul.Kids[1].Text(src))
	}
}

func TestIdentifierShapedKeysIgnoreCase(t *testing.T) {
	cases := map[string]string{
		"{DB_HOST: 1, name: 2}":         "(program (map (entry DB_HOST 1) (entry name 2)))",
		"m.DB_HOST":                     "(program (field DB_HOST m))",
		"match m: {DB_HOST: v} => v ;;": "(program (match m (arm (pmap (pentry DB_HOST (pbind v))) (nil) (block v))))",
		"{Ok: 1}":                       "(program (map (entry Ok 1)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestParameterPatterns(t *testing.T) {
	cases := map[string]string{
		"((a, b)) => a":             "(program (fn ((ptuple (pbind a) (pbind b))) (block a)))",
		"(a, b) => a":               "(program (fn (a, b) (block a)))",
		"_ => 0":                    "(program (fn ((pwild)) (block 0)))",
		"(Ok(x), {name: n}) => x":   "(program (fn ((ptag Ok (pbind x)), (pmap (pentry name (pbind n)))) (block x)))",
		"fn f((a, b), c) = a":       "(program (fn f((ptuple (pbind a) (pbind b)), c) (block a)))",
		"fn f([x, ...rest], _) = x": "(program (fn f((plist (pbind x) (prest rest)), (pwild)) (block x)))",
		"fn f(_, y = 1) = y":        "(program (fn f((pwild), y=?) 1 (block y)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	errs := map[string]string{
		"fn f((a, b) = 1) = a":  "E0110",
		"fn f((a, b): int) = a": "E0110",
		"(1 + 2) => 3":          "E0110",
	}
	for src, want := range errs {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: %s, want %s", src, got, want)
		}
	}
}
