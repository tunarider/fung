package format

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/lexer"
)

// check formats src and insists the result is what we want, that it is
// idempotent, and that the token stream survived.
func check(t *testing.T, src, want string) {
	t.Helper()
	got, err := Format("t.fung", src)
	if err != nil {
		t.Fatalf("%q: %v", src, err)
	}
	if got != want {
		t.Errorf("format %q\n got %q\nwant %q", src, got, want)
	}
	again, err := Format("t.fung", got)
	if err != nil || again != got {
		t.Errorf("not idempotent: %q -> %q -> %q (%v)", src, got, again, err)
	}
	if !sameTokens(src, got) {
		t.Errorf("tokens changed: %q -> %q", src, got)
	}
}

func sameTokens(a, b string) bool {
	ta, ca, _ := lexer.Lex("a", a)
	tb, cb, _ := lexer.Lex("b", b)
	return strings.Join(tokenTexts(a, ta), "\x00") == strings.Join(tokenTexts(b, tb), "\x00") && len(ca) == len(cb)
}

func tokenTexts(src string, toks []lexer.Token) []string {
	var out []string
	for _, t := range toks {
		if t.Kind != lexer.Newline && t.Kind != lexer.EOF {
			out = append(out, src[t.Off:t.End])
		}
	}
	return out
}

func TestSpacing(t *testing.T) {
	cases := map[string]string{
		"a+b*c":                  "a + b * c\n",
		"x=1":                    "x = 1\n",
		"f( a,b )":               "f(a, b)\n",
		"xs [ 0 ]":               "xs[0]\n",
		"m [ \"k\" ]":            "m[\"k\"]\n",
		"f(x)(y)":                "f(x)(y)\n",
		"{ a : 1 , b:2 }":        "{a: 1, b: 2}\n",
		"{a, b}":                 "{a, b}\n",
		"[ 1,2 ]":                "[1, 2]\n",
		"( 1, )":                 "(1,)\n",
		"()":                     "()\n",
		"m . a . b":              "m.a.b\n",
		"0 .. n":                 "0..n\n",
		"0..=10":                 "0..=10\n",
		"[ a, ... rest ] = xs":   "[a, ...rest] = xs\n",
		"x=>x+1":                 "x => x + 1\n",
		"(a,b)=>a":               "(a, b) => a\n",
		"()=>work()":             "() => work()\n",
		"fs.read(p) !":           "fs.read(p)!\n",
		"parse(s) ?":             "parse(s)?\n",
		"x ?? y":                 "x ?? y\n",
		"xs|>map(f)|>sum":        "xs |> map(f) |> sum\n",
		"- x":                    "-x\n",
		"a - b":                  "a - b\n",
		"a - -b":                 "a - -b\n",
		"f(-1, - 2)":             "f(-1, -2)\n",
		"x = - 5s":               "x = -5s\n",
		"return - 1":             "return -1\n",
		"not  x":                 "not x\n",
		"if(a==b): 1 ;;":         "if (a == b): 1 ;;\n",
		"fn add(a,b)=a+b":        "fn add(a, b) = a + b\n",
		"fn f(x:int)->bool=true": "fn f(x: int) -> bool = true\n",
		"fn ( x ) : x ;;":        "fn(x): x ;;\n",
		"each(xs, fn(x): x ;;)":  "each(xs, fn(x): x ;;)\n",
		"exec(cmd, cwd : d)":     "exec(cmd, cwd: d)\n",
		"match x: 0=>1;_=>2;;":   "match x: 0 => 1; _ => 2 ;;\n",
		"match v: 1|2 => 'a ;;":  "match v: 1 | 2 => 'a ;;\n",
		"Ok( x )":                "Ok(x)\n",
		"c then a else b":        "c then a else b\n",
		"x in [1, 2]":            "x in [1, 2]\n",
		"\"a ${ x+1 } b\"":       "\"a ${ x+1 } b\"\n",
		"`raw  text`":            "`raw  text`\n",
		"1_000 + 0xff + 2 ** 10": "1_000 + 0xff + 2 ** 10\n",
		"a;b":                    "a; b\n",
		"fn f(): a ;;":           "fn f(): a ;;\n",
		"x   \n":                 "x\n",
		"":                       "",
	}
	for src, want := range cases {
		check(t, src, want)
	}
}

