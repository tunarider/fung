// Package nat holds what the core prelude (internal/prelude) and the
// native bridges (internal/native) share: parameter declarations for Go
// functions, Ok/Err construction, and argument checks (06).
package nat

import (
	"os"
	"os/exec"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

func Param(name string) value.NativeParam { return value.NativeParam{Name: name} }
func Opt(name string, def value.Value) value.NativeParam {
	return value.NativeParam{Name: name, Default: def}
}
func Rest(name string) value.NativeParam { return value.NativeParam{Name: name, Rest: true} }

// Def declares a native with its parameter list (06: 같은 호출 규약). The
// VM checks arity and names; fn receives one slot per parameter, a rest
// parameter as a *value.List.
func Def(name string, fn func(value.Context, []value.Value) (value.Value, error), params ...value.NativeParam) *value.Native {
	return &value.Native{Name: name, Params: params, Fn: fn}
}

// Ok wraps a value as Ok(v).
func Ok(v value.Value) value.Value {
	return &value.Tag{Name: value.Intern("Ok"), Args: []value.Value{v}}
}

// ErrValue builds Err({kind, extra…, message}) (06: 표준 라이브러리 Err의
// 내용은 맵). extra holds key/value pairs, keys as Str; message comes
// last so the situation's fields read first.
func ErrValue(kind, message string, extra ...value.Value) value.Value {
	m := value.NewMapBuilder()
	m.Set("kind", value.Intern(kind))
	for i := 0; i+1 < len(extra); i += 2 {
		m.Set(string(extra[i].(value.Str)), extra[i+1])
	}
	m.Set("message", value.Str(message))
	return &value.Tag{Name: value.Intern("Err"), Args: []value.Value{m.Map()}}
}

// JoinForPrint renders arguments the way print shows them, space-separated.
func JoinForPrint(args []value.Value) string {
	parts := make([]string, len(args))
	for i, a := range args {
		parts[i] = value.FormatForPrint(a)
	}
	return strings.Join(parts, " ")
}

// Each walks what Items lists, one element at a time, stopping at the
// first error yield returns. Lists and ranges are walked in place: a range
// yields its numbers without ever building a list of them.
func Each(ctx value.Context, v value.Value, verb string, yield func(value.Value) error) error {
	switch x := v.(type) {
	case *value.List:
		for _, e := range x.All() {
			if err := yield(e); err != nil {
				return err
			}
		}
		return nil
	case value.Range:
		n := x.Len()
		for i := int64(0); i < n; i++ {
			if err := yield(value.Int(x.Start + i)); err != nil {
				return err
			}
		}
		return nil
	}
	xs, err := Items(ctx, v, verb)
	if err != nil {
		return err
	}
	for _, e := range xs {
		if err := yield(e); err != nil {
			return err
		}
	}
	return nil
}

// SizeHint is the element count of a list or range, 0 for anything else.
func SizeHint(v value.Value) int {
	switch x := v.(type) {
	case *value.List:
		return x.Len()
	case value.Range:
		return int(x.Len())
	}
	return 0
}

// Items lists what a collection function iterates (02): lists, ranges,
// and iterables such as channels, which drain until closed (05).
func Items(ctx value.Context, v value.Value, verb string) ([]value.Value, error) {
	switch x := v.(type) {
	case *value.List:
		return x.ToSlice(), nil
	case value.Range:
		return x.Items(), nil
	case value.Iterable:
		return x.Drain(ctx)
	}
	return nil, diag.Errorf("E0506", diag.Pos{}, verb, v.TypeName())
}

// CallBool runs a predicate and insists on a bool result (E0505).
func CallBool(ctx value.Context, f, x value.Value) (bool, error) {
	r, err := ctx.Call(f, []value.Value{x})
	if err != nil {
		return false, err
	}
	b, ok := r.(value.Bool)
	if !ok {
		return false, diag.Errorf("E0505", diag.Pos{}, r.TypeName())
	}
	return bool(b), nil
}

func NeedFn(name string, v value.Value) error {
	if _, ok := v.(value.Fn); !ok {
		return diag.Errorf("E0403", diag.Pos{}, name, "fn", v.TypeName())
	}
	return nil
}

func NeedMap(verb string, v value.Value) (*value.Map, error) {
	m, ok := v.(*value.Map)
	if !ok {
		return nil, diag.Errorf("E0506", diag.Pos{}, verb, v.TypeName())
	}
	return m, nil
}

func NeedKey(verb string, v value.Value) (string, error) {
	k, ok := v.(value.Str)
	if !ok {
		return "", diag.Errorf("E0501", diag.Pos{}, verb, "map", v.TypeName())
	}
	return string(k), nil
}

func NeedInt(name string, v value.Value) (int, error) {
	n, ok := v.(value.Int)
	if !ok {
		return 0, diag.Errorf("E0403", diag.Pos{}, name, "int", v.TypeName())
	}
	return int(n), nil
}

func NeedStr(name string, v value.Value) (string, error) {
	s, ok := v.(value.Str)
	if !ok {
		return "", diag.Errorf("E0403", diag.Pos{}, name, "str", v.TypeName())
	}
	return string(s), nil
}

// StrArgs checks that every named argument is a string, in order.
func StrArgs(args []value.Value, names ...string) ([]string, error) {
	out := make([]string, len(names))
	for i, n := range names {
		s, err := NeedStr(n, args[i])
		if err != nil {
			return nil, err
		}
		out[i] = s
	}
	return out, nil
}

// StrList reads a list of strings argument.
func StrList(name string, v value.Value) ([]string, error) {
	list, ok := v.(*value.List)
	if !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, name, "a list of strings", v.TypeName())
	}
	out := make([]string, list.Len())
	for i, item := range list.All() {
		s, ok := item.(value.Str)
		if !ok {
			return nil, diag.Errorf("E0403", diag.Pos{}, name, "a list of strings", "a list holding "+item.TypeName())
		}
		out[i] = string(s)
	}
	return out, nil
}

// OptStr reads a str | nil argument.
func OptStr(name string, v value.Value) (string, bool, error) {
	switch x := v.(type) {
	case value.Nil:
		return "", false, nil
	case value.Str:
		return string(x), true, nil
	}
	return "", false, diag.Errorf("E0403", diag.Pos{}, name, "str | nil", v.TypeName())
}

// NewList makes a list value; a nil slice becomes an empty list.
func NewList(items []value.Value) *value.List {
	if items == nil {
		items = []value.Value{}
	}
	return value.NewList(items)
}

// Reason strips Go's operation prefixes from an OS error so messages
// read "cannot run `x`: no such file or directory" (06).
func Reason(err error) string {
	for {
		switch e := err.(type) {
		case *exec.Error:
			err = e.Err
		case *os.PathError:
			err = e.Err
		case *os.LinkError:
			err = e.Err
		default:
			return err.Error()
		}
	}
}
