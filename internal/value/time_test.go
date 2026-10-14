package value

import (
	"testing"
	"time"
)

func mustTime(s string) Time {
	t, err := time.Parse(time.RFC3339Nano, s)
	if err != nil {
		panic(err)
	}
	return NewTime(t)
}

func TestTimeFormatISO(t *testing.T) {
	cases := map[string]string{
		"2026-10-14T09:00:00+09:00":     "2026-10-14T09:00:00+09:00",
		"2026-10-14T09:00:00.123+09:00": "2026-10-14T09:00:00.123+09:00",
		"2026-10-14T00:00:00Z":          "2026-10-14T00:00:00Z",
	}
	for in, want := range cases {
		if got := Format(mustTime(in)); got != want {
			t.Errorf("Format(%s) = %s, want %s", in, got, want)
		}
	}
	if mustTime("2026-10-14T00:00:00Z").TypeName() != "time" {
		t.Error("TypeName")
	}
}

func TestTimeEqualAndCompare(t *testing.T) {
	a := mustTime("2026-10-14T09:00:00+09:00")
	b := mustTime("2026-10-14T00:00:00Z") // same instant, other zone
	c := mustTime("2026-10-14T00:00:01Z")
	if !Equal(a, b) {
		t.Error("same instant in different zones must be ==")
	}
	if Equal(a, c) || Equal(a, Int(1)) {
		t.Error("different instant or type must not be ==")
	}
	if r, err := Compare(a, c); err != nil || r != -1 {
		t.Errorf("Compare(a, c) = %d, %v", r, err)
	}
	if r, err := Compare(c, a); err != nil || r != 1 {
		t.Errorf("Compare(c, a) = %d, %v", r, err)
	}
	if _, err := Compare(a, Dur(1)); err == nil || err.Code != "E0501" {
		t.Error("time < dur must be E0501")
	}
}

func TestTimeArithmetic(t *testing.T) {
	a := mustTime("2026-10-14T09:00:00+09:00")
	later, err := Add(a, Dur(90*time.Minute))
	if err != nil || Format(later) != "2026-10-14T10:30:00+09:00" {
		t.Errorf("time + dur = %v, %v", later, err)
	}
	later2, err := Add(Dur(time.Hour), a)
	if err != nil || Format(later2) != "2026-10-14T10:00:00+09:00" {
		t.Errorf("dur + time = %v, %v", later2, err)
	}
	earlier, err := Sub(a, Dur(time.Hour))
	if err != nil || Format(earlier) != "2026-10-14T08:00:00+09:00" {
		t.Errorf("time - dur = %v, %v", earlier, err)
	}
	d, err := Sub(later.(Time), a)
	if err != nil || d != Dur(90*time.Minute) {
		t.Errorf("time - time = %v, %v", d, err)
	}
	if _, err := Add(a, a); err == nil || err.Code != "E0501" {
		t.Error("time + time must be E0501")
	}
	if _, err := Mul(a, Int(2)); err == nil || err.Code != "E0501" {
		t.Error("time * int must be E0501")
	}
	if _, err := Sub(Dur(1), a); err == nil || err.Code != "E0501" {
		t.Error("dur - time must be E0501")
	}
	far := NewTime(time.Unix(0, 0).AddDate(400, 0, 0))
	if _, err := Sub(far, NewTime(time.Unix(0, 0))); err == nil || err.Code != "E0503" {
		t.Error("time - time beyond the dur range must be E0503")
	}
}
