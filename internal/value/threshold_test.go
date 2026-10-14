package value

import (
	"fmt"
	"testing"
)

// BenchmarkThreshold compares small-collection limits around their
// boundary: building and reading lists and maps of 16, 48 and 96 items
// with the limit at 16, 32 and 64 (M10, Task 6).
func BenchmarkThreshold(b *testing.B) {
	oldL, oldM := smallMax, smallMapMax
	defer func() { smallMax, smallMapMax = oldL, oldM }()
	for _, max := range []int{16, 32, 64} {
		smallMax, smallMapMax = max, max
		for _, n := range []int{16, 48, 96} {
			ks := benchKeys(n)
			b.Run(fmt.Sprintf("max=%d/list-append/n=%d", max, n), func(b *testing.B) {
				for b.Loop() {
					l := NewList(nil)
					for i := range n {
						l = l.Append(Int(i))
					}
				}
			})
			full := NewList(nil)
			for i := range n {
				full = full.Append(Int(i))
			}
			b.Run(fmt.Sprintf("max=%d/list-at/n=%d", max, n), func(b *testing.B) {
				for b.Loop() {
					for i := range n {
						full.At(i)
					}
				}
			})
			b.Run(fmt.Sprintf("max=%d/map-put/n=%d", max, n), func(b *testing.B) {
				for b.Loop() {
					m := NewMap()
					for i, k := range ks {
						m = m.Put(k, Int(i))
					}
				}
			})
			m := NewMap()
			for i, k := range ks {
				m = m.Put(k, Int(i))
			}
			b.Run(fmt.Sprintf("max=%d/map-get/n=%d", max, n), func(b *testing.B) {
				for b.Loop() {
					for _, k := range ks {
						m.Get(k)
					}
				}
			})
		}
	}
}
