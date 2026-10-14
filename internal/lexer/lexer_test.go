package lexer

import (
	"testing"

	"github.com/tunarider/fung/internal/diag"
)

// kinds returns the token kinds of a source, dropping the final EOF.
func kinds(t *testing.T, src string) []Kind {
	t.Helper()
	toks, _, err := Lex("t.fung", src)
	if err != nil {
		t.Fatalf("Lex(%q): %v", src, err)
	}
	var ks []Kind
	for _, tok := range toks[:len(toks)-1] {
		ks = append(ks, tok.Kind)
	}
	return ks
}

func equalKinds(a, b []Kind) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func TestPunctuationLongestMatch(t *testing.T) {
	cases := map[string][]Kind{
		"( ) [ ] { } , : ;":         {LParen, RParen, LBracket, RBracket, LBrace, RBrace, Comma, Colon, Semi},
		";; ;;; ;;;;":               {DoubleSemi, DoubleSemi, Semi, DoubleSemi, DoubleSemi},
		". .. ..= ...":              {Dot, DotDot, DotDotEq, Ellipsis},
		"= == => -> - <= < >= > !=": {Assign, Eq, FatArrow, Arrow, Minus, Le, Lt, Ge, Gt, Ne},
		"+ * ** / // % |> | ?? ? !": {Plus, Star, StarStar, Slash, SlashSlash, Percent, Pipe, Bar, Coalesce, Question, Bang},
		"a;;b":                      {Name, DoubleSemi, Name},
	}
	for src, want := range cases {
		if got := kinds(t, src); !equalKinds(got, want) {
			t.Errorf("%q: got %v, want %v", src, got, want)
		}
	}
}

func TestIdentifiersKeywordsAndTags(t *testing.T) {
	toks, _, err := Lex("t.fung", "fn is_log Ok _x 파일목록 Red true nil x1")
	if err != nil {
		t.Fatal(err)
	}
	want := []struct {
		kind Kind
		text string
	}{
		{KwFn, "fn"}, {Name, "is_log"}, {Tag, "Ok"}, {Name, "_x"}, {Name, "파일목록"},
		{Tag, "Red"}, {KwTrue, "true"}, {KwNil, "nil"}, {Name, "x1"},
	}
	for i, w := range want {
		if toks[i].Kind != w.kind || toks[i].Text != w.text {
			t.Errorf("token %d = %v %q, want %v %q", i, toks[i].Kind, toks[i].Text, w.kind, w.text)
		}
	}
}

func TestAllKeywordsAreRecognized(t *testing.T) {
	for word, kind := range Keywords() {
		toks, _, err := Lex("t.fung", word)
		if err != nil {
			t.Fatal(err)
		}
		if toks[0].Kind != kind {
			t.Errorf("%q lexed as %v, want %v", word, toks[0].Kind, kind)
		}
	}
	if len(Keywords()) != 20 {
		t.Errorf("Keywords() has %d entries, want 20 (01. 예약어)", len(Keywords()))
	}
}

func TestNewlinesCommentsAndPositions(t *testing.T) {
	src := "a # first\r\n  b\n\n# only comment\nc"
	toks, comments, err := Lex("t.fung", src)
	if err != nil {
		t.Fatal(err)
	}
	wantKinds := []Kind{Name, Newline, Name, Newline, Newline, Newline, Name, EOF}
	for i, k := range wantKinds {
		if toks[i].Kind != k {
			t.Fatalf("token %d = %v, want %v (all: %v)", i, toks[i].Kind, k, toks)
		}
	}
	wantPos := []diag.Pos{
		{File: "t.fung", Line: 1, Col: 1},  // a
		{File: "t.fung", Line: 1, Col: 10}, // the \r\n counts as one newline at the \r
		{File: "t.fung", Line: 2, Col: 3},  // b
	}
	for i, p := range wantPos {
		if toks[i].Pos != p {
			t.Errorf("token %d pos = %v, want %v", i, toks[i].Pos, p)
		}
	}
	if len(comments) != 2 || comments[0].Text != "# first" || comments[1].Text != "# only comment" {
		t.Fatalf("comments = %+v", comments)
	}
	if comments[1].Pos != (diag.Pos{File: "t.fung", Line: 4, Col: 1}) {
		t.Errorf("comment pos = %v", comments[1].Pos)
	}
}

func TestColumnsCountRunesNotBytes(t *testing.T) {
	toks, _, err := Lex("t.fung", "참치 x")
	if err != nil {
		t.Fatal(err)
	}
	if toks[1].Pos.Col != 4 {
		t.Fatalf("x at col %d, want 4", toks[1].Pos.Col)
	}
}

func TestUnexpectedCharacter(t *testing.T) {
	_, _, err := Lex("t.fung", "a @ b")
	if err == nil || err.Code != "E0101" {
		t.Fatalf("err = %v, want E0101", err)
	}
	if err.Pos != (diag.Pos{File: "t.fung", Line: 1, Col: 3}) {
		t.Fatalf("pos = %v", err.Pos)
	}
}

func TestEmptySourceIsJustEOF(t *testing.T) {
	toks, _, err := Lex("t.fung", "")
	if err != nil || len(toks) != 1 || toks[0].Kind != EOF {
		t.Fatalf("toks = %v, err = %v", toks, err)
	}
}

func TestNulByteIsRejectedNotTreatedAsEOF(t *testing.T) {
	_, _, err := Lex("t.fung", "x\x00y")
	if err == nil || err.Code != "E0101" || err.Pos.Col != 2 {
		t.Fatalf("err = %v, want E0101 at col 2", err)
	}
	toks, _, err := Lex("t.fung", "print(1)\n\x00print(2)")
	if err == nil {
		t.Fatalf("NUL silently truncated the source: %v", toks)
	}
}

func TestLeadingBOMIsSkipped(t *testing.T) {
	toks, _, err := Lex("t.fung", "\ufeffx")
	if err != nil {
		t.Fatal(err)
	}
	if toks[0].Kind != Name || toks[0].Text != "x" || toks[0].Pos.Col != 1 {
		t.Fatalf("toks = %v", toks)
	}
	// Only a leading BOM is special; elsewhere it is an unexpected character.
	if _, _, err := Lex("t.fung", "x\ufeff"); err == nil || err.Code != "E0101" {
		t.Fatalf("err = %v, want E0101 for a BOM in the middle", err)
	}
}

func TestLexAtStartsFromGivenPosition(t *testing.T) {
	toks, _, err := LexAt("t.fung", "x + 1", diag.Pos{File: "t.fung", Line: 3, Col: 10}, 0)
	if err != nil {
		t.Fatal(err)
	}
	if toks[0].Pos != (diag.Pos{File: "t.fung", Line: 3, Col: 10}) || toks[2].Pos.Col != 14 {
		t.Fatalf("toks = %v", toks)
	}
}

func TestTokensCarryByteSpans(t *testing.T) {
	src := "참치 = \"a b\" + 1 `raw` 'sym 1.5 2h"
	toks, _, err := Lex("t.fung", src)
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"참치", "=", "\"a b\"", "+", "1", "`raw`", "'sym", "1.5", "2h"}
	for i, w := range want {
		if got := src[toks[i].Off:toks[i].End]; got != w {
			t.Errorf("token %d span = %q, want %q", i, got, w)
		}
	}
	eof := toks[len(toks)-1]
	if eof.Off != len(src) || eof.End != len(src) {
		t.Errorf("EOF span = %d..%d", eof.Off, eof.End)
	}
}
