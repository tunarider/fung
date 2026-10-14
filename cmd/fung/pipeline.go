package main

import (
	"fmt"
	"github.com/tunarider/fung/extras"
	"io"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/frontend"
	"github.com/tunarider/fung/internal/prelude"
	"github.com/tunarider/fung/internal/value"
	"github.com/tunarider/fung/internal/vm"
)

// reportErrors prints errors with source excerpts; sources maps each
// file name used in positions to its text.
func reportErrors(errs []*diag.Error, sources map[string]string, stderr io.Writer) {
	for _, e := range errs {
		io.WriteString(stderr, diag.Format(e, sources[e.Pos.File]))
	}
}

// execute compiles an entry file with its import graph and runs it,
// mapping errors to exit codes (08): compile failure 2, runtime error 1,
// exit(n) n. at_exit handlers run after the program, even after a
// failure (03); a failing handler is reported and makes the code 1.
func execute(file, src string, scriptArgs []string, stdin io.Reader, stdout, stderr io.Writer) int {
	rt := &vm.Runtime{Stdout: stdout, Stderr: stderr, Stdin: stdin}
	prelude, errs := bootPrelude(rt, scriptArgs)
	if len(errs) > 0 {
		reportErrors(errs, nil, stderr)
		return exitCompileError
	}
	loader := loaderFor(prelude)
	prog, errs := loader.Load(file, src)
	if len(errs) > 0 {
		reportErrors(errs, prog.Sources, stderr)
		return exitCompileError
	}
	register(rt, prog)
	rt.Sources = prog.Sources
	code := exitOK
	_, err := rt.Run(prog.Entry)
	switch {
	case err == nil:
		rt.Wait() // the entry is done; remaining tasks keep the program alive (05)
	default:
		// An entry error (or an exit/deadlock reported elsewhere) ends the
		// program: cancel what is left and do not wait for it (05).
		rt.CancelAll()
		rt.Wait()
		if _, requested := rt.ExitCode(); !requested {
			reportErrors([]*diag.Error{err}, prog.Sources, stderr)
			code = exitRuntimeError
		}
	}
	if n, ok := rt.ExitCode(); ok {
		code = n
	} else if code == exitOK && rt.Failed() {
		code = exitRuntimeError // an un-awaited task failed (05)
	}
	before, hadBefore := rt.ExitCode()
	for _, err := range rt.RunExitHandlers() {
		if _, ok := err.ExitCode(); ok {
			continue
		}
		if err.Code == "E0701" || err.Code == "E0702" {
			continue // a handler cut short by an exit or the deadlock report, already told
		}
		reportErrors([]*diag.Error{err}, prog.Sources, stderr)
		if code == exitOK {
			code = exitRuntimeError
		}
	}
	if n, ok := rt.ExitCode(); ok && (!hadBefore || n != before) {
		code = n // an exit requested inside a handler, or by a task it spawned
	}
	return code
}

// registry assembles the library (06: 네 층): the built-in extras and
// their Go bridges, plus the concurrency bridge the VM provides. This is
// the only place the layers meet.
func registry() frontend.Registry {
	r := extras.Default()
	for k, v := range vm.NativeModules() {
		r.Natives[k] = v
	}
	return r
}

// loaderFor is a loader over the full library for a given prelude.
func loaderFor(prelude map[string]value.Value) *frontend.Loader {
	r := registry()
	return &frontend.Loader{Prelude: prelude, Natives: r.Natives, Stdlib: r.Stdlib}
}

// register makes a program's modules importable on a runtime.
func register(rt *vm.Runtime, prog frontend.Program) {
	for _, m := range prog.Modules {
		if m.Native != nil {
			rt.RegisterNative(m.Key, m.Native)
		} else {
			rt.Register(m.Key, m.Proto)
		}
	}
}

// bootPrelude builds the prelude: the Go natives plus the public names
// of extras/prelude.fung, compiled and run at startup (06: prelude만
// 시작 시). The fung part sees only the natives.
func bootPrelude(rt *vm.Runtime, scriptArgs []string) (map[string]value.Value, []*diag.Error) {
	prelude := prelude.Core(scriptArgs)
	for k, v := range vm.Concurrency() {
		prelude[k] = v
	}
	src, ok := extras.Source("prelude")
	if !ok {
		return prelude, nil
	}
	loader := loaderFor(prelude)
	loader.Internal = true
	prog, errs := loader.Load("extras/prelude.fung", src)
	if len(errs) > 0 {
		return nil, errs
	}
	register(rt, prog)
	rt.Register("prelude", prog.Entry)
	m, err := rt.ImportModule("prelude")
	if err != nil {
		return nil, []*diag.Error{err}
	}
	for _, name := range m.Names() {
		v, _ := m.Get(name)
		prelude[name] = v
	}
	return prelude, nil
}

func stageOf(stage string) frontend.Stage {
	switch stage {
	case "ast":
		return frontend.StageAST
	case "core":
		return frontend.StageCore
	}
	return frontend.StageAll
}

// dump prints one internal stage of a source text.
func dump(stage, file, src string, stdout, stderr io.Writer) int {
	// The same prelude as run, so names resolve identically.
	prelude, errs := bootPrelude(&vm.Runtime{}, nil)
	if len(errs) > 0 {
		reportErrors(errs, nil, stderr)
		return exitCompileError
	}
	b, errs := loaderFor(prelude).Compile(file, src, stageOf(stage))
	if len(errs) > 0 {
		reportErrors(errs, b.Program.Sources, stderr)
		return exitCompileError
	}
	switch stage {
	case "ast":
		fmt.Fprintln(stdout, b.Surface.String())
	case "core":
		fmt.Fprintln(stdout, b.Core.String())
	case "bytecode":
		io.WriteString(stdout, bytecode.Disassemble(b.Proto))
	}
	return exitOK
}
