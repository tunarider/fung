// Package vm executes bytecode. All state lives in a Task (one per fung
// task, M7) so nothing is global; the Runtime holds what tasks share.
package vm

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"os"
	"runtime/debug"
	"strings"
	"sync"
	"sync/atomic"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

// Runtime is what every task shares (09): the streams and the module
// table. Each module owns its globals (04); frames and closures carry the
// table of the module they belong to.
type Runtime struct {
	Stdout       io.Writer
	Stderr       io.Writer // nil: discard
	Stdin        io.Reader // nil: empty
	stdin        *bufio.Reader
	modules      map[string]*moduleState
	globals      []value.Value // the entry file's globals; they outlive one Run for the REPL (M6)
	replMode     bool          // PrepareREPL was called: the table has a fixed capacity
	tests        []Test
	exitHandlers []value.Value
	Sources      map[string]string // file → text, for excerpts in task failure reports
	outOnce      sync.Once
	out, errw    *lockedWriter
	conc         concState
}

type frame struct {
	proto     *bytecode.Proto
	ip        int
	base      int // index into the task stack where this frame's slots start
	closure   *bytecode.Closure
	globals   []value.Value // the globals of the module this code belongs to
	tailCalls int           // how many tail calls reused this frame (shown in call paths)
}

// Task is one thread of fung execution: an operand stack (whose first
// NumLocals cells are the main frame's slots) and call frames. An
// unassigned slot or global is a Go nil, distinct from fung's nil value.
type Task struct {
	rt        *Runtime
	stack     []value.Value
	frames    []frame
	ctx       context.Context    // cancellation scope (05); nil in tests that never cancel
	cancel    context.CancelFunc // cancels this task and its children
	cancelled atomic.Bool        // set when ctx ends; enter() reads it on every call
	handle    *TaskHandle        // the handle spawn gave out, nil for the entry

	argBuf   []value.Value // splitCall's reusable argument buffers
	namedBuf []namedArg
}

// Stdout implements value.Context for natives: one Write at a time (05).
func (t *Task) Stdout() io.Writer { return t.rt.stdout() }

// Stderr implements value.Context: eprint writes here.
func (t *Task) Stderr() io.Writer { return t.rt.stderr() }

// Done implements value.Context: the task's cancellation channel, so a
// native blocked in Go (an HTTP server or request) can stop with it.
func (t *Task) Done() <-chan struct{} { return t.done() }

// RunTask implements value.Context: it runs fn on a fresh child task on
// the calling goroutine (an HTTP handler, 06) and returns its result.
// The child counts as live for the deadlock detector and inherits this
// task's cancellation.
func (t *Task) RunTask(fn value.Value, args []value.Value) (value.Value, error) {
	child := t.rt.newTask(t.ctxOrRoot())
	defer child.cancel()
	var v value.Value
	var err error
	t.rt.runLive(func() { v, err = child.Call(fn, args) })
	return v, err
}

// ReadLine implements value.Context: one line of standard input, io.EOF
// at the end. One buffered reader is shared by every task under a mutex,
// so concurrent readers get whole lines.
func (t *Task) ReadLine() (string, error) {
	rt := t.rt
	rt.conc.stdinOnce.Do(func() {
		in := rt.Stdin
		if in == nil {
			in = strings.NewReader("")
		}
		rt.stdin = bufio.NewReader(in)
	})
	rt.conc.stdinMu.Lock()
	defer rt.conc.stdinMu.Unlock()
	line, err := rt.stdin.ReadString('\n')
	if err != nil && line == "" {
		return "", io.EOF
	}
	return line, nil
}

// Run executes a proto on a fresh task under the root context and
// returns the value it returned. The task counts as live (05) while it
// runs.
func (rt *Runtime) Run(p *bytecode.Proto) (value.Value, *diag.Error) {
	t := rt.rootTask()
	rt.addLive()
	defer rt.dropLive()
	if n := len(p.GlobalNames); n > len(rt.globals) {
		if rt.replMode && n > cap(rt.globals) {
			return nil, diag.Errorf("E0806", diag.Pos{}, "globals", maxReplGlobals)
		}
		rt.globals = append(rt.globals, make([]value.Value, n-len(rt.globals))...)
	}
	t.stack = make([]value.Value, p.NumLocals, p.NumLocals+16)
	t.frames = append(t.frames, frame{proto: p, base: 0, globals: rt.globals})
	return t.guarded(func() (value.Value, *diag.Error) { return t.run(0) })
}

