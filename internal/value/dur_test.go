package value

import (
	"testing"
	"time"
)

func TestDurFormat(t *testing.T) {
	cases := map[Dur]string{
		Dur(90 * time.Minute):                        "1h30m",
		Dur(1500 * time.Millisecond):                 "1s500ms",
		Dur(500 * time.Microsecond):                  "0.5ms",
		Dur(0):                                       "0s",
		Dur(-5 * time.Second):                        "-5s",
		Dur(2*time.Hour + 3*time.Second):             "2h3s",
		Dur(time.Millisecond + 500*time.Microsecond): "1.5ms",
	}
	for d, want := range cases {
		if got := Format(d); got != want {
			t.Errorf("Format(%v) = %q, want %q", time.Duration(d), got, want)
		}
	}
}

func TestDurArithmetic(t *testing.T) {
	s, m := Dur(time.Second), Dur(time.Minute)
	ok(t, Add, s, m, Dur(61*time.Second))
	ok(t, Sub, m, s, Dur(59*time.Second))
	ok(t, Mul, s, Int(3), Dur(3*time.Second))
	ok(t, Mul, Float(1.5), s, Dur(1500*time.Millisecond))
	ok(t, Div, m, Int(4), Dur(15*time.Second))
	ok(t, Div, s, Float(0.5), Dur(2*time.Second))
	fails(t, Add, s, Int(1), "E0501")
	fails(t, Div, m, s, "E0501")
	fails(t, Div, s, Int(0), "E0502")
	fails(t, FloorDiv, m, Int(2), "E0501")
	fails(t, Mod, m, Int(2), "E0501")
	if v, _ := Neg(s); v != Dur(-time.Second) {
		t.Error("Neg")
	}
	if c, err := Compare(s, m); err != nil || c != -1 {
		t.Error("Compare")
	}
	if !Equal(s, Dur(time.Second)) || Equal(s, Int(1000000000)) {
		t.Error("Equal")
	}
}

func TestDurOverflowIsAnError(t *testing.T) {
	big := Dur(1 << 62)
	fails(t, Add, big, big, "E0503")
	fails(t, Mul, big, Int(4), "E0503")
	fails(t, Mul, big, Float(4), "E0503")
}
