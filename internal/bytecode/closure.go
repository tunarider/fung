package bytecode

import "github.com/tunarider/fung/internal/value"

// Closure is a function value: a proto plus the values it captured when
// it was created (09: 클로저는 값을 복사한다).
type Closure struct {
	Proto    *Proto
	Captured []value.Value
	Globals  []value.Value // the globals of the module the closure was created in (04)
}

func (*Closure) TypeName() string { return "fn" }

// FnName implements value.Fn; "" for lambdas and anonymous functions.
func (c *Closure) FnName() string { return c.Proto.Name }
