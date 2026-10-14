package prelude

import (
	"testing"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

func TestStringFunctions(t *testing.T) {
	cases := []struct {
		name string
		args []value.Value
		want string
	}{
		{"split", []value.Value{value.Str("a,b,,c"), value.Str(",")}, `["a", "b", "", "c"]`},
		{"split", []value.Value{value.Str("참치"), value.Str("")}, `["참", "치"]`},
		{"join", []value.Value{list(value.Str("a"), value.Int(1)), value.Str("-")}, `"a-1"`},
		{"trim", []value.Value{value.Str("  a \n"), value.Nil{}}, `"a"`},
		{"trim", []value.Value{value.Str("xxaxx"), value.Str("x")}, `"a"`},
		{"upper", []value.Value{value.Str("aé")}, `"AÉ"`},
		{"lower", []value.Value{value.Str("AÉ")}, `"aé"`},
		{"replace", []value.Value{value.Str("a-b-c"), value.Str("-"), value.Str("+")}, `"a+b+c"`},
		{"starts_with", []value.Value{value.Str("abc"), value.Str("ab")}, "true"},
		{"ends_with", []value.Value{value.Str("abc"), value.Str("ab")}, "false"},
		{"repeat", []value.Value{value.Str("ab"), value.Int(2)}, `"abab"`},
		{"repeat", []value.Value{value.Str("ab"), value.Int(-1)}, `""`},
	}
	for _, c := range cases {
		got, err := callNative(t, c.name, c.args...)
		if err != nil || value.Format(got) != c.want {
			t.Errorf("%s(%v) = %v %v, want %s", c.name, c.args, got, err, c.want)
		}
	}
	if _, err := callNative(t, "upper", value.Int(1)); err == nil || err.(*diag.Error).Code != "E0403" {
		t.Errorf("upper(1): %v", err)
	}
	if _, err := callNative(t, "join", value.Int(1), value.Str("")); err == nil || err.(*diag.Error).Code != "E0506" {
		t.Errorf("join(1): %v", err)
	}
}

func TestStringSizeLimits(t *testing.T) {
	if _, err := callNative(t, "repeat", value.Str("a"), value.Int(200_000_000)); err == nil || err.(*diag.Error).Code != "E0806" {
		t.Errorf("huge repeat: %v", err)
	}
	if _, err := callNative(t, "repeat", value.Str("ab"), value.Int(3)); err != nil {
		t.Errorf("small repeat: %v", err)
	}
}
