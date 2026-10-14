package value

import "testing"

func TestFormatLiterals(t *testing.T) {
	cases := map[Value]string{
		Int(0): "0", Int(-42): "-42", Int(9223372036854775807): "9223372036854775807",
		Float(5): "5.0", Float(2.5): "2.5", Float(-0.25): "-0.25",
		Float(1e21): "1e21", Float(1e-7): "1e-07", Float(123456789.0): "123456789.0", Float(1e6): "1000000.0",
		Bool(true): "true", Bool(false): "false", Nil{}: "nil",
		&Native{Name: "print"}: "<fn print>",
	}
	for v, want := range cases {
		if got := Format(v); got != want {
			t.Errorf("Format(%#v) = %q, want %q", v, got, want)
		}
	}
}

func TestFormatShortestRoundTrip(t *testing.T) {
	// Computed at runtime: Go would fold the constant 0.1 + 0.2 to exactly 0.3.
	a, b := 0.1, 0.2
	if got := Format(Float(a + b)); got != "0.30000000000000004" {
		t.Fatalf("Format(0.1 + 0.2) = %q", got)
	}
}

func TestTypeNames(t *testing.T) {
	cases := map[Value]string{Int(1): "int", Float(1): "float", Bool(true): "bool", Nil{}: "nil", &Native{}: "fn"}
	for v, want := range cases {
		if got := v.TypeName(); got != want {
			t.Errorf("%#v.TypeName() = %q, want %q", v, got, want)
		}
	}
}
