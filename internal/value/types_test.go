package value

import "testing"

func TestMatchesType(t *testing.T) {
	cases := []struct {
		v    Value
		alts []string
		want bool
	}{
		{Int(1), []string{"int"}, true}, {Int(1), []string{"num"}, true}, {Float(1), []string{"num"}, true},
		{Str("a"), []string{"num"}, false}, {Nil{}, []string{"any"}, true}, {Nil{}, []string{"str", "nil"}, true},
		{Bool(true), []string{"str", "nil"}, false}, {&Native{Name: "p"}, []string{"fn"}, true},
		{Int(1), []string{"Circle"}, false}, {Int(1), nil, true},
	}
	for _, c := range cases {
		if got := MatchesType(c.v, c.alts); got != c.want {
			t.Errorf("MatchesType(%v, %v) = %v", c.v, c.alts, got)
		}
	}
	if TypeText([]string{"int", "nil"}) != "int | nil" {
		t.Error("TypeText")
	}
}

func TestFnFormatAndEquality(t *testing.T) {
	a, b := &Native{Name: "print"}, &Native{Name: "print"}
	if Format(a) != "<fn print>" || Format(&Native{}) != "<fn>" {
		t.Error("Format")
	}
	if !Equal(a, a) || Equal(a, b) {
		t.Error("identity")
	}
}
