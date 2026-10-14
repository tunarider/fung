package parser

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/lexer"
)

func TestCollectionLiterals(t *testing.T) {
	cases := map[string]string{
		"[1, 2, 3]":                "(program (list 1 2 3))",
		"[]":                       "(program (list))",
		"[\n  1,\n  2,\n]":         "(program (list 1 2))",
		"{name: \"a\", tags: [1]}": `(program (map (entry name "a") (entry tags (list 1))))`,
		"{\"a b\": 1}":             "(program (map (entry a b 1)))",
		"{}":                       "(program (map))",
		"()":                       "(program (tuple))",
		"(1,)":                     "(program (tuple 1))",
		"(1, 2)":                   "(program (tuple 1 2))",
		"(1)":                      "(program 1)",
		"Circle(3)":                "(program (tag Circle 3))",
		"Rect(2, 5)":               "(program (tag Rect 2 5))",
		"Red":                      "(program 'Red)",
		"Ok(x) + 1":                "(program (binary + (tag Ok x) 1))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
}

func TestPostfixAndNewInfix(t *testing.T) {
	cases := map[string]string{
		"xs[0]":                              "(program (index xs 0))",
		"xs[-1]":                             "(program (index xs (unary - 1)))",
		"xs[1..3]":                           "(program (slice xs 1 3))",
		"xs[..3]":                            "(program (slice xs (nil) 3))",
		"xs[-4..]":                           "(program (slice xs (unary - 4) (nil)))",
		"xs[1..=3]":                          "(program (slice ..= xs 1 3))",
		"xs[i ?? 0]":                         "(program (index xs (coalesce i 0)))",
		"xs[c then 0 else 1]":                "(program (index xs (then c 0 1)))",
		"xs[(i ?? 0)..]":                     "(program (slice xs (coalesce i 0) (nil)))",
		"m.name.first":                       "(program (field first (field name m)))",
		"f(1)!":                              "(program (unwrap ! (call f 1)))",
		"f(1)?":                              "(program (unwrap ? (call f 1)))",
		"a ?? b ?? c":                        "(program (coalesce (coalesce a b) c))",
		"xs |> map(f) |> sum":                "(program (pipe (pipe xs (call map f)) sum))",
		"0..10":                              "(program (range .. 0 10))",
		"0..=10":                             "(program (range ..= 0 10))",
		"x in xs":                            "(program (binary in x xs))",
		"x not in xs":                        "(program (binary not_in x xs))",
		"files |> len > 3":                   "(program (binary > (pipe files len) 3))",
		"a + b |> f":                         "(program (pipe (binary + a b) f))",
		"fs.read(p) ?? \"{}\" |> json.parse": `(program (pipe (coalesce (call (field read fs) p) "{}") (field parse json)))`,
		"xs\n  |> map(f)":                    "(program (pipe xs (call map f)))",
		"m\n  .name":                         "(program (field name m))",
		"f(_)":                               "(program (call f (hole)))",
		"path.join(dir, _)":                  "(program (call (field join path) dir (hole)))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	toks, _, _ := lexer.Lex("t.fung", "0..=10")
	n, _ := Parse(toks)
	if n.Kids[0].Lit != "..=" {
		t.Error("..= should record the inclusive operator")
	}
	toks, _, _ = lexer.Lex("t.fung", "f()?")
	n, _ = Parse(toks)
	if n.Kids[0].Lit != "?" {
		t.Error("unwrap lit")
	}
	for _, src := range []string{"x in xs in ys", "a < b in c", "f(a: _)"} {
		if got := parseErr(t, src); got != "E0111" && got != "E0110" {
			t.Errorf("%q: %s", src, got)
		}
	}
}

func TestPatternsAndMatch(t *testing.T) {
	cases := map[string]string{
		"match v:\n  0 => a\n  _ => b\n;;":                                                          "(program (match v (arm (plit 0) (nil) (block a)) (arm (pwild) (nil) (block b))))",
		"match v: 'ok => 1; Ok(x) => x; Err(e) => 0 ;;":                                             "(program (match v (arm (psym ok) (nil) (block 1)) (arm (ptag Ok (pbind x)) (nil) (block x)) (arm (ptag Err (pbind e)) (nil) (block 0))))",
		"match p: (0, 0) => a; (x, 0) => b ;;":                                                      "(program (match p (arm (ptuple (plit 0) (plit 0)) (nil) (block a)) (arm (ptuple (pbind x) (plit 0)) (nil) (block b))))",
		"match xs: [] => 0; [first, ...rest] => 1; [a, ...] => 2 ;;":                                "(program (match xs (arm (plist) (nil) (block 0)) (arm (plist (pbind first) (prest rest)) (nil) (block 1)) (arm (plist (pbind a) (prest)) (nil) (block 2))))",
		"match m: {name: n, port: 80} => n ;;":                                                      "(program (match m (arm (pmap (pentry name (pbind n)) (pentry port (plit 80))) (nil) (block n))))",
		"match x: n when n > 100 => big; 'a | 'b => ab ;;":                                          "(program (match x (arm (pbind n) (binary > n 100) (block big)) (arm (palt (psym a) (psym b)) (nil) (block ab))))",
		"match v: -1 => neg; \"s\" => s; 1.5 => f; nil => z; true => t; 1h => d ;;":                 `(program (match v (arm (plit -1) (nil) (block neg)) (arm (plit "s") (nil) (block s)) (arm (plit 1.5) (nil) (block f)) (arm (pnil) (nil) (block z)) (arm (plit true) (nil) (block t)) (arm (plit 1h) (nil) (block d))))`,
		"match r:\n  Ok(text) =>\n    lines = f(text)\n    len(lines)\n  Err(e) => print(e); 0\n;;": "(program (match r (arm (ptag Ok (pbind text)) (nil) (block (assign lines (call f text)) (call len lines))) (arm (ptag Err (pbind e)) (nil) (block (call print e) 0))))",
		"match v: Red => 1 ;;":                                                                      "(program (match v (arm (psym Red) (nil) (block 1))))",
		"match v: y when y == x => 1; _ => 2 ;;":                                                    "(program (match v (arm (pbind y) (binary == y x) (block 1)) (arm (pwild) (nil) (block 2))))",
		"match v: y when any(xs, x => x) => 1 ;;":                                                   "(program (match v (arm (pbind y) (call any xs (fn (x) (block x))) (block 1))))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	errs := map[string]string{
		"match v: (x) => 1 ;;":       "E0110", // (p) is not a pattern
		"match v: [...a, b] => 1 ;;": "E0110", // rest must be last
		"match v: 1 + 2 => 1 ;;":     "E0110",
		"match v: ;;":                "E0110", // no arms
		"match v: x => ;;":           "E0110", // empty arm body
	}
	for src, want := range errs {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: %s, want %s", src, got, want)
		}
	}
}

func TestForTypeAndDestructuring(t *testing.T) {
	cases := map[string]string{
		"for x in xs: print(x) ;;":     "(program (for (pbind x) xs (block (call print x))))",
		"for (k, v) in pairs(m): k ;;": "(program (for (ptuple (pbind k) (pbind v)) (call pairs m) (block k)))",
		"for i in 0..10: i ;;":         "(program (for (pbind i) (range .. 0 10) (block i)))",
		"type shape = Circle | Rect":   "(program (typealias shape))",
		"(q, r) = divmod(7, 2)":        "(program (assign (ptuple (pbind q) (pbind r)) (call divmod 7 2)))",
		"[first, ...rest] = files":     "(program (assign (plist (pbind first) (prest rest)) files))",
		"{name: n, port: p} = config":  "(program (assign (pmap (pentry name (pbind n)) (pentry port (pbind p))) config))",
		"Ok(text) = fs.read(\"a\")":    `(program (assign (ptag Ok (pbind text)) (call (field read fs) "a")))`,
		"x = 1":                        "(program (assign x 1))",
		"(a, b)":                       "(program (tuple a b))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	if got := parseErr(t, "for x xs: 1 ;;"); got != "E0112" {
		t.Errorf("for without in: %s", got)
	}
	if got := parseErr(t, "type Shape = Circle"); got != "E0112" {
		t.Errorf("alias must be lowercase: %s", got)
	}
}

func TestTagRejectsNamedArgs(t *testing.T) {
	if got := parseErr(t, "Ok(a: 1)"); got != "E0110" {
		t.Errorf("Ok(a: 1): %s, want E0110", got)
	}
}

func TestDeepPatternIsBounded(t *testing.T) {
	src := strings.Repeat("[", 20000) + "x" + strings.Repeat("]", 20000) + " = 1"
	if got := parseErrQuiet(t, src); got != "E0802" {
		t.Errorf("deep pattern: %s, want E0802", got)
	}
	src = "match v: " + strings.Repeat("Ok(", 20000) + "x" + strings.Repeat(")", 20000) + " => 1 ;;"
	if got := parseErrQuiet(t, src); got != "E0802" {
		t.Errorf("deep pattern in match: %s, want E0802", got)
	}
}

// parseErrQuiet is parseErr for huge sources: it never echoes the input.
func parseErrQuiet(t *testing.T, src string) string {
	t.Helper()
	toks, _, err := lexer.Lex("t.fung", src)
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	_, perr := Parse(toks)
	if perr == nil {
		return ""
	}
	return perr.Code
}
