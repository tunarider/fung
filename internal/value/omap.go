package value

// omap is a big map (M10): entries in insertion order in a persistent
// vector, and a HAMT from key to entry position. Removing a key leaves a
// dead entry behind; when dead entries outnumber live ones the map is
// rebuilt, so removal stays cheap on average.
type entry struct {
	key  string
	val  Value
	live bool
}

type omap struct {
	ents       *pvec[entry]
	idx        *hnode
	live, dead int
}

func omapOf(keys []string, val func(string) Value) *omap {
	es := make([]entry, len(keys))
	var idx *hnode
	for i, k := range keys {
		es[i] = entry{key: k, val: val(k), live: true}
		idx = idx.set(0, hashKey(k), k, i)
	}
	return &omap{ents: pvecOf(es), idx: idx, live: len(keys)}
}

func (o *omap) get(k string) (Value, bool) {
	pos, ok := o.idx.get(hashKey(k), k)
	if !ok {
		return nil, false
	}
	return o.ents.at(pos).val, true
}

// put sets k; an existing key keeps its position (02).
func (o *omap) put(k string, v Value) *omap {
	h := hashKey(k)
	if pos, ok := o.idx.get(h, k); ok {
		return &omap{ents: o.ents.set(pos, entry{key: k, val: v, live: true}), idx: o.idx, live: o.live, dead: o.dead}
	}
	pos := o.ents.cnt
	return &omap{ents: o.ents.push(entry{key: k, val: v, live: true}), idx: o.idx.set(0, h, k, pos), live: o.live + 1, dead: o.dead}
}

func (o *omap) remove(k string) *omap {
	h := hashKey(k)
	pos, ok := o.idx.get(h, k)
	if !ok {
		return o
	}
	idx, _ := o.idx.del(0, h, k)
	out := &omap{ents: o.ents.set(pos, entry{}), idx: idx, live: o.live - 1, dead: o.dead + 1}
	if out.dead > out.live {
		return out.compact()
	}
	return out
}

// each yields live entries in order, a leaf at a time.
func (o *omap) each(yield func(string, Value) bool) {
	for i := 0; i < o.ents.cnt; {
		leaf := o.ents.leafFor(i)
		for j := i & pvMask; j < len(leaf) && i < o.ents.cnt; j, i = j+1, i+1 {
			if e := leaf[j]; e.live && !yield(e.key, e.val) {
				return
			}
		}
	}
}

// liveFlat lists the live keys in order with their values.
func (o *omap) liveFlat() ([]string, map[string]Value) {
	keys := make([]string, 0, o.live)
	vals := make(map[string]Value, o.live)
	o.each(func(k string, v Value) bool {
		keys = append(keys, k)
		vals[k] = v
		return true
	})
	return keys, vals
}

func (o *omap) compact() *omap {
	keys, vals := o.liveFlat()
	return omapOf(keys, func(k string) Value { return vals[k] })
}
