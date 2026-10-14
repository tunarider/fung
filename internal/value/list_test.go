package value

import (
	"math/rand/v2"
	"testing"
	"time"
)

func sameList(t *testing.T, what string, l *List, ref []Value) {
	t.Helper()
	if l.Len() != len(ref) {
		t.Fatalf("%s: len %d want %d", what, l.Len(), len(ref))
	}
	for i, v := range ref {
		if l.At(i) != v {
			t.Fatalf("%s: At(%d) = %v want %v", what, i, l.At(i), v)
		}
	}
	n := 0
	for i, v := range l.All() {
		if i != n || v != ref[i] {
			t.Fatalf("%s: All gave (%d, %v) at step %d", what, i, v, n)
		}
		n++
	}
	if n != len(ref) {
		t.Fatalf("%s: All stopped after %d", what, n)
	}
}

// Random lists against plain slices; old versions are re-read to check
// that no operation changes a list it was derived from.
func TestListMatchesSlice(t *testing.T) {
	r := rand.New(rand.NewPCG(5, 6))
	type version struct {
		l   *List
		ref []Value
	}
	cur := version{NewList(nil), nil}
	var history []version
	for step := 0; step < 30000; step++ {
		n := len(cur.ref)
		switch op := r.IntN(20); {
		case op < 11 || n < 2: // append, most of the time
			x := Int(r.IntN(1000))
			cur = version{cur.l.Append(x), append(append([]Value(nil), cur.ref...), x)}
		case op < 14: // drop from the front (rest / slicing a tail)
			k := 1 + r.IntN(min(n, 40))
			cur = version{cur.l.Slice(k, n), append([]Value(nil), cur.ref[k:]...)}
		case op < 16: // take a front part
			k := r.IntN(n)
			cur = version{cur.l.Slice(0, k), append([]Value(nil), cur.ref[:k]...)}
		case op < 18: // concat a small or big list made from cur
			k := r.IntN(n)
			o := cur.l.Slice(k, n)
			cur = version{cur.l.Concat(o), append(append([]Value(nil), cur.ref...), cur.ref[k:]...)}
		default: // jump back to an old version
			if len(history) > 0 {
				cur = history[r.IntN(len(history))]
			}
		}
		if len(cur.ref) > 6000 { // keep sizes moving across the small/big boundary
			cur = version{cur.l.Slice(len(cur.ref)-30, len(cur.ref)), append([]Value(nil), cur.ref[len(cur.ref)-30:]...)}
		}
		if step%301 == 0 {
			history = append(history, cur)
		}
		if step%2003 == 0 {
			for i, h := range history {
				sameList(t, "history", h.l, h.ref)
				_ = i
			}
		}
	}
	sameList(t, "final", cur.l, cur.ref)
}

func TestListViewAppendKeepsOriginal(t *testing.T) {
	xs := NewList(nil)
	for i := range 100 {
		xs = xs.Append(Int(i))
	}
	ys := xs.Slice(0, 40)
	zs := ys.Append(Int(-1))
	if xs.At(40) != Int(40) || zs.At(40) != Int(-1) || zs.Len() != 41 {
		t.Fatal("appending to a front slice changed the original")
	}
	a, b := xs.Append(Int(1)), xs.Append(Int(2))
	if a.At(100) != Int(1) || b.At(100) != Int(2) || xs.Len() != 100 {
		t.Fatal("forked appends interfere")
	}
	tail := xs.Slice(60, 100)
	tz := tail.Append(Int(-2))
	if xs.Len() != 100 || tz.At(40) != Int(-2) || tz.At(0) != Int(60) {
		t.Fatal("appending to a tail view")
	}
}

// Appending and taking the tail one at a time must be linear overall:
// 100,000 steps of each are milliseconds with shared structure and tens
// of seconds with full copies.
func TestListRestIsCheap(t *testing.T) {
	start := time.Now()
	defer func() {
		if took := time.Since(start); took > 2*time.Second {
			t.Fatalf("100,000 appends and rests took %s", took)
		}
	}()
	xs := NewList(nil)
	for i := range 100000 {
		xs = xs.Append(Int(i))
	}
	for i := 0; xs.Len() > 0; i++ {
		if xs.At(0) != Int(i) {
			t.Fatalf("rest %d: %v", i, xs.At(0))
		}
		xs = xs.Slice(1, xs.Len())
	}
}

func TestListBoundaries(t *testing.T) {
	for _, n := range []int{0, 1, 31, 32, 33, 64, 65, 1056} {
		items := make([]Value, n)
		for i := range items {
			items[i] = Int(i)
		}
		l := NewList(append([]Value(nil), items...))
		sameList(t, "NewList", l, items)
		sameList(t, "Append", l.Append(Int(-1)), append(append([]Value(nil), items...), Int(-1)))
		if n > 0 {
			sameList(t, "Slice", l.Slice(1, n), items[1:])
		}
		if Format(l) != Format(NewList(items)) {
			t.Fatal("Format")
		}
	}
}