func TestRejectsSyntaxErrors(t *testing.T) {
	if _, err := Format("t.fung", "f(\n"); err == nil || err.Code != "E0110" {
		t.Errorf("a syntax error must come back untouched: %v", err)
	}
	if _, err := Format("t.fung", "\"open"); err == nil || err.Code != "E0104" {
		t.Errorf("a lex error must come back: %v", err)
	}
}

func TestIndentation(t *testing.T) {
	cases := map[string]string{
		// blocks
		"fn f(x):\nx + 1\n;;\n":                   "fn f(x):\n  x + 1\n;;\n",
		"fn f(x):\n    if x:\n  1\n  ;;\n;;\n":    "fn f(x):\n  if x:\n    1\n  ;;\n;;\n",
		"if c:\na\nelse:\nb\n;;\n":                "if c:\n  a\nelse:\n  b\n;;\n",
		"if c:\na\nelse if d:\nb\nelse:\nc\n;;\n": "if c:\n  a\nelse if d:\n  b\nelse:\n  c\n;;\n",
		"test \"t\":\nassert true\n;;\n":          "test \"t\":\n  assert true\n;;\n",
		"for x in xs:\nprint(x)\n;;\n":            "for x in xs:\n  print(x)\n;;\n",
		// then/else continuation vs if-chain else
		"x = c then a\nelse b\n":                            "x = c then a\n  else b\n",
		"if c:\n  x = d then 1\n  else 2\nelse:\n  3\n;;\n": "if c:\n  x = d then 1\n    else 2\nelse:\n  3\n;;\n",
		// match arms and bodies
		"match v:\nOk(x) =>\nx\nErr(e) => 0\n;;\n": "match v:\n  Ok(x)  =>\n    x\n  Err(e) => 0\n;;\n",
		"match v:\n  Ok(x) => f(y => y)\n;;\n":     "match v:\n  Ok(x) => f(y => y)\n;;\n",
		// brackets spanning lines; closers go back to the opener's line
		"f(a,\nb,\nc)\n":      "f(a,\n  b,\n  c)\n",
		"xs = [\n1,\n2,\n]\n": "xs = [\n  1,\n  2,\n]\n",
		"m = {\na: 1,\n}\n":   "m = {\n  a: 1,\n}\n",
		// multi-line lambda inside a call: body +1, `;;)` at the call's line
		"each(xs, fn(x):\nprint(x)\n;;)\n":              "each(xs, fn(x):\n  print(x)\n;;)\n",
		"fn g():\neach(xs, fn(x):\nprint(x)\n;;)\n;;\n": "fn g():\n  each(xs, fn(x):\n    print(x)\n  ;;)\n;;\n",
		// continuation lines
		"xs\n|> map(f)\n|> sum\n":       "xs\n  |> map(f)\n  |> sum\n",
		"a = b\n.c\n.d\n":               "a = b\n  .c\n  .d\n",
		"x = a\nand b\nor c\n":          "x = a\n  and b\n  or c\n",
		"v = f(x) ?? 1\n":               "v = f(x) ?? 1\n",
		"fn f():\n  xs\n  |> sum\n;;\n": "fn f():\n  xs\n    |> sum\n;;\n",
		// map colon and type annotation never open a block
		"m = {a: 1}\nx = 2\n":                                  "m = {a: 1}\nx = 2\n",
		"fn f(x: int) -> int = x\ny = 1\n":                     "fn f(x: int) -> int = x\ny = 1\n",
		"fn f(a) = if a: 1 else: 2 ;;\nmatch x:\n0 => 1\n;;\n": "fn f(a) = if a: 1 else: 2 ;;\nmatch x:\n  0 => 1\n;;\n",
		// raw string spanning lines
		"s = `a\n  b`\nx = 1\n": "s = `a\n  b`\nx = 1\n",
		// one-line forms are left alone
		"match x: 0 => 1; _ => 2 ;;\n": "match x: 0 => 1; _ => 2 ;;\n",
		"if c: a else: b ;;\n":         "if c: a else: b ;;\n",
	}
	for src, want := range cases {
		check(t, src, want)
	}
}

