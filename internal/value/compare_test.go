package value

import "testing"

func TestEqual(t *testing.T) {
	cases := []struct {
		a, b Value
		want bool
	}{
		{Int(1), Int(1), true}, {Int(1), Float(1), true}, {Float(1.5), Int(1), false},
		{Bool(true), Bool(true), true}, {Nil{}, Nil{}, true},
		{Int(1), Bool(true), false}, {Nil{}, Int(0), false}, {Int(0), Bool(false), false},
	}
	for _, c := range cases {
		if got := Equal(c.a, c.b); got != c.want {
			t.Errorf("Equal(%v, %v) = %v, want %v", c.a, c.b, got, c.want)
		}
	}
	n := &Native{Name: "print"}
	if !Equal(n, n) || Equal(n, &Native{Name: "print"}) {
		t.Error("natives compare by identity")
	}
}

func TestCompare(t *testing.T) {
	cases := []struct {
		a, b Value
		want int
	}{
		{Int(1), Int(2), -1}, {Int(2), Int(2), 0}, {Float(2.5), Int(2), 1}, {Int(1), Float(1), 0},
	}
	for _, c := range cases {
		got, err := Compare(c.a, c.b)
		if err != nil || got != c.want {
			t.Errorf("Compare(%v, %v) = %d, %v; want %d", c.a, c.b, got, err, c.want)
		}
	}
	for _, pair := range [][2]Value{{Bool(true), Bool(false)}, {Int(1), Bool(true)}, {Nil{}, Nil{}}} {
		if _, err := Compare(pair[0], pair[1]); err == nil || err.Code != "E0501" {
			t.Errorf("Compare(%v, %v) err = %v, want E0501", pair[0], pair[1], err)
		}
	}
}

func TestIntFloatComparisonIsExact(t *testing.T) {
	// 2^53 + 1 is not representable as a float64; float64 conversion would
	// make these equal (02: 정확 비교).
	i := Int(9007199254740993)
	f := Float(9007199254740992)
	if Equal(i, f) || Equal(f, i) {
		t.Error("Equal lost precision")
	}
	if c, _ := Compare(i, f); c != 1 {
		t.Errorf("Compare(i, f) = %d, want 1", c)
	}
	if c, _ := Compare(f, i); c != -1 {
		t.Errorf("Compare(f, i) = %d, want -1", c)
	}
	if c, _ := Compare(Int(-9223372036854775808), Float(-9.3e18)); c != 1 {
		t.Error("below int range")
	}
	if c, _ := Compare(Int(1), Float(1e19)); c != -1 {
		t.Error("above int range")
	}
	if !Equal(Int(3), Float(3)) || Equal(Int(3), Float(3.5)) {
		t.Error("small values")
	}
	if c, _ := Compare(Int(3), Float(3.5)); c != -1 {
		t.Error("fraction")
	}
}
