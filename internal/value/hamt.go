package value

import (
	"hash/maphash"
	"math/bits"
)

// A HAMT (hash array mapped trie) from string keys to positions (M10).
// Each node covers 5 bits of the key's 32-bit hash: a bitmap says which
// of the 32 slots are present and a compact array holds only those.
// Updates copy the path they change; older tries stay valid. When every
// hash bit is used up, keys with the same hash share a collision node.

var hashSeed = maphash.MakeSeed()

// hashKey is a variable so tests can force collisions.
var hashKey = func(s string) uint32 {
	h := maphash.String(hashSeed, s)
	return uint32(h) ^ uint32(h>>32)
}

const hamtMaxShift = 30 // levels use shifts 0, 5, …, 30; beyond that, collision nodes

type hslot struct {
	key  string
	hash uint32
	pos  int
	sub  *hnode // non-nil: a child node instead of an entry
}

type hnode struct {
	bitmap uint32
	slots  []hslot
	coll   []hslot // collision node: entries whose hashes are identical
}

func (n *hnode) get(h uint32, key string) (int, bool) {
	for shift := uint(0); n != nil; shift += pvBits {
		if n.coll != nil {
			for _, s := range n.coll {
				if s.key == key {
					return s.pos, true
				}
			}
			return 0, false
		}
		bit := uint32(1) << ((h >> shift) & pvMask)
		if n.bitmap&bit == 0 {
			return 0, false
		}
		s := n.slots[bits.OnesCount32(n.bitmap&(bit-1))]
		if s.sub == nil {
			return s.pos, s.key == key
		}
		n = s.sub
	}
	return 0, false
}

// set returns a trie with key at pos; it works on a nil (empty) trie.
func (n *hnode) set(shift uint, h uint32, key string, pos int) *hnode {
	if n == nil {
		n = &hnode{}
	}
	if n.coll != nil {
		coll := append([]hslot(nil), n.coll...)
		for i := range coll {
			if coll[i].key == key {
				coll[i].pos = pos
				return &hnode{coll: coll}
			}
		}
		return &hnode{coll: append(coll, hslot{key: key, hash: h, pos: pos})}
	}
	bit := uint32(1) << ((h >> shift) & pvMask)
	i := bits.OnesCount32(n.bitmap & (bit - 1))
	if n.bitmap&bit == 0 {
		slots := make([]hslot, len(n.slots)+1)
		copy(slots, n.slots[:i])
		slots[i] = hslot{key: key, hash: h, pos: pos}
		copy(slots[i+1:], n.slots[i:])
		return &hnode{bitmap: n.bitmap | bit, slots: slots}
	}
	slots := append([]hslot(nil), n.slots...)
	s := slots[i]
	switch {
	case s.sub != nil:
		slots[i].sub = s.sub.set(shift+pvBits, h, key, pos)
	case s.key == key:
		slots[i].pos = pos
	default:
		slots[i] = hslot{sub: pair(shift+pvBits, s, hslot{key: key, hash: h, pos: pos})}
	}
	return &hnode{bitmap: n.bitmap, slots: slots}
}

// pair makes the smallest subtree holding two entries whose hashes agree
// on the bits below shift.
func pair(shift uint, a, b hslot) *hnode {
	if shift > hamtMaxShift { // every hash bit is used: a true collision
		return &hnode{coll: []hslot{a, b}}
	}
	ia, ib := (a.hash>>shift)&pvMask, (b.hash>>shift)&pvMask
	if ia == ib {
		return &hnode{bitmap: 1 << ia, slots: []hslot{{sub: pair(shift+pvBits, a, b)}}}
	}
	if ia > ib {
		a, b, ia, ib = b, a, ib, ia
	}
	return &hnode{bitmap: 1<<ia | 1<<ib, slots: []hslot{a, b}}
}

// del returns the trie without key (nil when it becomes empty) and
// whether the key was there.
func (n *hnode) del(shift uint, h uint32, key string) (*hnode, bool) {
	if n == nil {
		return nil, false
	}
	if n.coll != nil {
		for i, s := range n.coll {
			if s.key == key {
				coll := append(append([]hslot(nil), n.coll[:i]...), n.coll[i+1:]...)
				if len(coll) == 0 {
					return nil, true
				}
				return &hnode{coll: coll}, true
			}
		}
		return n, false
	}
	bit := uint32(1) << ((h >> shift) & pvMask)
	if n.bitmap&bit == 0 {
		return n, false
	}
	i := bits.OnesCount32(n.bitmap & (bit - 1))
	s := n.slots[i]
	if s.sub != nil {
		sub, ok := s.sub.del(shift+pvBits, h, key)
		if !ok {
			return n, false
		}
		if sub == nil {
			return removeSlot(n, i, bit), true
		}
		slots := append([]hslot(nil), n.slots...)
		slots[i].sub = sub
		return &hnode{bitmap: n.bitmap, slots: slots}, true
	}
	if s.key != key {
		return n, false
	}
	return removeSlot(n, i, bit), true
}

func removeSlot(n *hnode, i int, bit uint32) *hnode {
	if len(n.slots) == 1 {
		return nil
	}
	slots := append(append([]hslot(nil), n.slots[:i]...), n.slots[i+1:]...)
	return &hnode{bitmap: n.bitmap &^ bit, slots: slots}
}
