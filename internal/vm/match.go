package vm

import (
	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/value"
)

// matchPattern tests v against p and, on success, writes every bound
// name into its slot or global (09: 패턴 의미가 한 곳에 모인다). A failed
// match may have written some names already; the compiler never reads a
// name from a failed arm, so those writes are harmless.
func (t *Task) matchPattern(f *frame, v value.Value, p *bytecode.Pattern) bool {
	switch p.Kind {
	case bytecode.PatWild:
		return true
	case bytecode.PatBind:
		if p.Global {
			f.globals[p.Index] = v
		} else {
			t.stack[f.base+p.Index] = v
		}
		return true
	case bytecode.PatLit:
		return value.Equal(v, p.Lit)
	case bytecode.PatTag:
		tag, ok := v.(*value.Tag)
		if !ok || tag.Name.Name() != p.Name {
			return false
		}
		return t.matchAll(f, tag.Args, p.Kids)
	case bytecode.PatTuple:
		tup, ok := v.(*value.Tuple)
		if !ok {
			return false
		}
		return t.matchAll(f, tup.Items, p.Kids)
	case bytecode.PatList:
		list, ok := v.(*value.List)
		if !ok {
			return false
		}
		n := len(p.Kids)
		if list.Len() < n || (p.Rest == nil && list.Len() != n) {
			return false
		}
		// Only the first n items are matched one by one; the rest is a
		// slice of the same list (cheap for big lists, M10).
		head := make([]value.Value, n)
		for i := range head {
			head[i] = list.At(i)
		}
		if !t.matchAll(f, head, p.Kids) {
			return false
		}
		if p.Rest == nil {
			return true
		}
		return t.matchPattern(f, list.Slice(n, list.Len()), p.Rest)
	case bytecode.PatMap:
		m, ok := v.(*value.Map)
		if !ok {
			return false
		}
		for i, k := range p.Keys {
			item, found := m.Get(k)
			if !found || !t.matchPattern(f, item, p.Kids[i]) {
				return false
			}
		}
		return true
	case bytecode.PatAlt:
		for _, alt := range p.Kids {
			if t.matchPattern(f, v, alt) {
				return true
			}
		}
		return false
	}
	return false
}

// matchAll matches items against patterns pairwise; lengths must agree.
func (t *Task) matchAll(f *frame, items []value.Value, pats []*bytecode.Pattern) bool {
	if len(items) != len(pats) {
		return false
	}
	for i, p := range pats {
		if !t.matchPattern(f, items[i], p) {
			return false
		}
	}
	return true
}