func TestBlankLinesAndComments(t *testing.T) {
	cases := map[string]string{
		"\n\nx = 1\n\n\n\ny = 2\n\n":                            "x = 1\n\ny = 2\n",
		"fn f():\n\n  x\n\n;;\n":                                "fn f():\n  x\n;;\n",
		"#!/usr/bin/env fung\n# about\nx = 1\n":                 "#!/usr/bin/env fung\n# about\nx = 1\n",
		"x = 1 # one\n":                                         "x = 1  # one\n",
		"x = 1    # one\nlong_name = 2 # two\n":                 "x = 1          # one\nlong_name = 2  # two\n",
		"x = 1 # one\ny = 2\nlong = 3 # three\n":                "x = 1  # one\ny = 2\nlong = 3  # three\n",
		"fn f():\n# inside\nx\n# before end\n;;\n# after\n":     "fn f():\n  # inside\n  x\n  # before end\n;;\n# after\n",
		"match v:\nOk(x) => 1\nErr(e) => 2\n;;\n":               "match v:\n  Ok(x)  => 1\n  Err(e) => 2\n;;\n",
		"match v:\nOk(text) =>\nlen(text)\nErr(e)   => 0\n;;\n": "match v:\n  Ok(text) =>\n    len(text)\n  Err(e)   => 0\n;;\n",
		"match v:\n  0 => a\n\n  _ => other\n;;\n":              "match v:\n  0 => a\n\n  _ => other\n;;\n",
		"# only a comment":                                      "# only a comment\n",
		"print(1)\n# expect: 1\nprint(2)\n# expect: 2\n":        "print(1)\n# expect: 1\nprint(2)\n# expect: 2\n",
		"f(a, # first\n  b) # second\n":                         "f(a,  # first\n  b)  # second\n",
	}
	for src, want := range cases {
		check(t, src, want)
	}
}

// Found by formatting the repository: import lists keep their space, and
// a line ending in an operator continues on the next line.
func TestRepositoryFindings(t *testing.T) {
	cases := map[string]string{
		"import \"./x\" (a, b)\n":                "import \"./x\" (a, b)\n",
		"import fs (read)\n":                     "import fs (read)\n",
		"import fs(read)\n":                      "import fs (read)\n",
		"fn f(x) =\nx + 1\n":                     "fn f(x) =\n  x + 1\n",
		"fn f(x) =\n  x + 1\ny = 2\n":            "fn f(x) =\n  x + 1\ny = 2\n",
		"total = a +\nb\n":                       "total = a +\n  b\n",
		"xs |> map(x =>\nx + 1)\n":               "xs |> map(x =>\n  x + 1)\n",
		"fn g():\n  v = f(x) ??\n  d\n  v\n;;\n": "fn g():\n  v = f(x) ??\n    d\n  v\n;;\n",
		"match v:\n  Ok(t) =>\n    t\n;;\n":      "match v:\n  Ok(t) =>\n    t\n;;\n",
	}
	for src, want := range cases {
		check(t, src, want)
	}
}

// From the branch review: inputs the first version got wrong.
func TestReviewFindings(t *testing.T) {
	cases := map[string]string{
		// tokens or a comment after a multi-line raw string stay on its last line
		"x = f(`a\nb`)\n":         "x = f(`a\nb`)\n",
		"x = `a\nb` + \"c\"\n":    "x = `a\nb` + \"c\"\n",
		"x = `a\nb` # note\n":     "x = `a\nb`  # note\n",
		"xs = [`a\nb`,\n  `c`]\n": "xs = [`a\nb`,\n  `c`]\n",
		// a lambda statement inside an arm body is not an arm (01: 절은 패턴 [when] =>)
		"match a:\n  Some(v) =>\n    y = 2\n    g = x => x + v\n    g(y)\n  _ => 0\n;;\n": "match a:\n  Some(v) =>\n    y = 2\n    g = x => x + v\n    g(y)\n  _       => 0\n;;\n",
		"match a:\n  Some(v) =>\n    return x => x\n  _ => 0\n;;\n":                       "match a:\n  Some(v) =>\n    return x => x\n  _       => 0\n;;\n",
		// an inner match aligns on its own even when an outer arm follows
		"match a:\n  Some(x) =>\n    match x:\n      1 => 2\n      100 => 3\n    ;;\n  _ => 0\n;;\n": "match a:\n  Some(x) =>\n    match x:\n      1   => 2\n      100 => 3\n    ;;\n  _       => 0\n;;\n",
		// a body that ends with `;;` at the arm indent does not break the run
		"match a:\n  Some(x) => each(xs, fn(y):\n    y\n  ;;)\n  _ => 0\n;;\n": "match a:\n  Some(x) => each(xs, fn(y):\n    y\n  ;;)\n  _       => 0\n;;\n",
		// `-` after a keyword used as a field name is a subtraction
		"x = m.type - 1\n":  "x = m.type - 1\n",
		"x = {type: - 1}\n": "x = {type: -1}\n",
		// trailing whitespace inside a comment goes
		"x = 1 # c   \n": "x = 1  # c\n",
		"# top   \n":     "# top\n",
	}
	for src, want := range cases {
		check(t, src, want)
	}
}