// guarded runs fn with the task's panic fence: Go panics (bugs in natives
// or the VM) must never reach the user as a Go stack trace (00, 09). It
// wraps every entry from Go into fung code: Run and Call.
func (t *Task) guarded(fn func() (value.Value, *diag.Error)) (v value.Value, err *diag.Error) {
	defer func() {
		if r := recover(); r != nil {
			if os.Getenv("FUNG_DEBUG_PANIC") != "" {
				// Development aid: the Go stack of a fenced panic.
				fmt.Fprintf(os.Stderr, "panic: %v\n%s", r, debug.Stack())
			}
			pos := diag.Pos{}
			if len(t.frames) > 0 {
				f := t.frames[len(t.frames)-1]
				if f.ip-1 >= 0 && f.ip-1 < len(f.proto.Pos) {
					pos = f.proto.Pos[f.ip-1]
				}
			}
			v, err = nil, diag.Errorf("E0801", pos, fmt.Sprint(r)).WithCallPath(t.callPath())
		}
	}()
	return fn()
}

// run executes until the frame count drops to stopDepth and attaches the
// call path to any error (innermost run loop first; outer loops keep it).
func (t *Task) run(stopDepth int) (value.Value, *diag.Error) {
	v, err := t.loop(stopDepth)
	if err != nil {
		t.attach(err)
	}
	return v, err
}

// attach records the call path once (innermost run loop first) and, when
// the error arose inside an embedded extras file, moves its position to
// the nearest frame of user code: the user cannot open extras/*.fung,
// and the extras frames stay visible in the call path (06).
func (t *Task) attach(err *diag.Error) {
	if len(err.CallPath) > 0 {
		return
	}
	err.WithCallPath(t.callPath())
	if n := len(t.frames); n > 0 && t.frames[n-1].proto.Builtin {
		for i := n - 2; i >= 0; i-- {
			f := t.frames[i]
			if !f.proto.Builtin && f.ip-1 >= 0 && f.ip-1 < len(f.proto.Pos) {
				err.Pos = f.proto.Pos[f.ip-1]
				return
			}
		}
	}
}

func (t *Task) push(v value.Value) { t.stack = append(t.stack, v) }

func (t *Task) pop() value.Value {
	n := len(t.stack) - 1
	v := t.stack[n]
	t.stack[n] = nil // a stale slot would keep the value alive (05: dropped task handles)
	t.stack = t.stack[:n]
	return v
}

// truncate shrinks the stack to n, clearing the vacated slots so the GC
// can collect what they held.
func (t *Task) truncate(n int) {
	clear(t.stack[n:])
	t.stack = t.stack[:n]
}

type binop func(a, b value.Value) (value.Value, *diag.Error)

// arith holds the binary arithmetic ops, OpAdd through OpPow in opcode
// order. The loop checks that range before its switch, so an array index
// replaces what used to be a Go map lookup on every instruction.
var arith = [...]binop{
	bytecode.OpAdd - bytecode.OpAdd: value.Add, bytecode.OpSub - bytecode.OpAdd: value.Sub,
	bytecode.OpMul - bytecode.OpAdd: value.Mul, bytecode.OpDiv - bytecode.OpAdd: value.Div,
	bytecode.OpFloorDiv - bytecode.OpAdd: value.FloorDiv, bytecode.OpMod - bytecode.OpAdd: value.Mod,
	bytecode.OpPow - bytecode.OpAdd: value.Pow,
}

