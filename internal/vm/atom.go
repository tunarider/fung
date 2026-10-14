package vm

import (
	"sync"
	"sync/atomic"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// Atom is the one mutable box in the language (05). The current value is
// an atomic pointer so deref and print never wait; the mutex only
// serialises swaps, which run the user's f under it.
type Atom struct {
	v      atomic.Pointer[value.Value]
	mu     sync.Mutex
	holder atomic.Pointer[Task] // the task inside swap, to catch re-entry (E0703)
}

func newAtom(v value.Value) *Atom {
	a := &Atom{}
	a.v.Store(&v)
	return a
}

func (a *Atom) load() value.Value { return *a.v.Load() }

func (*Atom) TypeName() string  { return "atom" }
func (a *Atom) Display() string { return "<atom " + value.Format(a.load()) + ">" }

func registerAtoms(add func(*value.Native)) {
	add(&value.Native{Name: "atom", Params: []value.NativeParam{{Name: "v"}}, Fn: atomFn})
	add(&value.Native{Name: "swap", Params: []value.NativeParam{{Name: "a"}, {Name: "f"}}, Fn: swapFn})
	add(&value.Native{Name: "deref", Params: []value.NativeParam{{Name: "a"}}, Fn: derefFn})
}

func needAtom(v value.Value) (*Atom, error) {
	a, ok := v.(*Atom)
	if !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "a", "atom", v.TypeName())
	}
	return a, nil
}

func atomFn(_ value.Context, args []value.Value) (value.Value, error) {
	return newAtom(args[0]), nil
}

// swapFn runs f on the current value under the lock, exactly once (05).
// Waiting for the lock counts for the deadlock detector; a swap of the
// same atom from inside f is E0703.
func swapFn(ctx value.Context, args []value.Value) (value.Value, error) {
	a, err := needAtom(args[0])
	if err != nil {
		return nil, err
	}
	if _, ok := args[1].(value.Fn); !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "f", "fn", args[1].TypeName())
	}
	t := ctx.(*Task)
	if !a.mu.TryLock() {
		if a.holder.Load() == t {
			return nil, diag.Errorf("E0703", diag.Pos{})
		}
		if berr := t.block(false, a.mu.Lock); berr != nil {
			return nil, berr
		}
	}
	a.holder.Store(t)
	defer func() {
		a.holder.Store(nil)
		a.mu.Unlock()
	}()
	r, cerr := ctx.Call(args[1], []value.Value{a.load()})
	if cerr != nil {
		return nil, cerr
	}
	a.v.Store(&r)
	return r, nil
}

func derefFn(_ value.Context, args []value.Value) (value.Value, error) {
	a, err := needAtom(args[0])
	if err != nil {
		return nil, err
	}
	return a.load(), nil
}
