package value

import (
	"strings"

	"github.com/tunarider/fung/internal/diag"
)

// Len counts runes of a string or elements of a collection (02).
func Len(v Value) (int, *diag.Error) {
	switch x := v.(type) {
	case Str:
		return len([]rune(string(x))), nil
	case *List:
		return x.Len(), nil
	case *Tuple:
		return len(x.Items), nil
	case *Map:
		return x.Len(), nil
	case Range:
		return int(x.Len()), nil
	}
	return 0, diag.Errorf("E0506", diag.Pos{}, "take the length of", v.TypeName())
}

// normIndex turns a possibly negative index into a position, or E0507.
func normIndex(i int64, n int) (int, *diag.Error) {
	idx := int(i)
	if idx < 0 {
		idx += n
	}
	if idx < 0 || idx >= n {
		return 0, diag.Errorf("E0507", diag.Pos{}, i, n)
	}
	return idx, nil
}

// Index implements x[i] (02): lists and strings by position (negative
// from the end), maps by string key, modules by member name (04: 비공개
// 이름은 E0304, 없는 이름은 E0303). Out of range is an error.
func Index(v, i Value) (Value, *diag.Error) {
	switch x := v.(type) {
	case *Module:
		k, ok := i.(Str)
		if !ok {
			return nil, diag.Errorf("E0501", diag.Pos{}, "index", v.TypeName(), i.TypeName())
		}
		if strings.HasPrefix(string(k), "_") {
			return nil, diag.Errorf("E0304", diag.Pos{}, string(k), x.Name)
		}
		member, found := x.Get(string(k))
		if !found {
			return nil, diag.Errorf("E0303", diag.Pos{}, x.Name, string(k))
		}
		return member, nil
	case *List:
		n, ok := i.(Int)
		if !ok {
			return nil, diag.Errorf("E0501", diag.Pos{}, "index", v.TypeName(), i.TypeName())
		}
		idx, err := normIndex(int64(n), x.Len())
		if err != nil {
			return nil, err
		}
		return x.At(idx), nil
	case Str:
		n, ok := i.(Int)
		if !ok {
			return nil, diag.Errorf("E0501", diag.Pos{}, "index", v.TypeName(), i.TypeName())
		}
		rs := []rune(string(x))
		idx, err := normIndex(int64(n), len(rs))
		if err != nil {
			return nil, err
		}
		return Str(string(rs[idx])), nil
	case *Map:
		k, ok := i.(Str)
		if !ok {
			return nil, diag.Errorf("E0501", diag.Pos{}, "index", v.TypeName(), i.TypeName())
		}
		val, found := x.Get(string(k))
		if !found {
			return nil, diag.Errorf("E0508", diag.Pos{}, string(k))
		}
		return val, nil
	}
	return nil, diag.Errorf("E0509", diag.Pos{}, v.TypeName())
}

// sliceBounds resolves optional start/end (Nil = open) against length n.
// Both ends may equal n; start may not exceed end (02: 범위를 벗어나면 에러).
// An inclusive end (`a..=b`) is normalized first, then extended by one.
func sliceBounds(start, end Value, n int, inclusive bool) (int, int, *diag.Error) {
	lo, hi := 0, n
	switch s := start.(type) {
	case Int:
		lo = int(s)
		if lo < 0 {
			lo += n
		}
	case Nil:
	default:
		return 0, 0, diag.Errorf("E0501", diag.Pos{}, "slice with", "int", start.TypeName())
	}
	switch e := end.(type) {
	case Int:
		hi = int(e)
		if hi < 0 {
			hi += n
		}
		if inclusive {
			hi++
		}
	case Nil:
	default:
		return 0, 0, diag.Errorf("E0501", diag.Pos{}, "slice with", "int", end.TypeName())
	}
	if lo < 0 || lo > n {
		return 0, 0, diag.Errorf("E0507", diag.Pos{}, lo, n)
	}
	if hi < 0 || hi > n {
		return 0, 0, diag.Errorf("E0507", diag.Pos{}, hi, n)
	}
	if lo > hi {
		return 0, 0, diag.Errorf("E0507", diag.Pos{}, lo, n)
	}
	return lo, hi, nil
}

// Slice implements x[a..b] and x[a..=b] on lists and strings (02).
func Slice(v, start, end Value, inclusive bool) (Value, *diag.Error) {
	switch x := v.(type) {
	case *List:
		lo, hi, err := sliceBounds(start, end, x.Len(), inclusive)
		if err != nil {
			return nil, err
		}
		return x.Slice(lo, hi), nil
	case Str:
		rs := []rune(string(x))
		lo, hi, err := sliceBounds(start, end, len(rs), inclusive)
		if err != nil {
			return nil, err
		}
		return Str(string(rs[lo:hi])), nil
	}
	return nil, diag.Errorf("E0511", diag.Pos{}, v.TypeName())
}

// Contains implements `in` (07): list elements, substrings, map keys,
// integers of a range.
func Contains(container, x Value) (bool, *diag.Error) {
	switch c := container.(type) {
	case *List:
		for _, item := range c.All() {
			if Equal(item, x) {
				return true, nil
			}
		}
		return false, nil
	case Str:
		s, ok := x.(Str)
		if !ok {
			return false, diag.Errorf("E0501", diag.Pos{}, "search", container.TypeName(), x.TypeName())
		}
		return strings.Contains(string(c), string(s)), nil
	case *Map:
		k, ok := x.(Str)
		if !ok {
			return false, diag.Errorf("E0501", diag.Pos{}, "search", container.TypeName(), x.TypeName())
		}
		_, found := c.Get(string(k))
		return found, nil
	case Range:
		i, ok := x.(Int)
		if !ok {
			return false, nil
		}
		return c.Contains(int64(i)), nil
	}
	return false, diag.Errorf("E0512", diag.Pos{}, container.TypeName())
}