func (t *Task) loop(stopDepth int) (value.Value, *diag.Error) {
	for {
		// Re-read the frame every instruction: calls and returns change it.
		f := &t.frames[len(t.frames)-1]
		in := f.proto.Code[f.ip]
		pos := f.proto.Pos[f.ip]
		f.ip++
		op := in.Op()

		if op >= bytecode.OpAdd && op <= bytecode.OpPow {
			b, a := t.pop(), t.pop()
			r, err := arith[op-bytecode.OpAdd](a, b)
			if err != nil {
				return nil, err.WithPos(pos)
			}
			t.push(r)
			continue
		}

		switch op {
		case bytecode.OpConst:
			t.push(f.proto.Consts[in.Arg()])
		case bytecode.OpPop:
			t.pop()
		case bytecode.OpNeg:
			r, err := value.Neg(t.pop())
			if err != nil {
				return nil, err.WithPos(pos)
			}
			t.push(r)
		case bytecode.OpNot:
			r, err := value.Not(t.pop())
			if err != nil {
				return nil, err.WithPos(pos)
			}
			t.push(r)
		case bytecode.OpEq, bytecode.OpNe:
			b, a := t.pop(), t.pop()
			eq := value.Equal(a, b)
			t.push(value.Bool(eq == (op == bytecode.OpEq)))
		case bytecode.OpLt, bytecode.OpGt, bytecode.OpLe, bytecode.OpGe:
			b, a := t.pop(), t.pop()
			c, err := value.Compare(a, b)
			if err != nil {
				return nil, err.WithPos(pos)
			}
			var r bool
			switch op {
			case bytecode.OpLt:
				r = c < 0
			case bytecode.OpGt:
				r = c > 0
			case bytecode.OpLe:
				r = c <= 0
			case bytecode.OpGe:
				r = c >= 0
			}
			t.push(value.Bool(r))
		case bytecode.OpCheckBool:
			top := t.stack[len(t.stack)-1]
			if _, ok := top.(value.Bool); !ok {
				return nil, diag.Errorf("E0505", pos, top.TypeName())
			}
		case bytecode.OpJump:
			f.ip = in.Arg()
		case bytecode.OpJumpIfFalse:
			v := t.pop()
			c, ok := v.(value.Bool)
			if !ok {
				return nil, diag.Errorf("E0505", pos, v.TypeName())
			}
			if !c {
				f.ip = in.Arg()
			}
		case bytecode.OpCall, bytecode.OpTailCall:
			callee, positional, named := t.splitCall(in.Arg())
			r, done, err := t.enter(callee, positional, named, pos, op == bytecode.OpTailCall)
			if err != nil {
				return nil, err
			}
			if done {
				t.push(r)
				if op == bytecode.OpTailCall {
					if v, stop := t.ret(stopDepth); stop {
						return v, nil
					}
				}
			}
		case bytecode.OpClosure:
			child := f.proto.Protos[in.Arg()]
			captured := make([]value.Value, len(child.Captures))
			for i, c := range child.Captures {
				switch c.Kind {
				case tree.CaptureLocal:
					captured[i] = t.stack[f.base+c.Index]
				case tree.CaptureCaptured:
					captured[i] = f.closure.Captured[c.Index]
				case tree.CaptureSelf:
					captured[i] = f.closure
				case tree.CaptureGlobal:
					captured[i] = f.globals[c.Index]
				}
			}
			t.push(&bytecode.Closure{Proto: child, Captured: captured, Globals: f.globals})
		case bytecode.OpLoadCaptured:
			t.push(f.closure.Captured[in.Arg()])
		case bytecode.OpLoadSelf:
			t.push(f.closure)
		case bytecode.OpJumpIfSet:
			if t.pop() != nil {
				f.ip = in.Arg()
			}
		case bytecode.OpCheckType:
			slot, ti := in.Arg()&0xFFF, in.Arg()>>12
			v := t.stack[f.base+slot]
			if !value.MatchesType(v, f.proto.Types[ti]) {
				return nil, diag.Errorf("E0403", pos, f.proto.Params[slot].Name, value.TypeText(f.proto.Types[ti]), v.TypeName())
			}
		case bytecode.OpCheckReturn:
			v := t.stack[len(t.stack)-1]
			if !value.MatchesType(v, f.proto.Types[in.Arg()]) {
				return nil, diag.Errorf("E0404", pos, value.TypeText(f.proto.Types[in.Arg()]), v.TypeName())
			}
		case bytecode.OpLoadLocal:
			t.push(t.stack[f.base+in.Arg()])
		case bytecode.OpStoreLocal:
			t.stack[f.base+in.Arg()] = t.pop()
		case bytecode.OpLoadGlobal:
			v := f.globals[in.Arg()]
			if v == nil {
				name := f.proto.GlobalNames[in.Arg()]
				if t.rt.replMode {
					// A REPL line declared the name, then failed before storing it (08).
					return nil, diag.Errorf("E0808", pos, name).Span(len([]rune(name)))
				}
				// Unreachable since model C: resolve binds top-down and
				// functions copy variables, so this is a compiler bug.
				return nil, diag.Errorf("E0801", pos, "global `"+name+"` read before assignment")
			}
			t.push(v)
		case bytecode.OpStoreGlobal:
			f.globals[in.Arg()] = t.pop()
		case bytecode.OpImport:
			key := string(f.proto.Consts[in.Arg()].(value.Str))
			m, err := t.importModule(key, pos)
			if err != nil {
				return nil, err
			}
			t.push(m)
		case bytecode.OpConcat:
			n := in.Arg()
			var b strings.Builder
			for _, part := range t.stack[len(t.stack)-n:] {
				b.WriteString(value.FormatForPrint(part))
			}
			t.truncate(len(t.stack) - n)
			t.push(value.Str(b.String()))
		case bytecode.OpReturn:
			if v, stop := t.ret(stopDepth); stop {
				return v, nil
			}
		default:
			handled, err := t.execM4(f, op, in, pos)
			if err != nil {
				return nil, err
			}
			if !handled {
				handled, err = t.execM6(f, op, in, pos)
				if err != nil {
					return nil, err
				}
			}
			if !handled {
				return nil, diag.Errorf("E0801", pos, fmt.Sprintf("unknown opcode %v", op))
			}
		}
	}
}
