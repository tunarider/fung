package prelude

import (
	"github.com/tunarider/fung/internal/nat"
	"sort"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// collections registers the core collection functions (06): the ones
// that walk elements in Go. They iterate lists and ranges (02: 순회 대상);
// strings and maps are turned into lists with chars/pairs first. The
// combinators built from these live in extras/prelude.fung.
func collections(add func(*value.Native)) {
	add(nat.Def("filter", filterFn, nat.Param("xs"), nat.Param("f")))
	add(nat.Def("fold", foldFn, nat.Param("xs"), nat.Param("init"), nat.Param("f")))
	add(nat.Def("each", eachFn, nat.Param("xs"), nat.Param("f")))
	add(nat.Def("sort", sortFn, nat.Param("xs")))
	add(nat.Def("sort_by", sortByFn, nat.Param("xs"), nat.Param("f")))
	add(nat.Def("find", findFn, nat.Param("xs"), nat.Param("f")))
	add(nat.Def("any", anyFn, nat.Param("xs"), nat.Param("f")))
	add(nat.Def("all", allFn, nat.Param("xs"), nat.Param("f")))
	add(nat.Def("flatten", flattenFn, nat.Param("xs")))
	add(nat.Def("group_by", groupByFn, nat.Param("xs"), nat.Param("f")))
	add(nat.Def("pairs", pairsFn, nat.Param("m")))
	add(nat.Def("put", putFn, nat.Param("m"), nat.Param("k"), nat.Param("v")))
	add(nat.Def("merge", mergeFn, nat.Param("a"), nat.Param("b")))
	add(nat.Def("remove", removeFn, nat.Param("m"), nat.Param("k")))
	add(nat.Def("chars", charsFn, nat.Param("s")))
}

func filterFn(ctx value.Context, args []value.Value) (value.Value, error) {
	xs, err := nat.Items(ctx, args[0], "filter")
	if err != nil {
		return nil, err
	}
	if err := nat.NeedFn("f", args[1]); err != nil {
		return nil, err
	}
	out := []value.Value{}
	for _, x := range xs {
		keep, err := nat.CallBool(ctx, args[1], x)
		if err != nil {
			return nil, err
		}
		if keep {
			out = append(out, x)
		}
	}
	return nat.NewList(out), nil
}

func foldFn(ctx value.Context, args []value.Value) (value.Value, error) {
	xs, err := nat.Items(ctx, args[0], "fold")
	if err != nil {
		return nil, err
	}
	if err := nat.NeedFn("f", args[2]); err != nil {
		return nil, err
	}
	acc := args[1]
	for _, x := range xs {
		r, err := ctx.Call(args[2], []value.Value{acc, x})
		if err != nil {
			return nil, err
		}
		acc = r
	}
	return acc, nil
}

func eachFn(ctx value.Context, args []value.Value) (value.Value, error) {
	xs, err := nat.Items(ctx, args[0], "iterate over")
	if err != nil {
		return nil, err
	}
	if err := nat.NeedFn("f", args[1]); err != nil {
		return nil, err
	}
	for _, x := range xs {
		if _, err := ctx.Call(args[1], []value.Value{x}); err != nil {
			return nil, err
		}
	}
	return value.Nil{}, nil
}

// sortedBy sorts a copy of xs by keys (parallel to xs), stable; the first
// comparison error wins.
func sortedBy(xs, keys []value.Value) (value.Value, error) {
	idx := make([]int, len(xs))
	for i := range idx {
		idx[i] = i
	}
	var firstErr *diag.Error
	sort.SliceStable(idx, func(a, b int) bool {
		if firstErr != nil {
			return false
		}
		c, err := value.Compare(keys[idx[a]], keys[idx[b]])
		if err != nil {
			firstErr = err
			return false
		}
		return c < 0
	})
	if firstErr != nil {
		return nil, firstErr
	}
	out := make([]value.Value, len(xs))
	for i, j := range idx {
		out[i] = xs[j]
	}
	return nat.NewList(out), nil
}

func sortFn(ctx value.Context, args []value.Value) (value.Value, error) {
	xs, err := nat.Items(ctx, args[0], "sort")
	if err != nil {
		return nil, err
	}
	return sortedBy(xs, xs)
}

func sortByFn(ctx value.Context, args []value.Value) (value.Value, error) {
	xs, err := nat.Items(ctx, args[0], "sort")
	if err != nil {
		return nil, err
	}
	if err := nat.NeedFn("f", args[1]); err != nil {
		return nil, err
	}
	keys := make([]value.Value, len(xs))
	for i, x := range xs {
		k, err := ctx.Call(args[1], []value.Value{x})
		if err != nil {
			return nil, err
		}
		keys[i] = k
	}
	return sortedBy(xs, keys)
}

func findFn(ctx value.Context, args []value.Value) (value.Value, error) {
	xs, err := nat.Items(ctx, args[0], "search")
	if err != nil {
		return nil, err
	}
	if err := nat.NeedFn("f", args[1]); err != nil {
		return nil, err
	}
	for _, x := range xs {
		hit, err := nat.CallBool(ctx, args[1], x)
		if err != nil {
			return nil, err
		}
		if hit {
			return x, nil
		}
	}
	return value.Nil{}, nil
}

// quantify implements any (want true) and all (want false): it stops at
// the first element whose predicate result equals stop.
func quantify(ctx value.Context, args []value.Value, verb string, stop bool) (value.Value, error) {
	xs, err := nat.Items(ctx, args[0], verb)
	if err != nil {
		return nil, err
	}
	if err := nat.NeedFn("f", args[1]); err != nil {
		return nil, err
	}
	for _, x := range xs {
		r, err := nat.CallBool(ctx, args[1], x)
		if err != nil {
			return nil, err
		}
		if r == stop {
			return value.Bool(stop), nil
		}
	}
	return value.Bool(!stop), nil
}

func anyFn(ctx value.Context, args []value.Value) (value.Value, error) {
	return quantify(ctx, args, "test", true)
}

func allFn(ctx value.Context, args []value.Value) (value.Value, error) {
	return quantify(ctx, args, "test", false)
}

// flattenFn splices nested lists one level deep. It stays in Go: a fung
// fold with `acc + x` copies the accumulator per element (quadratic).
func flattenFn(ctx value.Context, args []value.Value) (value.Value, error) {
	xs, err := nat.Items(ctx, args[0], "flatten")
	if err != nil {
		return nil, err
	}
	out := []value.Value{}
	for _, x := range xs {
		if inner, ok := x.(*value.List); ok {
			for _, v := range inner.All() {
				out = append(out, v)
			}
		} else {
			out = append(out, x)
		}
	}
	return nat.NewList(out), nil
}

func groupByFn(ctx value.Context, args []value.Value) (value.Value, error) {
	xs, err := nat.Items(ctx, args[0], "group")
	if err != nil {
		return nil, err
	}
	if err := nat.NeedFn("f", args[1]); err != nil {
		return nil, err
	}
	// Keys in first-seen order, each group in input order.
	var order []string
	groups := map[string][]value.Value{}
	for _, x := range xs {
		k, err := ctx.Call(args[1], []value.Value{x})
		if err != nil {
			return nil, err
		}
		key, ok := k.(value.Str)
		if !ok {
			return nil, diag.Errorf("E0514", diag.Pos{}, k.TypeName())
		}
		if _, seen := groups[string(key)]; !seen {
			order = append(order, string(key))
		}
		groups[string(key)] = append(groups[string(key)], x)
	}
	out := value.NewMapBuilder()
	for _, k := range order {
		out.Set(k, value.NewList(groups[k]))
	}
	return out.Map(), nil
}

func pairsFn(ctx value.Context, args []value.Value) (value.Value, error) {
	m, err := nat.NeedMap("list the pairs of", args[0])
	if err != nil {
		return nil, err
	}
	out := make([]value.Value, 0, m.Len())
	for k, v := range m.All() {
		out = append(out, &value.Tuple{Items: []value.Value{value.Str(k), v}})
	}
	return nat.NewList(out), nil
}

func putFn(ctx value.Context, args []value.Value) (value.Value, error) {
	m, err := nat.NeedMap("put into", args[0])
	if err != nil {
		return nil, err
	}
	k, err := nat.NeedKey("index", args[1])
	if err != nil {
		return nil, err
	}
	return m.Put(k, args[2]), nil
}

func mergeFn(ctx value.Context, args []value.Value) (value.Value, error) {
	a, err := nat.NeedMap("merge", args[0])
	if err != nil {
		return nil, err
	}
	b, err := nat.NeedMap("merge", args[1])
	if err != nil {
		return nil, err
	}
	return a.Merge(b), nil
}

func removeFn(ctx value.Context, args []value.Value) (value.Value, error) {
	m, err := nat.NeedMap("remove from", args[0])
	if err != nil {
		return nil, err
	}
	k, err := nat.NeedKey("index", args[1])
	if err != nil {
		return nil, err
	}
	return m.Remove(k), nil
}

func charsFn(ctx value.Context, args []value.Value) (value.Value, error) {
	s, ok := args[0].(value.Str)
	if !ok {
		return nil, diag.Errorf("E0506", diag.Pos{}, "list the characters of", args[0].TypeName())
	}
	out := []value.Value{}
	for _, r := range string(s) {
		out = append(out, value.Str(string(r)))
	}
	return nat.NewList(out), nil
}
