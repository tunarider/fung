package value

import "testing"

func ints(xs ...int) []Value {
	out := make([]Value, len(xs))
	for i, x := range xs {
		out[i] = Int(x)
	}
	return out
}

func TestListAPI(t *testing.T) {
	a := NewList(ints(1, 2, 3))
	if a.Len() != 3 || a.At(0) != Int(1) || a.At(2) != Int(3) {
		t.Fatal("Len/At")
	}
	b := a.Append(Int(4))
	if a.Len() != 3 || b.Len() != 4 || b.At(3) != Int(4) {
		t.Fatal("Append must not change the original")
	}
	s := b.Slice(1, 3)
	if Format(s) != "[2, 3]" || Format(b) != "[1, 2, 3, 4]" {
		t.Fatalf("Slice: %s %s", Format(s), Format(b))
	}
	if Format(a.Concat(NewList(ints(9)))) != "[1, 2, 3, 9]" {
		t.Fatal("Concat")
	}
	got := []Value{}
	for i, v := range b.All() {
		if Int(i+1) != v {
			t.Fatalf("All index %d value %v", i, v)
		}
		got = append(got, v)
	}
	if len(got) != 4 {
		t.Fatal("All length")
	}
	raw := b.ToSlice()
	raw[0] = Int(99)
	if b.At(0) != Int(1) {
		t.Fatal("ToSlice must copy")
	}
	var lb ListBuilder
	lb.Add(Int(7))
	lb.Add(Int(8))
	if Format(lb.List()) != "[7, 8]" {
		t.Fatal("ListBuilder")
	}
	if NewList(nil).Len() != 0 || Format(NewList(nil)) != "[]" {
		t.Fatal("empty list")
	}
}

func TestMapAPI(t *testing.T) {
	mb := NewMapBuilder()
	mb.Set("b", Int(1))
	mb.Set("a", Int(2))
	mb.Set("b", Int(3)) // replace keeps position
	m := mb.Map()
	if Format(m) != "{b: 3, a: 2}" || m.Len() != 2 {
		t.Fatalf("builder: %s", Format(m))
	}
	m2 := m.Put("c", Int(4)).Put("b", Int(5))
	if Format(m2) != "{b: 5, a: 2, c: 4}" || Format(m) != "{b: 3, a: 2}" {
		t.Fatalf("Put: %s / %s", Format(m2), Format(m))
	}
	if Format(m2.Remove("a")) != "{b: 5, c: 4}" || Format(m2.Remove("zz")) != Format(m2) {
		t.Fatal("Remove")
	}
	o := NewMapBuilder()
	o.Set("a", Int(0))
	o.Set("d", Int(1))
	if Format(m.Merge(o.Map())) != "{b: 3, a: 0, d: 1}" {
		t.Fatal("Merge")
	}
	keys := m2.Keys()
	keys[0] = "zz"
	if m2.Keys()[0] != "b" {
		t.Fatal("Keys must copy")
	}
	n := 0
	for k, v := range m2.All() {
		if got, _ := m2.Get(k); got != v {
			t.Fatal("All")
		}
		n++
	}
	if n != 3 || NewMap().Len() != 0 {
		t.Fatal("All count / empty")
	}
}
