package value

import "testing"

func TestSymbolsInternAndFormat(t *testing.T) {
	a, b := Intern("ok"), Intern("ok")
	if a != b || a.Name() != "ok" || Intern("err") == a {
		t.Fatalf("intern: %v %v", a, b)
	}
	if Format(a) != "'ok" || a.TypeName() != "sym" {
		t.Errorf("Format = %q", Format(a))
	}
	if !Equal(a, b) || Equal(a, Str("ok")) || Equal(a, Intern("err")) {
		t.Error("Equal")
	}
	if _, err := Compare(a, b); err == nil || err.Code != "E0501" {
		t.Error("symbols are not ordered")
	}
	fails(t, Add, a, b, "E0501")
}
