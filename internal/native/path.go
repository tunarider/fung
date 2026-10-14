package native

import (
	"github.com/tunarider/fung/internal/nat"
	"path/filepath"
	"strings"

	"github.com/tunarider/fung/internal/value"
)

// pathModule builds _native.path (06): pure path arithmetic, no I/O.
func pathModule() *value.Module {
	m := value.NewModule("path", "_native.path")
	add := func(n *value.Native) { m.Set(n.Name, n) }
	add(nat.Def("join", pathJoinFn, nat.Rest("parts")))
	add(nat.Def("base", pathBaseFn, nat.Param("p")))
	add(nat.Def("dir", pathDirFn, nat.Param("p")))
	add(nat.Def("ext", pathExtFn, nat.Param("p")))
	add(nat.Def("stem", pathStemFn, nat.Param("p")))
	add(nat.Def("abs", pathAbsFn, nat.Param("p")))
	return m
}

func pathJoinFn(_ value.Context, args []value.Value) (value.Value, error) {
	parts, err := nat.StrList("parts", args[0])
	if err != nil {
		return nil, err
	}
	return value.Str(filepath.Join(parts...)), nil
}

func pathFn(args []value.Value, f func(string) string) (value.Value, error) {
	p, err := nat.NeedStr("p", args[0])
	if err != nil {
		return nil, err
	}
	return value.Str(f(p)), nil
}

func pathBaseFn(_ value.Context, args []value.Value) (value.Value, error) {
	return pathFn(args, filepath.Base)
}

func pathDirFn(_ value.Context, args []value.Value) (value.Value, error) {
	return pathFn(args, filepath.Dir)
}

func pathExtFn(_ value.Context, args []value.Value) (value.Value, error) {
	return pathFn(args, filepath.Ext)
}

func pathStemFn(_ value.Context, args []value.Value) (value.Value, error) {
	return pathFn(args, func(p string) string {
		base := filepath.Base(p)
		return strings.TrimSuffix(base, filepath.Ext(base))
	})
}

func pathAbsFn(_ value.Context, args []value.Value) (value.Value, error) {
	return pathFn(args, func(p string) string {
		if abs, err := filepath.Abs(p); err == nil {
			return abs
		}
		return filepath.Clean(p)
	})
}
