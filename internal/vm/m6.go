package vm

import (
	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// Test is one registered test block: its name and the function to run.
type Test struct {
	Name string
	Fn   value.Value
}

// Tests lists the test blocks registered so far, in order.
func (rt *Runtime) Tests() []Test { return rt.tests }

// CallTest runs a test function on a fresh task and returns its error,
// including the exit signal, so `fung test` can report or stop.
func (rt *Runtime) CallTest(fn value.Value) *diag.Error {
	t := rt.newTask(rt.root())
	defer t.cancel()
	var err error
	rt.runLive(func() { _, err = t.Call(fn, nil) })
	if err == nil {
		return nil
	}
	return err.(*diag.Error)
}

// maxReplGlobals bounds the entry globals a REPL session may define: the
// table is allocated once at this capacity so closures from earlier
// lines keep seeing later definitions (08).
const maxReplGlobals = 65536

// PrepareREPL allocates the entry globals for a REPL session.
func (rt *Runtime) PrepareREPL() {
	rt.globals = make([]value.Value, 0, maxReplGlobals)
	rt.replMode = true
}

// compareBy applies an assert's comparison operator (08).
func compareBy(op string, l, r value.Value) (bool, *diag.Error) {
	switch op {
	case "==":
		return value.Equal(l, r), nil
	case "!=":
		return !value.Equal(l, r), nil
	}
	c, err := value.Compare(l, r)
	if err != nil {
		return false, err
	}
	switch op {
	case "<":
		return c < 0, nil
	case ">":
		return c > 0, nil
	case "<=":
		return c <= 0, nil
	}
	return c >= 0, nil
}

// execM6 runs ASSERT and TEST. It reports handled=false for other opcodes.
func (t *Task) execM6(f *frame, op bytecode.Op, in bytecode.Instr, pos diag.Pos) (bool, *diag.Error) {
	switch op {
	case bytecode.OpAssert:
		a := f.proto.Asserts[in.Arg()]
		if a.Op == "" {
			v := t.pop()
			b, ok := v.(value.Bool)
			if !ok {
				return true, diag.Errorf("E0505", pos, v.TypeName())
			}
			if !b {
				return true, diag.Errorf("E0807", pos, a.Text)
			}
			return true, nil
		}
		r, l := t.pop(), t.pop()
		pass, err := compareBy(a.Op, l, r)
		if err != nil {
			return true, err.WithPos(pos)
		}
		if !pass {
			e := diag.Errorf("E0807", pos, a.Text)
			if a.Left != "" {
				e.Notes = append(e.Notes, a.Left+" = "+value.Format(l))
			}
			if a.Right != "" {
				e.Notes = append(e.Notes, a.Right+" = "+value.Format(r))
			}
			return true, e
		}
	case bytecode.OpTest:
		fn := t.pop()
		t.rt.conc.mu.Lock()
		t.rt.tests = append(t.rt.tests, Test{Name: string(f.proto.Consts[in.Arg()].(value.Str)), Fn: fn})
		t.rt.conc.mu.Unlock()
	default:
		return false, nil
	}
	return true, nil
}
