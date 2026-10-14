package value

import (
	"strconv"
	"testing"
)

func benchKeys(n int) []string {
	ks := make([]string, n)
	for i := range ks {
		ks[i] = "k" + strconv.Itoa(i)
	}
	return ks
}

func BenchmarkMapPutFold20k(b *testing.B) {
	ks := benchKeys(20000)
	for b.Loop() {
		m := NewMap()
		for i, k := range ks {
			m = m.Put(k, Int(i))
		}
	}
}

// The word-count shape: every key is updated again after it exists.
func BenchmarkMapPutExisting20k(b *testing.B) {
	ks := benchKeys(20000)
	for b.Loop() {
		m := NewMap()
		for i, k := range ks {
			m = m.Put(k, Int(i))
		}
		for i, k := range ks {
			m = m.Put(k, Int(i+1))
		}
	}
}

func BenchmarkListAppend20k(b *testing.B) {
	for b.Loop() {
		l := NewList(nil)
		for i := range 20000 {
			l = l.Append(Int(i))
		}
	}
}

func BenchmarkListRest20k(b *testing.B) {
	items := make([]Value, 20000)
	for i := range items {
		items[i] = Int(i)
	}
	full := NewList(items)
	for b.Loop() {
		l := full
		for l.Len() > 0 {
			l = l.Slice(1, l.Len())
		}
	}
}

func BenchmarkSmallMapGet(b *testing.B) {
	for _, n := range []int{4, 16, 32} {
		mb := NewMapBuilder()
		ks := benchKeys(n)
		for i, k := range ks {
			mb.Set(k, Int(i))
		}
		m := mb.Map()
		b.Run(strconv.Itoa(n), func(b *testing.B) {
			for b.Loop() {
				for _, k := range ks {
					m.Get(k)
				}
			}
		})
	}
}

func BenchmarkSmallMapPut(b *testing.B) {
	for _, n := range []int{4, 16, 32} {
		ks := benchKeys(n)
		b.Run(strconv.Itoa(n), func(b *testing.B) {
			for b.Loop() {
				m := NewMap()
				for i, k := range ks {
					m = m.Put(k, Int(i))
				}
			}
		})
	}
}

func BenchmarkSmallListAt(b *testing.B) {
	for _, n := range []int{4, 16, 32} {
		l := NewList(make([]Value, n))
		b.Run(strconv.Itoa(n), func(b *testing.B) {
			for b.Loop() {
				for i := range n {
					l.At(i)
				}
			}
		})
	}
}

func BenchmarkSmallListAppend(b *testing.B) {
	for _, n := range []int{4, 16, 32} {
		b.Run(strconv.Itoa(n), func(b *testing.B) {
			for b.Loop() {
				l := NewList(nil)
				for i := range n {
					l = l.Append(Int(i))
				}
			}
		})
	}
}
