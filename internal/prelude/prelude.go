// Package prelude holds the core prelude: functions implemented in Go that
// every fung program can call without importing anything (06).
package prelude

import (
	"github.com/tunarider/fung/internal/nat"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// Test is Core with no script arguments (for tests).
func Test() map[string]value.Value { return Core(nil) }

// Core returns the core prelude: the value operations the language
// guarantees (06), with `args` as the string list after the script path.
func Core(args []string) map[string]value.Value {
	p := map[string]value.Value{}
	items := make([]value.Value, len(args))
	for i, a := range args {
		items[i] = value.Str(a)
	}
	p["args"] = value.NewList(items)
	add := func(n *value.Native) { p[n.Name] = n }
	add(nat.Def("print", printFn, nat.Rest("args")))
	add(nat.Def("eprint", eprintFn, nat.Rest("args")))
	add(nat.Def("fail", failFn, nat.Rest("args")))
	add(nat.Def("at_exit", atExitFn, nat.Param("f")))
	add(nat.Def("len", lenFn, nat.Param("x")))
	add(nat.Def("map", mapFn, nat.Param("xs"), nat.Param("f")))
	add(nat.Def("type_of", typeOfFn, nat.Param("x")))
	add(nat.Def("exit", exitFn, nat.Opt("code", value.Int(0))))
	add(nat.Def("read_line", readLineFn, nat.Opt("f", value.Nil{})))
	add(nat.Def("read_lines", readLinesFn, nat.Opt("f", value.Nil{})))
	add(nat.Def("dbg", dbgFn, nat.Param("x"), nat.Opt("text", value.Nil{}), nat.Opt("where", value.Nil{})))
	collections(add)
	stringFns(add)
	convertFns(add)
	return p
}

// atExitFn registers a function to run when the program ends (03).
func atExitFn(ctx value.Context, args []value.Value) (value.Value, error) {
	if _, ok := args[0].(value.Fn); !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "f", "fn", args[0].TypeName())
	}
	ctx.AtExit(args[0])
	return value.Nil{}, nil
}

// failFn raises a runtime error with the joined arguments as the message
// (06: `x ?? fail("…")`). The VM fills in the position.
func failFn(_ value.Context, args []value.Value) (value.Value, error) {
	msg := "fail"
	if items := args[0].(*value.List).ToSlice(); len(items) > 0 {
		msg = nat.JoinForPrint(items)
	}
	return nil, diag.Errorf("E0803", diag.Pos{}, msg)
}

// printFn joins its arguments with spaces and writes one line (06).
func printFn(ctx value.Context, args []value.Value) (value.Value, error) {
	_, err := ctx.Stdout().Write([]byte(nat.JoinForPrint(args[0].(*value.List).ToSlice()) + "\n"))
	return value.Nil{}, err
}

// lenFn counts runes or elements (02).
func lenFn(_ value.Context, args []value.Value) (value.Value, error) {
	n, err := value.Len(args[0])
	if err != nil {
		return nil, err
	}
	return value.Int(n), nil
}

// typeOfFn returns the type name as a symbol (07: `type_of(x)`).
func typeOfFn(_ value.Context, args []value.Value) (value.Value, error) {
	return value.Intern(args[0].TypeName()), nil
}

// mapFn applies f to every element of a list or range and collects the
// results (06). `for` lowers to this (03).
func mapFn(ctx value.Context, args []value.Value) (value.Value, error) {
	if _, ok := args[1].(value.Fn); !ok {
		if _, e := nat.Items(ctx, args[0], "map over"); e != nil {
			return nil, e
		}
		return nil, diag.Errorf("E0403", diag.Pos{}, "f", "fn", args[1].TypeName())
	}
	out := make([]value.Value, 0, nat.SizeHint(args[0]))
	buf := make([]value.Value, 1)
	err := nat.Each(ctx, args[0], "map over", func(x value.Value) error {
		buf[0] = x
		r, err := ctx.Call(args[1], buf)
		out = append(out, r)
		return err
	})
	if err != nil {
		return nil, err
	}
	return value.NewList(out), nil
}
