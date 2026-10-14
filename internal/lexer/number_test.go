package lexer

import (
	"math"
	"testing"
	"time"
)

func one(t *testing.T, src string) Token {
	t.Helper()
	toks, _, err := Lex("t.fung", src)
	if err != nil {
		t.Fatalf("Lex(%q): %v", src, err)
	}
	if len(toks) != 2 {
		t.Fatalf("Lex(%q) = %v, want one token + EOF", src, toks)
	}
	return toks[0]
}

func TestIntegerLiterals(t *testing.T) {
	cases := map[string]int64{
		"0": 0, "42": 42, "1_000_000": 1000000, "0xff": 255, "0XFF": 255,
		"0o17": 15, "0b1010": 10, "0b1_0": 2, "9223372036854775807": 9223372036854775807,
	}
	for src, want := range cases {
		tok := one(t, src)
		if tok.Kind != Int || tok.Int != want {
			t.Errorf("%q = %v %d, want Int %d", src, tok.Kind, tok.Int, want)
		}
	}
}

func TestFloatLiterals(t *testing.T) {
	cases := map[string]float64{
		"3.14": 3.14, "1e9": 1e9, "2.5e-3": 2.5e-3, "1E+2": 100, "1_0.5": 10.5, "0.1": 0.1,
	}
	for src, want := range cases {
		tok := one(t, src)
		if tok.Kind != Float || tok.Float != want {
			t.Errorf("%q = %v %g, want Float %g", src, tok.Kind, tok.Float, want)
		}
	}
}

func TestDurationLiterals(t *testing.T) {
	cases := map[string]time.Duration{
		"500ms": 500 * time.Millisecond, "30s": 30 * time.Second, "1.5s": 1500 * time.Millisecond,
		"1h30m": 90 * time.Minute, "2h": 2 * time.Hour, "1m30s500ms": time.Minute + 30*time.Second + 500*time.Millisecond,
	}
	for src, want := range cases {
		tok := one(t, src)
		if tok.Kind != Duration || tok.Dur != want {
			t.Errorf("%q = %v %v, want Duration %v", src, tok.Kind, tok.Dur, want)
		}
	}
}

func TestNumberFollowedByOperatorOrRange(t *testing.T) {
	if got := kinds(t, "1..3"); !equalKinds(got, []Kind{Int, DotDot, Int}) {
		t.Errorf("1..3 = %v", got)
	}
	if got := kinds(t, "1.5.3"); !equalKinds(got, []Kind{Float, Dot, Int}) {
		t.Errorf("1.5.3 = %v", got)
	}
	if got := kinds(t, "30 s"); !equalKinds(got, []Kind{Int, Name}) {
		t.Errorf("30 s = %v", got)
	}
	if got := kinds(t, "1+2"); !equalKinds(got, []Kind{Int, Plus, Int}) {
		t.Errorf("1+2 = %v", got)
	}
}

func TestMalformedNumbers(t *testing.T) {
	cases := map[string]string{
		"30abc":               "E0102",
		"1__0":                "E0102",
		"1_":                  "E0102",
		"0x":                  "E0102",
		"0xg":                 "E0102",
		"9223372036854775809": "E0102", // int64 max + 2 (max + 1 is the min-int magnitude token)
		"1e":                  "E0102",
		"1h30":                "E0105", // trailing component without unit
		"30m1h":               "E0105", // units must descend
		"5x":                  "E0102",
		"1e5s":                "E0102", // no exponent inside a duration component
	}
	for src, want := range cases {
		_, _, err := Lex("t.fung", src)
		if err == nil || err.Code != want {
			t.Errorf("%q: err = %v, want %s", src, err, want)
		}
	}
}

func TestDurationRoundsToNearestNanosecond(t *testing.T) {
	// 4.35h = 15660s exactly; naive float multiplication lands one ns short.
	if tok := one(t, "4.35h"); tok.Dur != 4*time.Hour+21*time.Minute {
		t.Fatalf("4.35h = %v", tok.Dur)
	}
}

func TestDurationOverflowIsAnError(t *testing.T) {
	for _, src := range []string{"9999999999h", "2562048h", "9223372036854775807ms", "2562047h47m17s"} {
		_, _, err := Lex("t.fung", src)
		if err == nil || err.Code != "E0105" {
			t.Errorf("%q: err = %v, want E0105", src, err)
		}
	}
	if tok := one(t, "2562047h"); tok.Dur != 2562047*time.Hour {
		t.Fatalf("2562047h = %v", tok.Dur)
	}
}

func TestMinIntMagnitudeIsATokenForTheParser(t *testing.T) {
	tok := one(t, "9223372036854775808")
	if tok.Kind != Int || tok.Int != math.MinInt64 {
		t.Fatalf("tok = %+v", tok)
	}
	if _, _, err := Lex("t.fung", "9223372036854775809"); err == nil || err.Code != "E0102" {
		t.Fatalf("one past: %v", err)
	}
}
