package value

import "testing"

func list(vs ...Value) *List { return NewList(vs) }

func TestCollectionFormat(t *testing.T) {
	m := NewMapBuilder()
	m.Set("name", Str("x"))
	m.Set("a b", Int(1))
	cases := map[Value]string{
		list(Int(1), Str("a"), Nil{}):          `[1, "a", nil]`,
		list():                                 "[]",
		&Tuple{Items: []Value{Int(1), Int(2)}}: "(1, 2)",
		&Tuple{Items: []Value{Int(1)}}:         "(1,)",
		&Tuple{}:                               "()",
		m.Map():                                `{name: "x", "a b": 1}`,
		NewMap():                               "{}",
		&Tag{Name: Intern("Circle"), Args: []Value{Int(3)}}:       "Circle(3)",
		&Tag{Name: Intern("Rect"), Args: []Value{Int(2), Int(5)}}: "Rect(2, 5)",
		Range{Start: 0, End: 10}:                                  "0..10",
		Range{Start: 0, End: 10, Inclusive: true}:                 "0..=10",
		list(list(Int(1)), &Tuple{Items: []Value{Str("s")}}):      `[[1], ("s",)]`,
	}
	for v, want := range cases {
		if got := Format(v); got != want {
			t.Errorf("Format = %q, want %q", got, want)
		}
	}
	if (&Tag{Name: Intern("Circle")}).TypeName() != "Circle" || list().TypeName() != "list" || NewMap().TypeName() != "map" || (&Tuple{}).TypeName() != "tuple" || (Range{}).TypeName() != "range" {
		t.Error("TypeName")
	}
}

func TestCollectionEquality(t *testing.T) {
	b1, b2 := NewMapBuilder(), NewMapBuilder()
	b1.Set("a", Int(1))
	b1.Set("b", Int(2))
	b2.Set("b", Int(2))
	b2.Set("a", Int(1))
	m1, m2 := b1.Map(), b2.Map()
	cases := []struct {
		a, b Value
		want bool
	}{
		{list(Int(1), Int(2)), list(Int(1), Float(2)), true},
		{list(Int(1)), list(Int(1), Int(2)), false},
		{list(Int(1)), &Tuple{Items: []Value{Int(1)}}, false},
		{m1, m2, true},
		{&Tag{Name: Intern("Ok"), Args: []Value{Int(1)}}, &Tag{Name: Intern("Ok"), Args: []Value{Int(1)}}, true},
		{&Tag{Name: Intern("Ok"), Args: []Value{Int(1)}}, &Tag{Name: Intern("Err"), Args: []Value{Int(1)}}, false},
		{Range{0, 3, false}, Range{0, 3, false}, true},
		{Range{0, 3, false}, Range{0, 3, true}, false},
		{list(), Nil{}, false},
	}
	for _, c := range cases {
		if got := Equal(c.a, c.b); got != c.want {
			t.Errorf("Equal(%s, %s) = %v", Format(c.a), Format(c.b), got)
		}
	}
}

func TestCollectionOrdering(t *testing.T) {
	if c, err := Compare(list(Int(1), Int(2)), list(Int(1), Int(3))); err != nil || c != -1 {
		t.Errorf("lexicographic: %d %v", c, err)
	}
	if c, _ := Compare(list(Int(1)), list(Int(1), Int(0))); c != -1 {
		t.Error("shorter is smaller")
	}
	if c, _ := Compare(&Tuple{Items: []Value{Str("b")}}, &Tuple{Items: []Value{Str("a")}}); c != 1 {
		t.Error("tuples")
	}
	for _, pair := range [][2]Value{{list(), &Tuple{}}, {NewMap(), NewMap()}, {&Tag{Name: Intern("A")}, &Tag{Name: Intern("A")}}, {list(Int(1)), list(Str("a"))}} {
		if _, err := Compare(pair[0], pair[1]); err == nil || err.Code != "E0501" {
			t.Errorf("Compare(%s, %s) should fail", Format(pair[0]), Format(pair[1]))
		}
	}
}

func TestListConcatAndMapPut(t *testing.T) {
	v, err := Add(list(Int(1)), list(Int(2)))
	if err != nil || Format(v) != "[1, 2]" {
		t.Fatalf("concat: %v %v", v, err)
	}
	if _, err := Add(list(), Int(1)); err == nil || err.Code != "E0501" {
		t.Error("list + int")
	}
	mB := NewMapBuilder()
	mB.Set("a", Int(1))
	m := mB.Map()
	m2 := m.Put("b", Int(2))
	if m.Len() != 1 || m2.Len() != 2 || Format(m2) != "{a: 1, b: 2}" {
		t.Fatalf("Put must copy: %s %s", Format(m), Format(m2))
	}
	m3 := m2.Put("a", Int(9))
	if Format(m3) != "{a: 9, b: 2}" {
		t.Fatalf("Put keeps order: %s", Format(m3))
	}
}
