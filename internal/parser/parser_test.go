package parser

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/lexer"
)

func parse(t *testing.T, src string) string {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatalf("lex %q: %v", src, err)
	}
	n, perr := Parse(toks)
	if perr != nil {
		t.Fatalf("parse %q: %v", src, perr)
	}
	return n.String()
}

func parseErr(t *testing.T, src string) string {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatalf("lex %q: %v", src, err)
	}
	_, perr := Parse(toks)
	if perr == nil {
		t.Fatalf("parse %q: want error", src)
	}
	return perr.Code
}

func TestPrecedenceAndAssociativity(t *testing.T) {
	cases := map[string]string{
		"1 + 2 * 3":       "(program (binary + 1 (binary * 2 3)))",
		"(1 + 2) * 3":     "(program (binary * (binary + 1 2) 3))",
		"1 - 2 - 3":       "(program (binary - (binary - 1 2) 3))",
		"2 ** 3 ** 2":     "(program (binary ** 2 (binary ** 3 2)))",
		"-2 ** 2":         "(program (unary - (binary ** 2 2)))",
		"2 ** -1":         "(program (binary ** 2 (unary - 1)))",
		"-7 // 2":         "(program (binary // (unary - 7) 2))",
		"7 % 3 / 2":       "(program (binary / (binary % 7 3) 2))",
		"1 < 2 and 2 < 3": "(program (binary and (binary < 1 2) (binary < 2 3)))",
		"not a or b":      "(program (binary or (unary not a) b))",
		"not a == b":      "(program (unary not (binary == a b)))",
		"a or b and c":    "(program (binary or a (binary and b c)))",
		"1 == 1.0":        "(program (binary == 1 1.0))",
		"true != false":   "(program (binary != true false))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestCalls(t *testing.T) {
	cases := map[string]string{
		"print(1 + 2 * 3)":     "(program (call print (binary + 1 (binary * 2 3))))",
		"print()":              "(program (call print))",
		"print(1, 2,)":         "(program (call print 1 2))",
		"f(1)(2)":              "(program (call (call f 1) 2))",
		"print(\n  1,\n  2\n)": "(program (call print 1 2))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestStatementsAndNewlines(t *testing.T) {
	cases := map[string]string{
		"":                  "(program)",
		"\n\n":              "(program)",
		"1\n2":              "(program 1 2)",
		"1; 2;":             "(program 1 2)",
		"1 +\n\n  2":        "(program (binary + 1 2))",
		"true\n  and false": "(program (binary and true false))",
		"1\n-2":             "(program 1 (unary - 2))",
		"f\n(1)":            "(program f 1)",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestSyntaxErrors(t *testing.T) {
	cases := map[string]string{
		"1 < 2 < 3":   "E0111",
		"a == b == c": "E0111",
		"1 +":         "E0110",
		"(1":          "E0110",
		"1 2":         "E0110",
		")":           "E0110",
		"print(1,,2)": "E0110",
		"1 ;; 2":      "E0110",
	}
	for src, want := range cases {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: code = %s, want %s", src, got, want)
		}
	}
}

func TestErrorPositionPointsAtOffendingToken(t *testing.T) {
	toks, _, _ := lexer.Lex("t.fung", "print(1 2)")
	_, err := Parse(toks)
	if err == nil || err.Pos.Line != 1 || err.Pos.Col != 9 {
		t.Fatalf("err = %v", err)
	}
}

func TestDeepNestingIsAFungErrorNotAGoCrash(t *testing.T) {
	deep := strings.Repeat("(", 20000) + "1" + strings.Repeat(")", 20000)
	if got := parseErr(t, deep); got != "E0802" {
		t.Fatalf("code = %s, want E0802", got)
	}
	fine := strings.Repeat("(", 1000) + "1" + strings.Repeat(")", 1000)
	if got := parse(t, fine); got != "(program 1)" {
		t.Fatalf("1000-deep nesting: %s", got)
	}
}

func TestAssignmentStatements(t *testing.T) {
	cases := map[string]string{
		"x = 1":            "(program (assign x 1))",
		"x = 1\ny = x + 1": "(program (assign x 1) (assign y (binary + x 1)))",
		"x =\n  1 + 2":     "(program (assign x (binary + 1 2)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	if got := parseErr(t, "x = y = 1"); got != "E0110" {
		t.Errorf("chained assignment: %s, want E0110", got)
	}
	if got := parseErr(t, "x == 1 = 2"); got != "E0110" {
		t.Errorf("assignment to expression: %s, want E0110", got)
	}
}

func TestLiterals(t *testing.T) {
	cases := map[string]string{
		"nil":           "(program (nil))",
		`"a b"`:         `(program "a b")`,
		"`raw ${x}`":    `(program "raw \${x}")`,
		"'ok":           "(program 'ok)",
		"1h30m":         "(program 1h30m)",
		`"x=${x + 1}!"`: `(program (interp "x=" (binary + x 1) "!"))`,
		`"${a}${b}"`:    "(program (interp a b))",
		`"${m}"`:        "(program (interp m))",
		`"${"in" + s}"`: `(program (interp (binary + "in" s)))`,
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestInterpolationErrorsKeepOuterPositions(t *testing.T) {
	toks, _, _ := lexer.Lex("t.fung", "x = 1\nprint(\"a ${1 +} b\")")
	_, err := Parse(toks)
	if err == nil || err.Code != "E0110" || err.Pos.Line != 2 || err.Pos.Col != 15 {
		t.Fatalf("err = %v, want E0110 at 2:15 (the closing brace)", err)
	}
}

func TestThenElse(t *testing.T) {
	cases := map[string]string{
		"c then 1 else 2":                     "(program (then c 1 2))",
		"a then 1 else b then 2 else 3":       "(program (then a 1 (then b 2 3)))",
		"x = n > 0 then \"pos\" else \"neg\"": `(program (assign x (then (binary > n 0) "pos" "neg")))`,
		"c then 1\n  else 2":                  "(program (then c 1 2))",
		"c then a or b else d":                "(program (then c (binary or a b) d))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	if got := parseErr(t, "c then 1"); got != "E0112" {
		t.Errorf("then without else: %s, want E0112", got)
	}
}

func TestIfBlocks(t *testing.T) {
	cases := map[string]string{
		"if c: 1 ;;":                          "(program (if c (block 1) (nil)))",
		"if c: 1 else: 2 ;;":                  "(program (if c (block 1) (block 2)))",
		"if a: 1 else if b: 2 ;;":             "(program (if a (block 1) (if b (block 2) (nil))))",
		"if a: 1 else if b: 2 else: 3 ;;":     "(program (if a (block 1) (if b (block 2) (block 3))))",
		"if c:\n  x = 1\n  x\nelse:\n  2\n;;": "(program (if c (block (assign x 1) x) (block 2)))",
		"if c: ;;":                            "(program (if c (block) (nil)))",
		"x = if c: 1 else: 2 ;;":              "(program (assign x (if c (block 1) (block 2))))",
		"print(if c:\n  1\nelse:\n  2\n;;)":   "(program (call print (if c (block 1) (block 2))))",
		"if c: if d: 1 ;; else: 2 ;;":         "(program (if c (block (if d (block 1) (nil))) (block 2)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	errs := map[string]string{
		"if c 1 ;;":         "E0112", // missing colon
		"if c: 1":           "E0112", // missing ;;
		"if c: 1 else 2 ;;": "E0112", // else needs colon or if
		"else: 1 ;;":        "E0110",
		";;":                "E0110",
		"if c: 1 ;; 2":      "E0110", // `;;` is not a statement separator (01 EBNF)
	}
	for src, want := range errs {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: %s, want %s", src, got, want)
		}
	}
}

func TestDeeplyNestedBlocksHitTheDepthLimit(t *testing.T) {
	src := strings.Repeat("if true: ", 20000) + "1" + strings.Repeat(" ;;", 20000)
	if got := parseErr(t, src); got != "E0802" {
		t.Fatalf("code = %s, want E0802", got)
	}
}

func TestMinIntLiteral(t *testing.T) {
	if got := parse(t, "-9223372036854775808"); got != "(program -9223372036854775808)" {
		t.Fatalf("got %s", got)
	}
	if got := parse(t, "- 9_223_372_036_854_775_808"); got != "(program -9223372036854775808)" {
		t.Fatalf("with separators: got %s", got)
	}
	for _, src := range []string{"9223372036854775808", "1 + 9223372036854775808", "-(9223372036854775808)"} {
		if got := parseErr(t, src); got != "E0102" {
			t.Errorf("%q: %s, want E0102", src, got)
		}
	}
	if got := parse(t, "-9223372036854775807"); got != "(program (unary - 9223372036854775807))" {
		t.Fatalf("ordinary negative: %s", got)
	}
}