// A `fn` type annotation inside a parameter list is not a block keyword
// waiting for a `:` after the list closes: the body stays indented.
func TestFnTypedParameters(t *testing.T) {
	cases := map[string]string{
		"fn f(h: fn, x = 1):\n  y = x\n  h(y)\n;;\n":      "fn f(h: fn, x = 1):\n  y = x\n  h(y)\n;;\n",
		"fn f(h: fn):\nh()\n;;\n":                         "fn f(h: fn):\n  h()\n;;\n",
		"fn f(a: fn, b: fn, c: str):\na(b(c))\n;;\n":      "fn f(a: fn, b: fn, c: str):\n  a(b(c))\n;;\n",
		"fn f(h: fn = print):\nh(1)\n;;\n":                "fn f(h: fn = print):\n  h(1)\n;;\n",
		"fn f(h: fn, xs):\nfor x in xs:\nh(x)\n;;\n;;\n":  "fn f(h: fn, xs):\n  for x in xs:\n    h(x)\n  ;;\n;;\n",
		"fn f(h: fn):\nif h:\n1\nelse:\n2\n;;\n;;\nx=1\n": "fn f(h: fn):\n  if h:\n    1\n  else:\n    2\n  ;;\n;;\nx = 1\n",
		"fn g(h: fn):\neach(xs, fn(x):\nh(x)\n;;)\n;;\n":  "fn g(h: fn):\n  each(xs, fn(x):\n    h(x)\n  ;;)\n;;\n",
		"fn f(h: fn, x: int) = h(x)\ny = 1\n":             "fn f(h: fn, x: int) = h(x)\ny = 1\n",
		"fn f() -> fn:\nprint\n;;\n":                      "fn f() -> fn:\n  print\n;;\n",
		"fn f(h: fn | nil, g: list[fn]):\nh\n;;\n":        "fn f(h: fn | nil, g: list[fn]):\n  h\n;;\n",
		"apply(fn(x): x ;;, fn(y):\ny\n;;)\nz = 1\n":      "apply(fn(x): x ;;, fn(y):\n  y\n;;)\nz = 1\n",
		"fn g():\nh = fn(x: fn):\nx()\n;;\nh\n;;\n":       "fn g():\n  h = fn(x: fn):\n    x()\n  ;;\n  h\n;;\n",
	}
	for src, want := range cases {
		check(t, src, want)
	}
}

// A leading byte order mark is skipped like the lexer skips it and never
// written back; a second mark is code the lexer refuses (E0101).
func TestByteOrderMark(t *testing.T) {
	cases := map[string]string{
		"\ufeffx=1\n":                "x = 1\n",
		"\ufefffn f():\nx\n;;\n":     "fn f():\n  x\n;;\n",
		"\ufeff# only a comment\n":   "# only a comment\n",
		"\ufeff":                     "",
		"\ufeff\n\nx = 1 # c   \n\n": "x = 1  # c\n",
	}
	for src, want := range cases {
		got, err := Format("t.fung", src)
		if err != nil || got != want {
			t.Errorf("format %q\n got %q (%v)\nwant %q", src, got, err, want)
		}
	}
	if _, err := Format("t.fung", "\ufeff\ufeffx = 1\n"); err == nil || err.Code != "E0101" {
		t.Errorf("a doubled mark must be refused like the lexer refuses it: %v", err)
	}
}
