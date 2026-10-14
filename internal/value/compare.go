package value

import (
	"strings"

	"github.com/tunarider/fung/internal/diag"
)

// Equal implements == from the table in 02: numbers compare by value
// across int/float (exactly, without rounding the int), same types by
// value, reference types by identity, different types are not equal.
func Equal(a, b Value) bool {
	switch x := a.(type) {
	case Int:
		switch y := b.(type) {
		case Int:
			return x == y
		case Float:
			return cmpIntFloat(int64(x), float64(y)) == 0
		}
		return false
	case Float:
		switch y := b.(type) {
		case Int:
			return cmpIntFloat(int64(y), float64(x)) == 0
		case Float:
			return x == y
		}
		return false
	case Bool:
		y, ok := b.(Bool)
		return ok && x == y
	case Nil:
		_, ok := b.(Nil)
		return ok
	case Str:
		y, ok := b.(Str)
		return ok && x == y
	case Sym:
		y, ok := b.(Sym)
		return ok && x == y
	case Dur:
		y, ok := b.(Dur)
		return ok && x == y
	case Time:
		y, ok := b.(Time)
		return ok && x.T.Equal(y.T)
	case *List:
		y, ok := b.(*List)
		return ok && equalItems(listSeq(x), listSeq(y))
	case *Tuple:
		y, ok := b.(*Tuple)
		return ok && equalItems(sliceSeq(x.Items), sliceSeq(y.Items))
	case *Map:
		y, ok := b.(*Map)
		if !ok || x.Len() != y.Len() {
			return false
		}
		for k, xv := range x.All() {
			yv, found := y.Get(k)
			if !found || !Equal(xv, yv) {
				return false
			}
		}
		return true
	case *Tag:
		y, ok := b.(*Tag)
		return ok && x.Name == y.Name && equalItems(sliceSeq(x.Args), sliceSeq(y.Args))
	case Range:
		y, ok := b.(Range)
		return ok && x == y
	case Fn:
		y, ok := b.(Fn)
		return ok && x == y
	case Opaque:
		return a == b
	case *Module:
		y, ok := b.(*Module)
		return ok && x == y
	case *File:
		y, ok := b.(*File)
		return ok && x == y
	}
	return false
}

// Compare implements < > <= >= for numbers, strings (code point order)
// and durations. Anything else is E0501.
func Compare(a, b Value) (int, *diag.Error) {
	switch x := a.(type) {
	case Int:
		switch y := b.(type) {
		case Int:
			return cmp3(x < y, x > y), nil
		case Float:
			return cmpIntFloat(int64(x), float64(y)), nil
		}
	case Float:
		switch y := b.(type) {
		case Int:
			return -cmpIntFloat(int64(y), float64(x)), nil
		case Float:
			return cmp3(x < y, x > y), nil
		}
	case Str:
		if y, ok := b.(Str); ok {
			return strings.Compare(string(x), string(y)), nil
		}
	case Dur:
		if y, ok := b.(Dur); ok {
			return cmp3(x < y, x > y), nil
		}
	case Time:
		if y, ok := b.(Time); ok {
			return cmp3(x.T.Before(y.T), x.T.After(y.T)), nil
		}
	case *List:
		if y, ok := b.(*List); ok {
			if c, ok := compareItems(listSeq(x), listSeq(y)); ok {
				return c, nil
			}
		}
	case *Tuple:
		if y, ok := b.(*Tuple); ok {
			if c, ok := compareItems(sliceSeq(x.Items), sliceSeq(y.Items)); ok {
				return c, nil
			}
		}
	}
	return 0, diag.Errorf("E0501", diag.Pos{}, "compare", a.TypeName(), b.TypeName())
}

func cmp3(less, greater bool) int {
	switch {
	case less:
		return -1
	case greater:
		return 1
	}
	return 0
}

// cmpIntFloat compares an int and a float exactly, without converting the
// int to float64 (02: 정확 비교). Floats are never NaN in fung.
func cmpIntFloat(i int64, f float64) int {
	if f >= 9223372036854775808.0 { // 2^63
		return -1
	}
	if f < -9223372036854775808.0 {
		return 1
	}
	t := int64(f) // exact: |f| < 2^63 and truncation of a float is exact
	switch {
	case i < t:
		return -1
	case i > t:
		return 1
	}
	frac := f - float64(t)
	switch {
	case frac > 0:
		return -1
	case frac < 0:
		return 1
	}
	return 0
}
