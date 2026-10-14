package vm

import (
	"context"
	"io"
	"sync"
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// lockedWriter makes each Write atomic so lines from different tasks do
// not interleave (05: 출력 원자성).
type lockedWriter struct {
	mu sync.Mutex
	w  io.Writer
}

func (l *lockedWriter) Write(b []byte) (int, error) {
	l.mu.Lock()
	defer l.mu.Unlock()
	return l.w.Write(b)
}

// concState is the runtime's shared concurrency state (05, 09): the root
// context every task descends from, the wait group of spawned tasks, the
// counters the deadlock detector reads, and the first exit request.
type concState struct {
	rootOnce   sync.Once
	rootCtx    context.Context
	rootCancel context.CancelFunc
	wg         sync.WaitGroup
	// exit handlers run after the root is cancelled, on their own context
	// that a later exit(n) (from a task a handler spawned) still cancels.
	handlerOnce   sync.Once
	handlerCtx    context.Context
	handlerCancel context.CancelFunc
	stdinMu       sync.Mutex
	stdinOnce     sync.Once

	mu         sync.Mutex // guards the fields below
	live       int        // tasks running fung code (the entry included)
	blocked    int        // tasks waiting in a fung primitive (await, recv, send, swap, sleep)
	waking     int        // of those, tasks a timer will wake (sleep, timeouts)
	gen        uint64     // bumped whenever live/blocked change: the deadlock watchdog's "nothing moved" check
	deadlocked bool
	exitSet    bool
	exit       int
	failed     bool // a task died unawaited (05: exit code 1)
}

// deadlockGrace is how long every live task must stay blocked, with no
// change at all, before the watchdog declares a deadlock. A task whose
// wait is already satisfiable (a full channel, a finished task) is still
// counted as blocked until it is scheduled; the grace period absorbs that.
const deadlockGrace = 50 * time.Millisecond

// root returns the context all tasks descend from; CancelAll cancels it.
func (rt *Runtime) root() context.Context {
	rt.conc.rootOnce.Do(func() {
		rt.conc.rootCtx, rt.conc.rootCancel = context.WithCancel(context.Background())
	})
	return rt.conc.rootCtx
}

// CancelAll cancels every task (exit, deadlock, an entry error).
func (rt *Runtime) CancelAll() {
	rt.root()
	rt.conc.rootCancel()
}

// Wait blocks until every spawned task has finished.
func (rt *Runtime) Wait() { rt.conc.wg.Wait() }

// ExitCode reports the code of the first exit(n) requested anywhere.
func (rt *Runtime) ExitCode() (int, bool) {
	rt.conc.mu.Lock()
	defer rt.conc.mu.Unlock()
	return rt.conc.exit, rt.conc.exitSet
}

// Failed reports whether a task died without anyone awaiting it (05).
func (rt *Runtime) Failed() bool {
	rt.conc.mu.Lock()
	defer rt.conc.mu.Unlock()
	return rt.conc.failed
}

// requestExit records the first exit code and cancels every task: the
// requesting task unwinds on its exit signal, the others on E0701. An
// exit requested while handlers run cancels the handler context too.
func (rt *Runtime) requestExit(code int) {
	rt.conc.mu.Lock()
	if !rt.conc.exitSet {
		rt.conc.exitSet, rt.conc.exit = true, code
	}
	cancelHandlers := rt.conc.handlerCancel
	rt.conc.mu.Unlock()
	rt.CancelAll()
	if cancelHandlers != nil {
		cancelHandlers()
	}
}

// handlerContext is the context exit handlers run on: not the root
// (already cancelled by exit), but still cancellable by a later exit.
func (rt *Runtime) handlerContext() context.Context {
	rt.conc.handlerOnce.Do(func() {
		ctx, cancel := context.WithCancel(context.Background())
		rt.conc.mu.Lock()
		rt.conc.handlerCtx, rt.conc.handlerCancel = ctx, cancel
		exited := rt.conc.exitSet
		rt.conc.mu.Unlock()
		_ = exited
	})
	return rt.conc.handlerCtx
}

// deadlock reports that every live task is waiting (05) and ends the
// program with exit code 1. Every blocked task then wakes (the root is
// cancelled) and returns E0702.
func (rt *Runtime) deadlock() {
	io.WriteString(rt.stderr(), diag.Errorf("E0702", diag.Pos{}).Error()+"\n")
	rt.requestExit(1)
}

// watchdog runs after deadlockGrace: if nothing moved since gen and every
// live task is still blocked with no timer to wake one, it is a deadlock.
func (rt *Runtime) watchdog(gen uint64) {
	c := &rt.conc
	c.mu.Lock()
	stuck := c.gen == gen && c.live > 0 && c.blocked == c.live && c.waking == 0 && !c.exitSet && !c.deadlocked
	if stuck {
		c.deadlocked = true
	}
	c.mu.Unlock()
	if stuck {
		rt.deadlock()
	}
}

// addLive / dropLive bracket a task running fung code: the entry, spawned
// tasks, test bodies, exit handlers, module bodies. When the last runner
// finishes, the tasks still blocked are judged again.
func (rt *Runtime) addLive() {
	rt.conc.mu.Lock()
	rt.conc.live++
	rt.conc.gen++
	rt.conc.mu.Unlock()
}

func (rt *Runtime) dropLive() {
	rt.conc.mu.Lock()
	rt.conc.live--
	rt.conc.gen++
	rt.armWatchdog()
	rt.conc.mu.Unlock()
}

// armWatchdog (conc.mu held) schedules a judgement when every live task
// is blocked with nothing to wake it.
func (rt *Runtime) armWatchdog() {
	c := &rt.conc
	if c.live > 0 && c.blocked == c.live && c.waking == 0 && !c.deadlocked && !c.exitSet {
		gen := c.gen
		time.AfterFunc(deadlockGrace, func() { rt.watchdog(gen) })
	}
}

// runLive runs fn on t as a live task (CallTest, exit handlers, module
// bodies), so the deadlock detector sees it.
func (rt *Runtime) runLive(fn func()) {
	rt.addLive()
	defer rt.dropLive()
	fn()
}

// newTask makes a task under ctx with its cancellation flag wired up.
func (rt *Runtime) newTask(parent context.Context) *Task {
	ctx, cancel := context.WithCancel(parent)
	t := &Task{rt: rt, ctx: ctx, cancel: cancel}
	context.AfterFunc(ctx, func() { t.cancelled.Store(true) })
	return t
}

// rootTask makes the entry task: it lives on the root context itself, so
// finishing the entry does not cancel the tasks it spawned (05: 진입
// 파일이 끝나면 남은 태스크를 기다린다); only CancelAll ends them.
func (rt *Runtime) rootTask() *Task {
	ctx := rt.root()
	t := &Task{rt: rt, ctx: ctx, cancel: func() {}}
	context.AfterFunc(ctx, func() { t.cancelled.Store(true) })
	return t
}

// done is the task's cancellation channel (nil, never ready, for tasks
// made without a context, such as tests).
func (t *Task) done() <-chan struct{} {
	if t.ctx == nil {
		return nil
	}
	return t.ctx.Done()
}

// isCancelled reports whether this task's context is cancelled.
func (t *Task) isCancelled() bool {
	return t.ctx != nil && t.ctx.Err() != nil
}

// block runs wait with the deadlock accounting (05): if every live task
// would then be waiting and no timer can wake one, it is a deadlock and
// the wait is not entered. timed says a timer (sleep, timeout) will wake
// this task.
func (t *Task) block(timed bool, wait func()) *diag.Error {
	c := &t.rt.conc
	c.mu.Lock()
	c.blocked++
	c.gen++
	if timed {
		c.waking++
	}
	t.rt.armWatchdog()
	c.mu.Unlock()
	wait()
	c.mu.Lock()
	c.blocked--
	c.gen++
	if timed {
		c.waking--
	}
	deadlocked := c.deadlocked
	c.mu.Unlock()
	if deadlocked && t.isCancelled() {
		// Woken by the deadlock report: unwind. Tasks on other contexts
		// (exit handlers) keep going.
		return diag.Errorf("E0702", diag.Pos{})
	}
	return nil
}

// stdout / stderr return the locked writers, wrapping lazily.
func (rt *Runtime) stdout() io.Writer {
	rt.outOnce.Do(func() {
		out, errw := rt.Stdout, rt.Stderr
		if out == nil {
			out = io.Discard
		}
		if errw == nil {
			errw = io.Discard
		}
		rt.out = &lockedWriter{w: out}
		if errw == out {
			rt.errw = rt.out
		} else {
			rt.errw = &lockedWriter{w: errw}
		}
	})
	return rt.out
}

func (rt *Runtime) stderr() io.Writer {
	rt.stdout()
	return rt.errw
}

// Concurrency returns the concurrency natives (05): they live in this
// package because they need the task and runtime internals.
func Concurrency() map[string]value.Value {
	p := map[string]value.Value{}
	add := func(n *value.Native) { p[n.Name] = n }
	add(&value.Native{Name: "sleep", Params: []value.NativeParam{{Name: "d"}}, Fn: sleepFn})
	registerTasks(add)
	registerChannels(add)
	registerAtoms(add)
	return p
}
