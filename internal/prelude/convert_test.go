package prelude

import (
	"math"
	"testing"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

func TestConversions(t *testing.T) {
	cases := []struct {
		name string
		arg  value.Value
		want string
	}{
		{"int", value.Int(3), "3"}, {"int", value.Float(3.9), "3"}, {"int", value.Float(-3.9), "-3"},
		{"int", value.Str("42"), "42"}, {"int", value.Str("-1_000"), "-1000"},
		{"float", value.Int(2), "2.0"}, {"float", value.Str("2.5"), "2.5"}, {"float", value.Str("1e3"), "1000.0"},
		{"str", value.Str("a"), `"a"`}, {"str", value.Int(1), `"1"`}, {"str", value.Float(1), `"1.0"`},
		{"str", value.Intern("ok"), `"'ok"`}, {"str", list(value.Int(1)), `"[1]"`},
		{"sym", value.Str("ok"), "'ok"}, {"sym", value.Intern("x"), "'x"},
		{"parse_int", value.Str("12"), "Ok(12)"}, {"parse_int", value.Str("x"), `Err({kind: 'parse, message: "not an integer: x"})`},
		{"parse_float", value.Str("1.5"), "Ok(1.5)"}, {"parse_float", value.Str("x"), `Err({kind: 'parse, message: "not a number: x"})`},
		{"parse_dur", value.Str("1h30m"), "Ok(1h30m)"}, {"parse_dur", value.Str(" 500ms "), "Ok(500ms)"}, {"parse_dur", value.Str("-5s"), "Ok(-5s)"},
		{"parse_dur", value.Str("30"), `Err({kind: 'parse, message: "not a duration: 30"})`}, {"parse_dur", value.Str("1h x"), `Err({kind: 'parse, message: "not a duration: 1h x"})`},
		{"parse_dur", value.Str("30m1h"), `Err({kind: 'parse, message: "not a duration: 30m1h"})`}, {"parse_dur", value.Str(""), `Err({kind: 'parse, message: "not a duration: "})`},
	}
	for _, c := range cases {
		got, err := callNative(t, c.name, c.arg)
		if err != nil || value.Format(got) != c.want {
			t.Errorf("%s(%s) = %v %v, want %s", c.name, value.Format(c.arg), got, err, c.want)
		}
	}
	errs := []struct {
		name string
		arg  value.Value
	}{
		{"int", value.Str("x")}, {"int", value.Str(" 1")}, {"int", value.Bool(true)}, {"int", value.Float(math.Inf(1))},
		{"float", value.Str("abc")}, {"sym", value.Int(1)},
	}
	for _, c := range errs {
		if _, err := callNative(t, c.name, c.arg); err == nil || err.(*diag.Error).Code != "E0513" {
			t.Errorf("%s(%s): %v, want E0513", c.name, value.Format(c.arg), err)
		}
	}
	if _, err := callNative(t, "parse_int", value.Int(1)); err == nil || err.(*diag.Error).Code != "E0403" {
		t.Errorf("parse_int(1): %v", err)
	}
}

func TestNumericSeparatorsAreStrict(t *testing.T) {
	for _, bad := range []string{"1_", "_1", "1__2", "1_.5", "1._5"} {
		if _, err := callNative(t, "int", value.Str(bad)); err == nil {
			if _, ferr := callNative(t, "float", value.Str(bad)); ferr == nil {
				t.Errorf("%q accepted by both int and float", bad)
			}
		}
		if _, err := callNative(t, "float", value.Str(bad)); err == nil {
			t.Errorf("float(%q) accepted", bad)
		}
	}
	if v, err := callNative(t, "int", value.Str("1_000")); err != nil || v != value.Int(1000) {
		t.Errorf("int(\"1_000\") = %v %v", v, err)
	}
	if v, err := callNative(t, "float", value.Str("1_0.5")); err != nil || v != value.Float(10.5) {
		t.Errorf("float(\"1_0.5\") = %v %v", v, err)
	}
}

func TestSymRequiresAName(t *testing.T) {
	for _, bad := range []string{"", "a b", "Ab", "1a", "a-b"} {
		if _, err := callNative(t, "sym", value.Str(bad)); err == nil || err.(*diag.Error).Code != "E0513" {
			t.Errorf("sym(%q): %v, want E0513", bad, err)
		}
	}
	for _, good := range []string{"ok", "snake_case", "참치", "_x", "a1"} {
		if _, err := callNative(t, "sym", value.Str(good)); err != nil {
			t.Errorf("sym(%q): %v", good, err)
		}
	}
}
