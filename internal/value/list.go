package value

import "iter"

// smallMax is the largest list kept as a plain slice; bigger lists live
// in a persistent vector viewed through [lo, hi) (M10). 32 and 64 measured
// the same at the fung level (Task 6); 32 is one trie node.
var smallMax = 32

// List is an ordered sequence (02). Immutable: every operation returns a
// new list. Small lists are plain slices; big ones are views on a shared
// persistent vector, so appending and taking the tail are cheap. The
// representation is private to this package.
type List struct {
	flat   []Value      // small (vec == nil)
	vec    *pvec[Value] // big
	lo, hi int
}

// NewList makes a list that owns items; the caller must not change the
// slice afterwards.
func NewList(items []Value) *List {
	if items == nil {
		items = []Value{}
	}
	if len(items) <= smallMax {
		return &List{flat: items}
	}
	v := pvecOf(items)
	return &List{vec: v, hi: v.cnt}
}

func (l *List) Len() int {
	if l.vec == nil {
		return len(l.flat)
	}
	return l.hi - l.lo
}

// At stays small enough to inline for the common small list; big lists
// take the out-of-line path.
func (l *List) At(i int) Value {
	if l.vec == nil {
		return l.flat[i]
	}
	return l.atBig(i)
}

//go:noinline
func (l *List) atBig(i int) Value { return l.vec.at(l.lo + i) }

// All iterates index and item in order, a leaf at a time for big lists.
func (l *List) All() iter.Seq2[int, Value] {
	return func(yield func(int, Value) bool) {
		if l.vec == nil {
			for i, v := range l.flat {
				if !yield(i, v) {
					return
				}
			}
			return
		}
		for i := l.lo; i < l.hi; {
			leaf := l.vec.leafFor(i)
			for j := i & pvMask; j < len(leaf) && i < l.hi; j, i = j+1, i+1 {
				if !yield(i-l.lo, leaf[j]) {
					return
				}
			}
		}
	}
}

// ToSlice copies the items into a new slice.
func (l *List) ToSlice() []Value { return l.appendTo(make([]Value, 0, l.Len())) }

// sliceCopy copies items [lo, hi) into a new slice.
func (l *List) sliceCopy(lo, hi int) []Value {
	if l.vec == nil {
		return append([]Value(nil), l.flat[lo:hi]...)
	}
	out := make([]Value, 0, hi-lo)
	for i := lo; i < hi; i++ {
		out = append(out, l.At(i))
	}
	return out
}

// Slice is items [lo, hi) (bounds are checked by callers). A big result
// is a view on the same vector; a small one, or one much smaller than
// the vector it would keep alive, is copied out.
func (l *List) Slice(lo, hi int) *List {
	n := hi - lo
	if l.vec == nil || n <= smallMax || n*4 < l.vec.cnt {
		return NewList(l.sliceCopy(lo, hi))
	}
	return &List{vec: l.vec, lo: l.lo + lo, hi: l.lo + hi}
}

func (l *List) Append(x Value) *List {
	switch {
	case l.vec == nil && len(l.flat) < smallMax:
		out := make([]Value, len(l.flat)+1)
		copy(out, l.flat)
		out[len(l.flat)] = x
		return &List{flat: out}
	case l.vec == nil:
		return NewList(append(append(make([]Value, 0, len(l.flat)+1), l.flat...), x))
	case l.hi == l.vec.cnt: // the view reaches the vector's end: grow it
		return &List{vec: l.vec.push(x), lo: l.lo, hi: l.hi + 1}
	default: // a front slice gets its own vector, so the original is untouched
		return NewList(append(l.ToSlice(), x))
	}
}

// Concat is l followed by o. Its cost follows o's length when l is big,
// both lengths otherwise.
func (l *List) Concat(o *List) *List {
	switch {
	case o.Len() == 0:
		return l
	case l.Len() == 0:
		return o
	case l.vec == nil || l.Len()+o.Len() <= smallMax:
		// One exact-size copy of both (acc + [x] on a small list).
		out := make([]Value, 0, l.Len()+o.Len())
		out = l.appendTo(out)
		return NewList(o.appendTo(out))
	}
	out := l
	for _, v := range o.All() {
		out = out.Append(v)
	}
	return out
}

// appendTo appends the list's items to dst without an intermediate copy.
func (l *List) appendTo(dst []Value) []Value {
	if l.vec == nil {
		return append(dst, l.flat...)
	}
	for _, v := range l.All() {
		dst = append(dst, v)
	}
	return dst
}

// ListBuilder collects items in place before the list is shared.
type ListBuilder struct{ items []Value }

func (b *ListBuilder) Add(v Value) { b.items = append(b.items, v) }

// List freezes the builder; it must not be used afterwards.
func (b *ListBuilder) List() *List {
	l := NewList(b.items)
	b.items = nil
	return l
}
