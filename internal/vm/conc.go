package vm

import (
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// sleepFn waits for a duration; cancellation ends the wait with E0701.
func sleepFn(ctx value.Context, args []value.Value) (value.Value, error) {
	d, ok := args[0].(value.Dur)
	if !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "d", "dur", args[0].TypeName())
	}
	t := ctx.(*Task)
	if d <= 0 {
		return value.Nil{}, nil
	}
	if err := t.block(true, func() {
		timer := time.NewTimer(time.Duration(d))
		defer timer.Stop()
		select {
		case <-timer.C:
		case <-t.done():
		}
	}); err != nil {
		return nil, err
	}
	if t.isCancelled() {
		return nil, diag.Errorf("E0701", diag.Pos{})
	}
	return value.Nil{}, nil
}
