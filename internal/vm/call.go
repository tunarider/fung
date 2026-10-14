package vm

import (
	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// maxFrames bounds non-tail recursion (09: 깊이 한도 100,000).
const maxFrames = 100000

type namedArg struct {
	name  value.Sym
	value value.Value
}

// argSpec is one parameter as bindArgs sees it, for fung functions
// (bytecode.Param: defaults come from the prologue) and natives
// (value.NativeParam: defaults are constants filled here).
type argSpec struct {
	name   string
	rest   bool
	hasDef bool
	def    value.Value // natives only
}

// paramList is a function's parameters seen as argSpecs, read in place:
// building an []argSpec on every call was a heap allocation per call.
type paramList struct {
	proto  []bytecode.Param
	native []value.NativeParam
}

func (l paramList) len() int {
	if l.proto != nil {
		return len(l.proto)
	}
	return len(l.native)
}

func (l paramList) at(i int) argSpec {
	if l.proto != nil {
		p := l.proto[i]
		return argSpec{name: p.Name, rest: p.Rest, hasDef: p.HasDefault}
	}
	p := l.native[i]
	return argSpec{name: p.Name, rest: p.Rest, hasDef: p.Default != nil, def: p.Default}
}

// bindArgs lays out a call's arguments in a fresh slot array (09. 호출
// 규약): positionals in order, named by lookup, a trailing rest parameter
// collecting the surplus as a list (03). A slot left as Go nil means
// "use the default" (a fung prologue fills it; a native default is
// filled here); a required slot left empty is E0407. Natives share this
// one function with fung functions (06: 같은 호출 규약).
func bindArgs(name string, specs paramList, numSlots int, positional []value.Value, named []namedArg) ([]value.Value, *diag.Error) {
	fixed := specs.len()
	rest := -1
	if fixed > 0 && specs.at(fixed-1).rest {
		fixed--
		rest = fixed
	}
	if len(positional) > fixed && rest < 0 {
		return nil, diag.Errorf("E0402", diag.Pos{}, name, fixed, len(positional))
	}
	slots := make([]value.Value, numSlots)
	if len(positional) > fixed {
		copy(slots, positional[:fixed])
		slots[rest] = value.NewList(append([]value.Value(nil), positional[fixed:]...))
	} else {
		copy(slots, positional)
	}
	if rest >= 0 && slots[rest] == nil {
		slots[rest] = value.NewList(nil)
	}
	for _, na := range named {
		idx := -1
		for i := 0; i < fixed; i++ {
			if specs.at(i).name == na.name.Name() {
				idx = i
				break
			}
		}
		if idx < 0 {
			return nil, diag.Errorf("E0406", diag.Pos{}, name, na.name.Name())
		}
		if slots[idx] != nil {
			return nil, diag.Errorf("E0405", diag.Pos{}, na.name.Name())
		}
		slots[idx] = na.value
	}
	for i := 0; i < fixed; i++ {
		if slots[i] != nil {
			continue
		}
		spec := specs.at(i)
		if !spec.hasDef {
			return nil, diag.Errorf("E0407", diag.Pos{}, spec.name)
		}
		if spec.def != nil {
			slots[i] = spec.def
		}
	}
	return slots, nil
}

// splitCall pops a call's operands off the stack: the callee, positional
// arguments, and (name, value) pairs for named arguments. The argument
// slices are the task's reusable buffers: enter copies them onward (into
// the callee's frame or a native's fresh slot array) before anything can
// make another call, so they are free again by then.
func (t *Task) splitCall(arg int) (value.Value, []value.Value, []namedArg) {
	npos, nnamed := bytecode.DecodeCall(arg)
	top := len(t.stack)
	namedStart := top - 2*nnamed
	named := t.namedBuf[:0]
	for i := 0; i < nnamed; i++ {
		named = append(named, namedArg{name: t.stack[namedStart+2*i].(value.Sym), value: t.stack[namedStart+2*i+1]})
	}
	t.namedBuf = named
	posStart := namedStart - npos
	positional := append(t.argBuf[:0], t.stack[posStart:namedStart]...)
	t.argBuf = positional
	callee := t.stack[posStart-1]
	t.truncate(posStart - 1)
	return callee, positional, named
}

// enter starts running callee. A native runs immediately and its result
// comes back with done=true; a closure gets a new frame (or reuses the
// current one when tail is set) and done=false.
func (t *Task) enter(callee value.Value, positional []value.Value, named []namedArg, pos diag.Pos, tail bool) (result value.Value, done bool, err *diag.Error) {
	if t.cancelled.Load() {
		// Cooperative cancellation (05): checked on every call.
		return nil, true, diag.Errorf("E0701", pos)
	}
	switch fn := callee.(type) {
	case *value.Native:
		slots, e := bindArgs(fn.Name, paramList{native: fn.Params}, len(fn.Params), positional, named)
		if e != nil {
			return nil, true, e.WithPos(pos)
		}
		r, e2 := fn.Fn(t, slots)
		if e2 != nil {
			if d, ok := e2.(*diag.Error); ok {
				if d == nil {
					// A typed nil error is a bug in the native, not a failure.
					return nil, true, diag.Errorf("E0801", pos, "native `"+fn.Name+"` returned a nil error value")
				}
				if code, isExit := d.ExitCode(); isExit {
					// exit(n) ends the whole program from any task (05).
					t.rt.requestExit(code)
				}
				return nil, true, d.WithPos(pos)
			}
			return nil, true, diag.Errorf("E0801", pos, e2)
		}
		return r, true, nil
	case *bytecode.Closure:
		name := fn.Proto.Name
		if name == "" {
			name = "the function"
		}
		slots := positional
		if !exactPositional(fn.Proto, len(positional), len(named)) {
			// Defaults, a rest parameter, named arguments or an arity
			// error: lay the slots out the general way.
			s, e := bindArgs(name, paramList{proto: fn.Proto.Params}, fn.Proto.NumLocals, positional, named)
			if e != nil {
				return nil, true, e.WithPos(pos)
			}
			slots = s
		}
		if tail {
			f := &t.frames[len(t.frames)-1]
			t.truncate(f.base)
			t.pushFrameSlots(slots, fn.Proto.NumLocals)
			f.proto, f.closure, f.globals, f.ip = fn.Proto, fn, fn.Globals, 0
			f.tailCalls++
			return nil, false, nil
		}
		if len(t.frames) >= maxFrames {
			return nil, true, diag.Errorf("E0804", pos, maxFrames)
		}
		t.frames = append(t.frames, frame{proto: fn.Proto, closure: fn, base: len(t.stack), globals: fn.Globals})
		t.pushFrameSlots(slots, fn.Proto.NumLocals)
		return nil, false, nil
	}
	return nil, true, diag.Errorf("E0401", pos, callee.TypeName())
}

// exactPositional reports whether a call fills every parameter of p by
// position and nothing else: the arguments are then the first slots of
// the frame as they are, with no binding work (09: 호출 규약, fast path).
func exactPositional(p *bytecode.Proto, npos, nnamed int) bool {
	n := len(p.Params)
	return nnamed == 0 && npos == n && (n == 0 || !p.Params[n-1].Rest)
}

// pushFrameSlots appends a frame's slots: the given leading values, then
// unassigned (Go nil) slots up to numLocals.
func (t *Task) pushFrameSlots(lead []value.Value, numLocals int) {
	t.stack = append(t.stack, lead...)
	for i := len(lead); i < numLocals; i++ {
		t.stack = append(t.stack, nil)
	}
}

// ret pops the current frame, leaving its result on the caller's stack.
// It reports stop=true when the frame count has dropped to stopDepth.
func (t *Task) ret(stopDepth int) (value.Value, bool) {
	v := t.pop()
	f := t.frames[len(t.frames)-1]
	t.truncate(f.base)
	t.frames = t.frames[:len(t.frames)-1]
	if len(t.frames) == stopDepth {
		return v, true
	}
	t.push(v)
	return nil, false
}

// Call runs a fung function from Go (natives such as at_exit handlers
// and, later, map/filter). It returns when that function's frame returns.
func (t *Task) Call(fn value.Value, args []value.Value) (value.Value, error) {
	v, err := t.guarded(func() (value.Value, *diag.Error) {
		depth := len(t.frames)
		r, done, err := t.enter(fn, args, nil, diag.Pos{}, false)
		if err != nil || done {
			return r, err
		}
		return t.run(depth)
	})
	if err != nil {
		return nil, err
	}
	return v, nil
}

// AtExit registers an exit handler (03).
func (t *Task) AtExit(fn value.Value) {
	t.rt.conc.mu.Lock()
	t.rt.exitHandlers = append(t.rt.exitHandlers, fn)
	t.rt.conc.mu.Unlock()
}

// maxExitHandlers bounds a handler that keeps registering handlers.
const maxExitHandlers = 10000

// RunExitHandlers runs the at_exit handlers in reverse registration order
// on fresh tasks and returns every error they raised; one failing handler
// does not stop the others. Handlers registered while exiting run too
// (like C's atexit), up to a limit.
func (rt *Runtime) RunExitHandlers() []*diag.Error {
	var errs []*diag.Error
	for ran := 0; ; ran++ {
		rt.conc.mu.Lock()
		n := len(rt.exitHandlers)
		if n == 0 {
			rt.conc.mu.Unlock()
			break
		}
		h := rt.exitHandlers[n-1]
		rt.exitHandlers = rt.exitHandlers[:n-1]
		rt.conc.mu.Unlock()
		if ran >= maxExitHandlers {
			errs = append(errs, diag.Errorf("E0806", diag.Pos{}, "at_exit handlers", maxExitHandlers))
			break
		}
		// Handlers run after exit or cancellation, on their own context
		// (05); they count as live for the deadlock detector.
		t := rt.newTask(rt.handlerContext())
		rt.runLive(func() {
			if _, err := t.Call(h, nil); err != nil {
				errs = append(errs, err.(*diag.Error))
			}
		})
		t.cancel()
	}
	return errs
}

// callPath lists the frames innermost first for error reports (08).
func (t *Task) callPath() []diag.CallSite {
	path := make([]diag.CallSite, 0, len(t.frames))
	for i := len(t.frames) - 1; i >= 0; i-- {
		f := t.frames[i]
		pos := diag.Pos{}
		if f.ip-1 >= 0 && f.ip-1 < len(f.proto.Pos) {
			pos = f.proto.Pos[f.ip-1]
		}
		name := f.proto.Name
		if name == "" {
			name = "<lambda>"
		}
		path = append(path, diag.CallSite{File: pos.File, Line: pos.Line, Fn: name, Elided: f.tailCalls})
	}
	return path
}
