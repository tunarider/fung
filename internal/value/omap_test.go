package value

import (
	"fmt"
	"math/rand/v2"
	"strconv"
	"testing"
	"time"
)

// refMap is the reference model of 02: insertion order, an existing key
// keeps its place on put, a removed key comes back at the end.
type refMap struct {
	keys []string
	vals map[string]Value
}

func (r refMap) put(k string, v Value) refMap {
	out := refMap{keys: append([]string(nil), r.keys...), vals: map[string]Value{}}
	for a, b := range r.vals {
		out.vals[a] = b
	}
	if _, ok := out.vals[k]; !ok {
		out.keys = append(out.keys, k)
	}
	out.vals[k] = v
	return out
}

func (r refMap) remove(k string) refMap {
	out := refMap{vals: map[string]Value{}}
	for _, a := range r.keys {
		if a != k {
			out.keys = append(out.keys, a)
			out.vals[a] = r.vals[a]
		}
	}
	return out
}

func sameMap(t *testing.T, what string, m *Map, r refMap) {
	t.Helper()
	if m.Len() != len(r.keys) {
		t.Fatalf("%s: Len %d want %d", what, m.Len(), len(r.keys))
	}
	ks := m.Keys()
	for i, k := range r.keys {
		if ks[i] != k {
			t.Fatalf("%s: key %d is %q want %q", what, i, ks[i], k)
		}
		if v, ok := m.Get(k); !ok || v != r.vals[k] {
			t.Fatalf("%s: Get(%q) = %v,%v want %v", what, k, v, ok, r.vals[k])
		}
	}
	i := 0
	for k, v := range m.All() {
		if k != r.keys[i] || v != r.vals[k] {
			t.Fatalf("%s: All step %d gave %q", what, i, k)
		}
		i++
	}
	if _, ok := m.Get("absent-key"); ok {
		t.Fatalf("%s: absent key found", what)
	}
}

func TestMapMatchesReference(t *testing.T) {
	r := rand.New(rand.NewPCG(7, 8))
	type version struct {
		m   *Map
		ref refMap
	}
	cur := version{NewMap(), refMap{vals: map[string]Value{}}}
	var history []version
	keyspace := 40 // starts near the small/big boundary, then grows
	for step := 0; step < 20000; step++ {
		if step%2500 == 0 {
			keyspace *= 2
		}
		k := "k" + strconv.Itoa(r.IntN(keyspace))
		switch op := r.IntN(10); {
		case op < 6:
			v := Int(step)
			cur = version{cur.m.Put(k, v), cur.ref.put(k, v)}
		case op < 9:
			cur = version{cur.m.Remove(k), cur.ref.remove(k)}
		default: // merge a few keys from a fresh builder
			b := NewMapBuilder()
			ref := cur.ref
			for j := 0; j < 3; j++ {
				kk := "k" + strconv.Itoa(r.IntN(keyspace))
				b.Set(kk, Int(-step-j))
			}
			o := b.Map()
			for _, kk := range o.Keys() {
				v, _ := o.Get(kk)
				ref = ref.put(kk, v)
			}
			cur = version{cur.m.Merge(o), ref}
		}
		if step%397 == 0 {
			history = append(history, cur)
		}
		if step%1999 == 0 {
			for _, h := range history {
				sameMap(t, "history", h.m, h.ref)
			}
		}
	}
	sameMap(t, "final", cur.m, cur.ref)
	if !Equal(cur.m, cur.m.Put("zz", Int(1)).Remove("zz")) {
		t.Fatal("== must ignore internal state")
	}
}

func TestMapPutExistingKeepsPosition(t *testing.T) {
	m := NewMap()
	for i := range 100 {
		m = m.Put("k"+strconv.Itoa(i), Int(i))
	}
	m = m.Put("k0", Int(-1))
	if v, _ := m.Get("k0"); m.Keys()[0] != "k0" || v != Int(-1) {
		t.Fatal("put on an existing key moved it")
	}
	m = m.Remove("k0").Put("k0", Int(1))
	if ks := m.Keys(); ks[len(ks)-1] != "k0" || len(ks) != 100 {
		t.Fatal("a removed key comes back at the end")
	}
}

func TestMapBuilderBig(t *testing.T) {
	b := NewMapBuilder()
	ref := refMap{vals: map[string]Value{}}
	for i := range 500 {
		k := "k" + strconv.Itoa(i%300)
		b.Set(k, Int(i))
		ref = ref.put(k, Int(i))
	}
	sameMap(t, "builder", b.Map(), ref)
}

// 100,000 new keys then 100,000 updates must be linear overall.
func TestMapPutFoldIsCheap(t *testing.T) {
	start := time.Now()
	m := NewMap()
	for i := range 100000 {
		m = m.Put("k"+strconv.Itoa(i), Int(i))
	}
	for i := range 100000 {
		m = m.Put("k"+strconv.Itoa(i), Int(-i))
	}
	if took := time.Since(start); took > 2*time.Second {
		t.Fatalf("200,000 puts took %s", took)
	}
	if v, _ := m.Get("k99999"); v != Int(-99999) || m.Len() != 100000 {
		t.Fatal("contents")
	}
}

// Values are shared between tasks without locks: many goroutines
// deriving from the same big map and list must not see each other.
func TestSharedValuesAcrossGoroutines(t *testing.T) {
	base := NewMap()
	list := NewList(nil)
	for i := range 200 {
		base = base.Put("k"+strconv.Itoa(i), Int(i))
		list = list.Append(Int(i))
	}
	done := make(chan error, 8)
	for g := range 8 {
		go func() {
			m, l := base, list
			for i := range 500 {
				m = m.Put("g"+strconv.Itoa(g)+"-"+strconv.Itoa(i), Int(i))
				m = m.Put("k0", Int(g))
				l = l.Append(Int(g))
			}
			if v, _ := m.Get("k0"); v != Int(g) || m.Len() != 700 || l.Len() != 700 || l.At(699) != Int(g) {
				done <- fmt.Errorf("goroutine %d saw another's update", g)
				return
			}
			done <- nil
		}()
	}
	for range 8 {
		if err := <-done; err != nil {
			t.Fatal(err)
		}
	}
	if v, _ := base.Get("k0"); v != Int(0) || base.Len() != 200 || list.Len() != 200 {
		t.Fatal("the shared base changed")
	}
}
