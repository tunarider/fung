package lexer

import (
	"testing"

	"github.com/tunarider/fung/internal/diag"
)

func TestPlainStringAndEscapes(t *testing.T) {
	tok := one(t, `"a\tb\n\\\"\$\u{1F600}"`)
	if tok.Kind != String || len(tok.Parts) != 1 || tok.Parts[0].IsCode {
		t.Fatalf("tok = %+v", tok)
	}
	if want := "a\tb\n\\\"$\U0001F600"; tok.Parts[0].Text != want {
		t.Fatalf("text = %q, want %q", tok.Parts[0].Text, want)
	}
}

func TestInterpolationParts(t *testing.T) {
	tok := one(t, `"x = ${x + 1}, m = ${m["k"]}!"`)
	want := []Part{
		{Text: "x = "},
		{IsCode: true, Text: "x + 1", Pos: diag.Pos{File: "t.fung", Line: 1, Col: 8}},
		{Text: ", m = "},
		{IsCode: true, Text: `m["k"]`, Pos: diag.Pos{File: "t.fung", Line: 1, Col: 22}},
		{Text: "!"},
	}
	if len(tok.Parts) != len(want) {
		t.Fatalf("parts = %+v", tok.Parts)
	}
	for i, w := range want {
		got := tok.Parts[i]
		if got.IsCode != w.IsCode || got.Text != w.Text || (w.IsCode && got.Pos != w.Pos) {
			t.Errorf("part %d = %+v, want %+v", i, got, w)
		}
	}
}

func TestNestedBracesInsideInterpolation(t *testing.T) {
	tok := one(t, `"${ {a: 1}.a }"`)
	if len(tok.Parts) != 1 || tok.Parts[0].Text != " {a: 1}.a " {
		t.Fatalf("parts = %+v", tok.Parts)
	}
}

func TestEmptyString(t *testing.T) {
	tok := one(t, `""`)
	if tok.Kind != String || len(tok.Parts) != 0 {
		t.Fatalf("tok = %+v", tok)
	}
}

func TestRawString(t *testing.T) {
	tok := one(t, "`C:\\new\nline ${x}`")
	if tok.Kind != RawString || tok.Text != "C:\\new\nline ${x}" {
		t.Fatalf("tok = %+v", tok)
	}
	toks, _, _ := Lex("t.fung", "`a\nb` c")
	if toks[1].Pos != (diag.Pos{File: "t.fung", Line: 2, Col: 4}) {
		t.Fatalf("token after multi-line raw string at %v", toks[1].Pos)
	}
}

func TestSymbols(t *testing.T) {
	toks, _, err := Lex("t.fung", "'ok 'not_found 'Red")
	if err != nil {
		t.Fatal(err)
	}
	for i, want := range []string{"ok", "not_found", "Red"} {
		if toks[i].Kind != Symbol || toks[i].Text != want {
			t.Errorf("token %d = %v, want Symbol %q", i, toks[i], want)
		}
	}
}

func TestStringErrors(t *testing.T) {
	cases := map[string]string{
		`"abc`:     "E0104",
		"\"a\nb\"": "E0104", // literal newline
		`"\q"`:     "E0103",
		`"\u{zz}"`: "E0103",
		`"${x"`:    "E0104",
		"`abc":     "E0104",
		"'":        "E0101",
		"' x":      "E0101",
	}
	for src, want := range cases {
		_, _, err := Lex("t.fung", src)
		if err == nil || err.Code != want {
			t.Errorf("%q: err = %v, want %s", src, err, want)
		}
	}
}

func TestSurrogateEscapeIsRejected(t *testing.T) {
	for _, src := range []string{`"\u{D800}"`, `"\u{DFFF}"`} {
		if _, _, err := Lex("t.fung", src); err == nil || err.Code != "E0103" {
			t.Errorf("%s: err = %v, want E0103", src, err)
		}
	}
	if _, _, err := Lex("t.fung", `"\u{E000}"`); err != nil {
		t.Errorf("E000 is fine: %v", err)
	}
}
