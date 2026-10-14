package vm

import (
	"context"
	"io"
	"sync/atomic"
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// TaskHandle is the value spawn returns (05): the task's result once it
// has finished, and a way to cancel it. Results are kept, so await can
// be called any number of times.
type TaskHandle struct {
	done    chan struct{}
	result  value.Value
	waiters atomic.Int32 // tasks currently inside await (05: 죽는 순간 기다리는 이)
	cancel  context.CancelFunc
}

func (*TaskHandle) TypeName() string { return "task" }
func (*TaskHandle) Display() string  { return "<task>" }

func registerTasks(add func(*value.Native)) {
	add(&value.Native{Name: "spawn", Params: []value.NativeParam{{Name: "f"}}, Fn: spawnFn})
	add(&value.Native{Name: "await", Params: []value.NativeParam{{Name: "t"}, {Name: "timeout", Default: value.Nil{}}}, Fn: awaitFn})
	add(&value.Native{Name: "cancel", Params: []value.NativeParam{{Name: "t"}}, Fn: cancelFn})
}

// errKind builds Err({kind: 'k}) for task results.
func errKind(kind string) value.Value {
	m := value.NewMapBuilder()
	m.Set("kind", value.Intern(kind))
	return &value.Tag{Name: value.Intern("Err"), Args: []value.Value{m.Map()}}
}

// spawn runs fn on a new task whose context descends from this one, so
// cancelling the parent cancels it too (05).
//
// awaited marks the task as awaited before it starts: a helper that
// promises to await it (spawn_awaited) must have its waiter counted before
// the task can finish, or a quick failure would be reported as unawaited.
func (t *Task) spawn(fn value.Value, awaited bool) *TaskHandle {
	child := t.rt.newTask(t.ctxOrRoot())
	h := &TaskHandle{done: make(chan struct{}), cancel: child.cancel}
	if awaited {
		h.waiters.Add(1) // never released; the failure stays with its Err
	}
	child.handle = h
	rt := t.rt
	rt.conc.wg.Add(1)
	rt.addLive()
	go func() {
		defer rt.conc.wg.Done()
		defer rt.dropLive()
		defer close(h.done)
		defer child.cancel()
		v, err := child.guarded(func() (value.Value, *diag.Error) {
			r, e := child.Call(fn, nil)
			if e != nil {
				return nil, e.(*diag.Error)
			}
			return r, nil
		})
		h.finish(rt, v, err)
	}()
	return h
}

// ctxOrRoot is this task's context, or the root for tasks made without one.
func (t *Task) ctxOrRoot() context.Context {
	if t.ctx == nil {
		return t.rt.root()
	}
	return t.ctx
}

// finish stores the task's result (05: 태스크의 결과와 에러). A failure
// nobody is awaiting is reported now and makes the exit code 1.
func (h *TaskHandle) finish(rt *Runtime, v value.Value, err *diag.Error) {
	switch {
	case err == nil:
		h.result = v
	case err.Code == "E0701", err.Code == "E0702":
		// Cancelled, or woken by the deadlock report: not this task's failure.
		h.result = errKind("cancelled")
	default:
		if _, isExit := err.ExitCode(); isExit {
			h.result = errKind("cancelled")
			return
		}
		m := value.NewMapBuilder()
		m.Set("kind", value.Intern("error"))
		m.Set("message", value.Str(err.Error()))
		h.result = &value.Tag{Name: value.Intern("Err"), Args: []value.Value{m.Map()}}
		if h.waiters.Load() == 0 {
			io.WriteString(rt.stderr(), "task failed: "+diag.Format(err, rt.Sources[err.Pos.File]))
			rt.conc.mu.Lock()
			rt.conc.failed = true
			rt.conc.mu.Unlock()
		}
	}
}

func spawnFn(ctx value.Context, args []value.Value) (value.Value, error) {
	if _, ok := args[0].(value.Fn); !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "f", "fn", args[0].TypeName())
	}
	return ctx.(*Task).spawn(args[0], false), nil
}

// timeoutArg reads a `timeout` argument: nil, or a duration.
func timeoutArg(v value.Value) (time.Duration, bool, error) {
	switch x := v.(type) {
	case value.Nil:
		return 0, false, nil
	case value.Dur:
		return time.Duration(x), true, nil
	}
	return 0, false, diag.Errorf("E0403", diag.Pos{}, "timeout", "dur | nil", v.TypeName())
}

// awaitFn waits for a task's result (05). Waiting is cancellable and
// counts for the deadlock detector; a timeout keeps the task running.
func awaitFn(ctx value.Context, args []value.Value) (value.Value, error) {
	h, ok := args[0].(*TaskHandle)
	if !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "t", "task", args[0].TypeName())
	}
	d, timed, err := timeoutArg(args[1])
	if err != nil {
		return nil, err
	}
	t := ctx.(*Task)
	h.waiters.Add(1)
	defer h.waiters.Add(-1)
	// Fast path: a finished task is not something to wait for, so it must
	// not count toward the deadlock detector.
	select {
	case <-h.done:
		return h.result, nil
	default:
	}
	timedOut := false
	if berr := t.block(timed, func() {
		var timer <-chan time.Time
		if timed {
			tm := time.NewTimer(d)
			defer tm.Stop()
			timer = tm.C
		}
		select {
		case <-h.done:
		case <-timer:
			timedOut = true
		case <-t.done():
		}
	}); berr != nil {
		return nil, berr
	}
	if t.isCancelled() {
		// Cancellation (an exit anywhere, 05) wins over a result that
		// arrived at the same time.
		return nil, diag.Errorf("E0701", diag.Pos{})
	}
	select {
	case <-h.done:
		return h.result, nil
	default:
	}
	if timedOut {
		return errKind("timeout"), nil
	}
	return nil, diag.Errorf("E0801", diag.Pos{}, "await woke without a result")
}

// NativeModules returns the hidden modules the fung stdlib builds on for
// concurrency (06): `_native.conc` with spawn_awaited, a spawn whose task
// counts as awaited from the start, so helpers such as pmap that await
// every task they spawn never report a failure twice.
func NativeModules() map[string]*value.Module {
	m := value.NewModule("conc", "_native.conc")
	m.Set("spawn_awaited", &value.Native{Name: "spawn_awaited", Params: []value.NativeParam{{Name: "f"}}, Fn: spawnAwaitedFn})
	return map[string]*value.Module{"_native.conc": m}
}

func spawnAwaitedFn(ctx value.Context, args []value.Value) (value.Value, error) {
	if _, ok := args[0].(value.Fn); !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "f", "fn", args[0].TypeName())
	}
	return ctx.(*Task).spawn(args[0], true), nil
}

func cancelFn(_ value.Context, args []value.Value) (value.Value, error) {
	h, ok := args[0].(*TaskHandle)
	if !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "t", "task", args[0].TypeName())
	}
	h.cancel()
	return value.Nil{}, nil
}
