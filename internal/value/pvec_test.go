package value

import (
	"math/rand/v2"
	"testing"
)

// pvec against a plain slice: every version stays readable and unchanged.
func TestPvecMatchesSlice(t *testing.T) {
	r := rand.New(rand.NewPCG(1, 2))
	type version struct {
		v   *pvec[int]
		ref []int
	}
	cur := version{v: pvecOf[int](nil), ref: nil}
	history := []version{cur}
	for step := 0; step < 40000; step++ {
		switch op := r.IntN(10); {
		case op < 7 || len(cur.ref) == 0:
			x := r.Int()
			cur = version{cur.v.push(x), append(append([]int(nil), cur.ref...), x)}
		default:
			i := r.IntN(len(cur.ref))
			x := r.Int()
			ref := append([]int(nil), cur.ref...)
			ref[i] = x
			cur = version{cur.v.set(i, x), ref}
		}
		if step%997 == 0 {
			history = append(history, cur)
		}
		if step%4999 == 0 {
			// fork from an old version: the fork must not touch its parent
			old := history[r.IntN(len(history))]
			f := old.v.push(-1)
			if f.at(f.cnt-1) != -1 || old.v.cnt != len(old.ref) {
				t.Fatal("fork changed its parent")
			}
		}
	}
	for _, h := range append(history, cur) {
		if h.v.cnt != len(h.ref) {
			t.Fatalf("len %d want %d", h.v.cnt, len(h.ref))
		}
		for i, x := range h.ref {
			if h.v.at(i) != x {
				t.Fatalf("at(%d) = %d want %d (len %d)", i, h.v.at(i), x, len(h.ref))
			}
		}
	}
}

func TestPvecOfBoundaries(t *testing.T) {
	for _, n := range []int{0, 1, 31, 32, 33, 64, 1023, 1024, 1025, 1056, 32768, 32800} {
		items := make([]int, n)
		for i := range items {
			items[i] = i * 3
		}
		v := pvecOf(items)
		if v.cnt != n {
			t.Fatalf("n=%d cnt=%d", n, v.cnt)
		}
		for i := range n {
			if v.at(i) != i*3 {
				t.Fatalf("n=%d at(%d)=%d", n, i, v.at(i))
			}
		}
		w := v
		for k := range 1100 { // push across several tail flushes and a root growth
			w = w.push(-k)
		}
		for k := range 1100 {
			if w.at(n+k) != -k {
				t.Fatalf("n=%d push %d: %d", n, k, w.at(n+k))
			}
		}
		for i := range n {
			if w.at(i) != i*3 || v.at(i) != i*3 {
				t.Fatalf("n=%d prefix changed at %d", n, i)
			}
		}
		if n > 1 {
			s := v.set(0, 7).set(n-1, 8)
			if s.at(0) != 7 || s.at(n-1) != 8 || v.at(0) != 0 || v.at(n-1) != (n-1)*3 {
				t.Fatalf("n=%d set", n)
			}
		}
	}
}
