package vm

import (
	"testing"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// TestRunTaskRunsOnAFreshTask: a native that runs a fung function
// through ctx.RunTask gets its result, its error, and sees the parent's
// cancellation signal.
func TestRunTaskRunsOnAFreshTask(t *testing.T) {
	var gotDone bool
	runTwice := &value.Native{
		Name:   "run_twice",
		Params: []value.NativeParam{{Name: "f"}},
		Fn: func(ctx value.Context, args []value.Value) (value.Value, error) {
			gotDone = ctx.Done() != nil
			a, err := ctx.RunTask(args[0], []value.Value{value.Int(1)})
			if err != nil {
				return nil, err
			}
			b, err := ctx.RunTask(args[0], []value.Value{value.Int(2)})
			if err != nil {
				return nil, err
			}
			sum, aerr := value.Add(a, b)
			if aerr != nil {
				return nil, aerr
			}
			return sum, nil
		},
	}
	// runWith boots a runtime whose prelude also has run_twice.
	runWith := func(src string) (value.Value, *diag.Error) {
		rt := &Runtime{}
		p := bootTestPrelude(t, rt)
		p["run_twice"] = runTwice
		b := buildWith(t, src, p)
		registerModules(rt, b)
		v, err := rt.Run(b.Proto)
		rt.Wait()
		return v, err
	}
	v, err := runWith("run_twice(x => x * 10)")
	if err != nil || v != value.Int(30) {
		t.Fatalf("got %v, %v", v, err)
	}
	if !gotDone {
		t.Error("the entry task must expose a Done channel")
	}
	if _, err := runWith("run_twice(x => fail(\"boom\"))"); err == nil || err.Code != "E0803" {
		t.Fatalf("the callee's error must come back: %v", err)
	}
}
