package value

import "testing"

type indexCase struct {
	v, i Value
	want string
	code string
}

func TestLen(t *testing.T) {
	cases := map[Value]int{Str("참치"): 2, list(Int(1), Int(2)): 2, &Tuple{}: 0, Range{Start: 0, End: 10}: 10, Range{0, 10, true}: 11, Range{5, 2, false}: 0}
	for v, want := range cases {
		if got, err := Len(v); err != nil || got != want {
			t.Errorf("Len(%s) = %d %v, want %d", Format(v), got, err, want)
		}
	}
	m := NewMapBuilder()
	m.Set("k", Nil{})
	if got, _ := Len(m.Map()); got != 1 {
		t.Error("map len")
	}
	if _, err := Len(Int(1)); err == nil || err.Code != "E0506" {
		t.Error("len of int")
	}
}

func TestIndex(t *testing.T) {
	xs := list(Int(10), Int(20), Int(30))
	mB := NewMapBuilder()
	mB.Set("name", Str("x"))
	m := mB.Map()
	cases := []indexCase{
		{xs, Int(0), "10", ""}, {xs, Int(-1), "30", ""}, {xs, Int(3), "", "E0507"}, {xs, Int(-4), "", "E0507"},
		{Str("참치"), Int(1), `"치"`, ""}, {Str("ab"), Int(2), "", "E0507"},
		{xs, Str("a"), "", "E0501"}, {&Tuple{Items: xs.ToSlice()}, Int(0), "", "E0509"}, {Int(1), Int(0), "", "E0509"},
		{m, Str("name"), `"x"`, ""}, {m, Str("age"), "", "E0508"}, {m, Int(0), "", "E0501"},
	}
	for _, c := range cases {
		got, err := Index(c.v, c.i)
		if c.code != "" {
			if err == nil || err.Code != c.code {
				t.Errorf("Index(%s, %s) err = %v, want %s", Format(c.v), Format(c.i), err, c.code)
			}
			continue
		}
		if err != nil || Format(got) != c.want {
			t.Errorf("Index(%s, %s) = %v %v, want %s", Format(c.v), Format(c.i), got, err, c.want)
		}
	}
}

func TestSlice(t *testing.T) {
	xs := list(Int(1), Int(2), Int(3))
	cases := []struct {
		start, end Value
		want       string
		code       string
	}{
		{Int(1), Int(3), "[2, 3]", ""}, {Nil{}, Int(2), "[1, 2]", ""}, {Int(-2), Nil{}, "[2, 3]", ""},
		{Int(0), Int(0), "[]", ""}, {Int(3), Nil{}, "[]", ""}, {Int(4), Nil{}, "", "E0507"}, {Int(0), Int(4), "", "E0507"},
		{Int(2), Int(1), "", "E0507"}, {Nil{}, Nil{}, "[1, 2, 3]", ""},
	}
	for _, c := range cases {
		got, err := Slice(xs, c.start, c.end, false)
		if c.code != "" {
			if err == nil || err.Code != c.code {
				t.Errorf("Slice(%s, %s) err = %v, want %s", Format(c.start), Format(c.end), err, c.code)
			}
			continue
		}
		if err != nil || Format(got) != c.want {
			t.Errorf("Slice(%s, %s) = %v %v, want %s", Format(c.start), Format(c.end), got, err, c.want)
		}
	}
	if got, _ := Slice(Str("참치회"), Int(1), Nil{}, false); Format(got) != `"치회"` {
		t.Errorf("string slice by rune: %v", got)
	}
	if _, err := Slice(&Tuple{}, Nil{}, Nil{}, false); err == nil || err.Code != "E0511" {
		t.Error("tuple slice")
	}
}

func TestContains(t *testing.T) {
	mB := NewMapBuilder()
	mB.Set("k", Int(1))
	m := mB.Map()
	cases := []struct {
		c, x Value
		want bool
	}{
		{list(Int(1), Int(2)), Int(2), true}, {list(Int(1)), Float(1), true}, {list(), Int(1), false},
		{Str("hello"), Str("ell"), true}, {Str("hello"), Str("z"), false},
		{m, Str("k"), true}, {m, Str("z"), false},
		{Range{0, 10, false}, Int(9), true}, {Range{0, 10, false}, Int(10), false}, {Range{0, 10, true}, Int(10), true}, {Range{0, 10, false}, Float(1), false},
	}
	for _, c := range cases {
		if got, err := Contains(c.c, c.x); err != nil || got != c.want {
			t.Errorf("Contains(%s, %s) = %v %v", Format(c.c), Format(c.x), got, err)
		}
	}
	if _, err := Contains(Str("a"), Int(1)); err == nil || err.Code != "E0501" {
		t.Error("str contains int")
	}
	if _, err := Contains(Int(1), Int(1)); err == nil || err.Code != "E0512" {
		t.Error("int container")
	}
}

func TestSliceInclusive(t *testing.T) {
	xs := list(Int(1), Int(2), Int(3))
	cases := []struct {
		start, end Value
		want       string
		code       string
	}{
		{Int(1), Int(2), "[2, 3]", ""}, {Int(0), Int(-1), "[1, 2, 3]", ""}, {Nil{}, Int(0), "[1]", ""},
		{Int(0), Int(3), "", "E0507"}, {Int(2), Int(0), "", "E0507"},
	}
	for _, c := range cases {
		got, err := Slice(xs, c.start, c.end, true)
		if c.code != "" {
			if err == nil || err.Code != c.code {
				t.Errorf("Slice(%s..=%s) err = %v, want %s", Format(c.start), Format(c.end), err, c.code)
			}
			continue
		}
		if err != nil || Format(got) != c.want {
			t.Errorf("Slice(%s..=%s) = %v %v, want %s", Format(c.start), Format(c.end), got, err, c.want)
		}
	}
}
