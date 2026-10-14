package value

// pvec is a persistent vector (Clojure's PersistentVector): a 32-way
// trie of leaves plus a tail of up to 32 items. Updates copy the path
// they change and share the rest, so old versions stay valid (M10).
const (
	pvBits  = 5
	pvWidth = 1 << pvBits
	pvMask  = pvWidth - 1
)

type pnode[T any] struct {
	kids []*pnode[T] // internal node
	vals []T         // leaf (always pvWidth items)
}

type pvec[T any] struct {
	cnt   int
	shift uint // bits below the root's children; leaves are at level 0
	root  *pnode[T]
	tail  []T
}

func (v *pvec[T]) tailOff() int { return v.cnt - len(v.tail) }

// leafFor returns the 32-item array holding index i (the tail or a leaf).
func (v *pvec[T]) leafFor(i int) []T {
	if i >= v.tailOff() {
		return v.tail
	}
	n := v.root
	for level := v.shift; level > 0; level -= pvBits {
		n = n.kids[(i>>level)&pvMask]
	}
	return n.vals
}

func (v *pvec[T]) at(i int) T { return v.leafFor(i)[i&pvMask] }

// pvecOf builds a vector from items in O(n): full 32-chunks become
// leaves, the last chunk the tail, then levels are grouped bottom-up.
func pvecOf[T any](items []T) *pvec[T] {
	n := len(items)
	if n == 0 {
		return &pvec[T]{shift: pvBits, root: &pnode[T]{}}
	}
	tailLen := n % pvWidth
	if tailLen == 0 {
		tailLen = pvWidth
	}
	off := n - tailLen
	tail := append([]T(nil), items[off:]...)
	var nodes []*pnode[T]
	for i := 0; i < off; i += pvWidth {
		nodes = append(nodes, &pnode[T]{vals: append([]T(nil), items[i:i+pvWidth]...)})
	}
	shift := uint(pvBits)
	for len(nodes) > pvWidth {
		var up []*pnode[T]
		for i := 0; i < len(nodes); i += pvWidth {
			up = append(up, &pnode[T]{kids: nodes[i:min(i+pvWidth, len(nodes)):min(i+pvWidth, len(nodes))]})
		}
		nodes = up
		shift += pvBits
	}
	return &pvec[T]{cnt: n, shift: shift, root: &pnode[T]{kids: nodes}, tail: tail}
}

func newPath[T any](level uint, node *pnode[T]) *pnode[T] {
	if level == 0 {
		return node
	}
	return &pnode[T]{kids: []*pnode[T]{newPath(level-pvBits, node)}}
}

// pushTail inserts a full tail leaf at the end of the trie, copying the
// path from parent down. v.cnt counts the items including that tail.
func (v *pvec[T]) pushTail(level uint, parent *pnode[T], leaf *pnode[T]) *pnode[T] {
	sub := ((v.cnt - 1) >> level) & pvMask
	kids := make([]*pnode[T], len(parent.kids), max(len(parent.kids), sub+1))
	copy(kids, parent.kids)
	var child *pnode[T]
	switch {
	case level == pvBits:
		child = leaf
	case sub < len(parent.kids):
		child = v.pushTail(level-pvBits, parent.kids[sub], leaf)
	default:
		child = newPath(level-pvBits, leaf)
	}
	if sub < len(kids) {
		kids[sub] = child
	} else {
		kids = append(kids, child)
	}
	return &pnode[T]{kids: kids}
}

func (v *pvec[T]) push(x T) *pvec[T] {
	if len(v.tail) < pvWidth {
		tail := make([]T, len(v.tail)+1, pvWidth)
		copy(tail, v.tail)
		tail[len(v.tail)] = x
		return &pvec[T]{cnt: v.cnt + 1, shift: v.shift, root: v.root, tail: tail}
	}
	leaf := &pnode[T]{vals: v.tail}
	shift := v.shift
	var root *pnode[T]
	if (v.cnt >> pvBits) > (1 << v.shift) { // the root is full: grow a level
		root = &pnode[T]{kids: []*pnode[T]{v.root, newPath(v.shift, leaf)}}
		shift += pvBits
	} else {
		root = v.pushTail(v.shift, v.root, leaf)
	}
	tail := make([]T, 1, pvWidth)
	tail[0] = x
	return &pvec[T]{cnt: v.cnt + 1, shift: shift, root: root, tail: tail}
}

func (v *pvec[T]) set(i int, x T) *pvec[T] {
	if i >= v.tailOff() {
		tail := append([]T(nil), v.tail...)
		tail[i&pvMask] = x
		return &pvec[T]{cnt: v.cnt, shift: v.shift, root: v.root, tail: tail}
	}
	return &pvec[T]{cnt: v.cnt, shift: v.shift, root: assocPath(v.shift, v.root, i, x), tail: v.tail}
}

func assocPath[T any](level uint, n *pnode[T], i int, x T) *pnode[T] {
	if level == 0 {
		vals := append([]T(nil), n.vals...)
		vals[i&pvMask] = x
		return &pnode[T]{vals: vals}
	}
	kids := append([]*pnode[T](nil), n.kids...)
	sub := (i >> level) & pvMask
	kids[sub] = assocPath(level-pvBits, kids[sub], i, x)
	return &pnode[T]{kids: kids}
}
