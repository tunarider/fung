package vm

import (
	"strings"
	"sync"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// moduleState is one registered module: its compiled body, the globals
// it owns, and its value once the body has run (04: 처음 import될 때 한 번).
type moduleState struct {
	mu      sync.Mutex // one task initialises; others wait (05)
	proto   *bytecode.Proto
	globals []value.Value
	value   *value.Module
	loading bool
}

// Register makes a compiled module importable under key.
func (rt *Runtime) Register(key string, p *bytecode.Proto) {
	if rt.modules == nil {
		rt.modules = map[string]*moduleState{}
	}
	if st, ok := rt.modules[key]; ok && st.value != nil {
		return // already ran: a module's top level runs once (04)
	}
	rt.modules[key] = &moduleState{proto: p}
}

// RegisterNative makes a Go module importable under key.
func (rt *Runtime) RegisterNative(key string, m *value.Module) {
	if rt.modules == nil {
		rt.modules = map[string]*moduleState{}
	}
	rt.modules[key] = &moduleState{value: m}
}

// ImportModule returns a module's value from a fresh task, running its
// top level first if needed (the command uses it for prelude.fung).
func (rt *Runtime) ImportModule(key string) (*value.Module, *diag.Error) {
	t := rt.newTask(rt.root())
	defer t.cancel()
	var m *value.Module
	var err *diag.Error
	rt.runLive(func() {
		_, err = t.guarded(func() (value.Value, *diag.Error) {
			v, err := t.importModule(key, diag.Pos{})
			if err != nil {
				return nil, err
			}
			m = v
			return v, nil
		})
	})
	return m, err
}

// importModule returns a module's value, running its top level first
// when this is the first import. The body runs as a nested frame on this
// task, like a call, with its own globals; afterwards the public globals
// become the immutable module value.
func (t *Task) importModule(key string, pos diag.Pos) (*value.Module, *diag.Error) {
	st, ok := t.rt.modules[key]
	if !ok {
		return nil, diag.Errorf("E0801", pos, "module `"+key+"` was not registered")
	}
	st.mu.Lock()
	defer st.mu.Unlock()
	if st.value != nil {
		return st.value, nil
	}
	if st.loading {
		return nil, diag.Errorf("E0801", pos, "module `"+key+"` imports itself at run time")
	}
	st.loading = true
	st.globals = make([]value.Value, len(st.proto.GlobalNames))
	depth := len(t.frames)
	t.frames = append(t.frames, frame{proto: st.proto, base: len(t.stack), globals: st.globals})
	t.stack = append(t.stack, make([]value.Value, st.proto.NumLocals)...)
	if _, err := t.run(depth); err != nil {
		return nil, err
	}
	m := value.NewModule(moduleName(st.proto), key)
	index := map[string]int{}
	for i, name := range st.proto.GlobalNames {
		index[name] = i
	}
	for _, name := range st.proto.Exports {
		if v := st.globals[index[name]]; v != nil {
			m.Set(name, v)
		}
	}
	st.value, st.loading = m, false
	return m, nil
}

// moduleName strips the "<module …>" frame label the loader gives a
// module body.
func moduleName(p *bytecode.Proto) string {
	return strings.TrimSuffix(strings.TrimPrefix(p.Name, "<module "), ">")
}
