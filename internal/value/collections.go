package value

import (
	"iter"
	"strings"
	"unicode"
)

// Tuple is a fixed-size, position-meaningful group (02).
type Tuple struct{ Items []Value }

// smallMapMax is the largest map kept in the flat representation; bigger
// maps are an ordered vector plus a HAMT (M10). Measured at the fung level
// (Task 6), 16 and 32 read alike while 16 builds 48-key maps ~30% faster:
// a flat put clones the whole Go map.
var smallMapMax = 16

// Map keeps insertion order (02). It is immutable: Put, Remove and Merge
// return new maps, MapBuilder makes one. Small maps are a key slice plus
// a Go map (copied on change); big maps share structure (omap).
type Map struct {
	keys []string
	vals map[string]Value
	big  *omap
}

func NewMap() *Map { return &Map{vals: map[string]Value{}} }

func (m *Map) Get(k string) (Value, bool) {
	if m.big != nil {
		return m.big.get(k)
	}
	v, ok := m.vals[k]
	return v, ok
}

func (m *Map) Len() int {
	if m.big != nil {
		return m.big.live
	}
	return len(m.keys)
}

// Keys copies the keys in insertion order.
func (m *Map) Keys() []string {
	if m.big != nil {
		keys, _ := m.big.liveFlat()
		return keys
	}
	return append([]string(nil), m.keys...)
}

// All iterates keys and values in insertion order.
func (m *Map) All() iter.Seq2[string, Value] {
	return func(yield func(string, Value) bool) {
		if m.big != nil {
			m.big.each(yield)
			return
		}
		for _, k := range m.keys {
			if !yield(k, m.vals[k]) {
				return
			}
		}
	}
}

// set adds or replaces a key in place on a flat map nobody sees yet.
func (m *Map) set(k string, v Value) {
	if _, ok := m.vals[k]; !ok {
		m.keys = append(m.keys, k)
	}
	m.vals[k] = v
}

func (m *Map) clone() *Map {
	out := &Map{keys: append([]string(nil), m.keys...), vals: make(map[string]Value, len(m.vals)+1)}
	for k, v := range m.vals {
		out.vals[k] = v
	}
	return out
}

// fromFlat picks the representation for a flat map that may have grown.
func fromFlat(m *Map) *Map {
	if len(m.keys) <= smallMapMax {
		return m
	}
	return &Map{big: omapOf(m.keys, func(k string) Value { return m.vals[k] })}
}

// Put returns a map with k set; an existing key keeps its place (06 put).
func (m *Map) Put(k string, v Value) *Map {
	if m.big != nil {
		return &Map{big: m.big.put(k, v)}
	}
	out := m.clone()
	out.set(k, v)
	return fromFlat(out)
}

// Remove returns a map without k (the same map when k is absent).
func (m *Map) Remove(k string) *Map {
	if _, ok := m.Get(k); !ok {
		return m
	}
	if m.big != nil {
		big := m.big.remove(k)
		if big.live > smallMapMax {
			return &Map{big: big}
		}
		keys, vals := big.liveFlat()
		return &Map{keys: keys, vals: vals}
	}
	out := NewMap()
	for _, key := range m.keys {
		if key != k {
			out.set(key, m.vals[key])
		}
	}
	return out
}

// Merge returns m with o's entries put in o's order; o wins on a clash.
func (m *Map) Merge(o *Map) *Map {
	out := m
	for k, v := range o.All() {
		out = out.Put(k, v)
	}
	return out
}

// MapBuilder makes a map by setting keys in place before it is shared.
type MapBuilder struct{ m *Map }

func NewMapBuilder() *MapBuilder            { return &MapBuilder{m: NewMap()} }
func (b *MapBuilder) Set(k string, v Value) { b.m.set(k, v) }
func (b *MapBuilder) Len() int              { return b.m.Len() }

// Map freezes the builder; it must not be used afterwards.
func (b *MapBuilder) Map() *Map {
	m := fromFlat(b.m)
	b.m = nil
	return m
}

// Tag is a tagged value: Circle(3), Ok(x). Its type name is the tag's name (02).
type Tag struct {
	Name Sym
	Args []Value
}

// Range is a lazy integer range (02): Start..End or Start..=End.
type Range struct {
	Start, End int64
	Inclusive  bool
}

func (r Range) Len() int64 {
	n := r.End - r.Start
	if r.Inclusive {
		n++
	}
	if n < 0 {
		return 0
	}
	return n
}

func (r Range) Contains(i int64) bool {
	if r.Inclusive {
		return i >= r.Start && i <= r.End
	}
	return i >= r.Start && i < r.End
}

// Items materializes the range (for map/for, which build lists anyway).
func (r Range) Items() []Value {
	n := r.Len()
	out := make([]Value, 0, n)
	for i := int64(0); i < n; i++ {
		out = append(out, Int(r.Start+i))
	}
	return out
}

func (*List) TypeName() string  { return "list" }
func (*Tuple) TypeName() string { return "tuple" }
func (*Map) TypeName() string   { return "map" }
func (t *Tag) TypeName() string { return t.Name.Name() }
func (Range) TypeName() string  { return "range" }

func formatItems(items []Value) string {
	parts := make([]string, len(items))
	for i, v := range items {
		parts[i] = Format(v)
	}
	return strings.Join(parts, ", ")
}

// isPlainKey reports whether a map key can be written without quotes.
func isPlainKey(k string) bool {
	if k == "" {
		return false
	}
	for i, r := range k {
		if i == 0 && !(r == '_' || unicode.IsLetter(r)) {
			return false
		}
		if !(r == '_' || unicode.IsLetter(r) || unicode.IsDigit(r)) {
			return false
		}
	}
	return true
}

// seq is a read-only view of a list or tuple for comparisons.
type seq struct {
	n  int
	at func(int) Value
}

func listSeq(l *List) seq        { return seq{l.Len(), l.At} }
func sliceSeq(items []Value) seq { return seq{len(items), func(i int) Value { return items[i] }} }

// equalItems compares two sequences element by element (02).
func equalItems(a, b seq) bool {
	if a.n != b.n {
		return false
	}
	for i := 0; i < a.n; i++ {
		if !Equal(a.at(i), b.at(i)) {
			return false
		}
	}
	return true
}

// compareItems orders two sequences lexicographically; a shorter prefix
// is smaller (02).
func compareItems(a, b seq) (int, bool) {
	for i := 0; i < a.n && i < b.n; i++ {
		c, err := Compare(a.at(i), b.at(i))
		if err != nil {
			return 0, false
		}
		if c != 0 {
			return c, true
		}
	}
	return cmp3(a.n < b.n, a.n > b.n), true
}
