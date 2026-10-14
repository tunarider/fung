package desugar

import (
	"testing"

	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/tree"
)

func core(t *testing.T, src string) string {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatal(err)
	}
	surface, perr := parser.Parse(toks)
	if perr != nil {
		t.Fatal(perr)
	}
	c, derr := Desugar(surface)
	if derr != nil {
		t.Fatal(derr)
	}
	return c.String()
}

func TestDesugarArithmeticToOps(t *testing.T) {
	cases := map[string]string{
		"1 + 2 * 3": "(do (op + 1 (op * 2 3)))",
		"-x":        "(do (op - x))",
		"not x":     "(do (op not x))",
		"print(1)":  "(do (call print 1))",
		"1\n2":      "(do 1 2)",
		"":          "(do)",
		"a == b":    "(do (op == a b))",
	}
	for src, want := range cases {
		if got := core(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestDesugarShortCircuitToIf(t *testing.T) {
	cases := map[string]string{
		"a and b":      "(do (if a (op bool b) false))",
		"a or b":       "(do (if a true (op bool b)))",
		"a and b or c": "(do (if (if a (op bool b) false) true (op bool c)))",
	}
	for src, want := range cases {
		if got := core(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestDesugarKeepsPositions(t *testing.T) {
	toks, _, _ := lexer.Lex("t.fung", "1 + 2")
	surface, _ := parser.Parse(toks)
	c, _ := Desugar(surface)
	if got := c.Kids[0].Pos.Col; got != 3 {
		t.Fatalf("op node col = %d, want 3 (the operator)", got)
	}
}

func TestDesugarStatementsAndBlocks(t *testing.T) {
	cases := map[string]string{
		"x = 1":                  "(do (let x 1))",
		"if c: x = 1 else: 2 ;;": "(do (if c (do (let x 1)) (do 2)))",
		"if c: 1 ;;":             "(do (if c (do 1) (nil)))",
		"c then 1 else 2":        "(do (if c 1 2))",
		`"a${x}b"`:               `(do (concat "a" x "b"))`,
		"nil":                    "(do (nil))",
		"'ok":                    "(do 'ok)",
		"1h":                     "(do 1h)",
		`"plain"`:                `(do "plain")`,
		"c then a and b else d":  "(do (if c (if a (op bool b) false) d))",
	}
	for src, want := range cases {
		if got := core(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestDesugarFunctions(t *testing.T) {
	cases := map[string]string{
		"fn add(a, b) = a + b":       "(do (fn add(a, b) (do (op + a b))))",
		"f = x => x":                 "(do (let f (fn (x) (do x))))",
		"fn g(a = 1 + 1) = return a": "(do (fn g(a=?) (op + 1 1) (do (return a))))",
		"f(1, k: 2 * 3)":             "(do (call f 1 (named k (op * 2 3))))",
	}
	for src, want := range cases {
		if got := core(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestDesugarM4(t *testing.T) {
	cases := map[string]string{
		"xs |> map(f) |> sum":        "(do (call sum (call map xs f)))",
		"x |> f":                     "(do (call f x))",
		"x in xs":                    "(do (op in x xs))",
		"x not in xs":                "(do (op not (op in x xs)))",
		"(q, r) = f()":               "(do (let (ptuple (pbind q) (pbind r)) (call f)))",
		"for x in xs: g(x) ;;":       "(do (call (prelude map) xs (fn (x) (do (call g x)))))",
		"for (k, v) in m: k ;;":      "(do (call (prelude map) m (fn ($f0) (do (let fresh (ptuple (pbind k) (pbind v)) $f0) k))))",
		"match v: 0 => a; _ => b ;;": "(do (do (let $m0 v) (match $m0 (arm (plit 0) (nil) (do a)) (arm (pwild) (nil) (do b)))))",
		"f(a, _)":                    "(do (do (let $p0 f) (let $p1 a) (fn (h0) (do (call $p0 $p1 h0)))))",
		"g(_, k(1), _)":              "(do (do (let $p0 g) (let $p1 (call k 1)) (fn (h0, h1) (do (call $p0 h0 $p1 h1)))))",
		"xs[0] + m.k":                "(do (op + (index xs 0) (field k m)))",
		"r!":                         "(do (unwrap ! r))",
		"f(x, k: y, _)":              "", // parser error (hole after named)
	}
	for src, want := range cases {
		if want == "" {
			continue
		}
		if got := core(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestForBodyLambdaIsMarked(t *testing.T) {
	toks, _, _ := lexer.Lex("t.fung", "for x in xs: x ;;")
	s, _ := parser.Parse(toks)
	c, _ := Desugar(s)
	fn := c.Kids[0].Kids[2]
	if info, ok := fn.Lit.(*tree.FnInfo); !ok || !info.ForBody {
		t.Fatalf("for body lambda = %s", fn)
	}
}

func TestChainedPipeWithLambdasWalksEachOperandOnce(t *testing.T) {
	got := core(t, "xs |> filter(x => x) |> map(x => x)")
	want := "(do (call map (call filter xs (fn (x) (do x))) (fn (x) (do x))))"
	if got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	got = core(t, "xs |> (x => x) |> f")
	want = "(do (call f (call (fn (x) (do x)) xs)))"
	if got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
}

func TestTestBlockBecomesAFunction(t *testing.T) {
	got := core(t, "test \"a\":\n  assert f() == 1\n;;")
	want := `(do (test "a" (fn test: a() (do (assert (op == (call f) 1))))))`
	if got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
}

func TestParameterPatternsDestructureAtEntry(t *testing.T) {
	got := core(t, "((k, v)) => k")
	want := "(do (fn ($a0) (do (let fresh (ptuple (pbind k) (pbind v)) $a0) k)))"
	if got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
	got = core(t, "fn f(a, (b, c), d = 1) = a")
	want = "(do (fn f(a, $a0, d=?) 1 (do (let fresh (ptuple (pbind b) (pbind c)) $a0) a)))"
	if got != want {
		t.Fatalf("\n got %s\nwant %s", got, want)
	}
}
