// Package repl is the read-eval-print loop (08): each line is compiled
// against the names defined so far and run on one shared runtime, so
// definitions persist and functions may be redefined.
package repl

import (
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/frontend"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
	"github.com/tunarider/fung/internal/resolve"
	"github.com/tunarider/fung/internal/value"
	"github.com/tunarider/fung/internal/vm"
)

// Session is one REPL run.
type Session struct {
	in      LineReader
	out     io.Writer
	err     io.Writer
	prelude map[string]value.Value
	lib     frontend.Registry // what the session may import (06)
	rt      *vm.Runtime
	globals []string // the entry globals defined so far, in index order
}

// New builds a session over a runtime whose streams are already set.
func New(in LineReader, out, errOut io.Writer, prelude map[string]value.Value, lib frontend.Registry, rt *vm.Runtime) *Session {
	return &Session{in: in, out: out, err: errOut, prelude: prelude, lib: lib, rt: rt}
}

const helpText = `commands:
  :help            show this text
  :quit            leave (Ctrl-D does the same)
  :load file.fung  run a file in this session; its definitions stay
`

// Run reads lines until :quit, end of input, or exit(n). It returns the
// exit code: 0 normally, n for exit(n).
func (s *Session) Run() int {
	s.rt.PrepareREPL()
	for {
		line, err := s.in.ReadLine("> ")
		if err != nil {
			io.WriteString(s.out, "\n")
			return s.finish(0)
		}
		if strings.HasPrefix(strings.TrimSpace(line), ":") {
			if stop, code := s.command(strings.TrimSpace(line)); stop {
				return s.finish(code)
			}
			continue
		}
		// Keep reading while the text ends mid-block or mid-expression.
		src := line
		for needsMore(src) {
			more, err := s.in.ReadLine(". ")
			if err != nil {
				io.WriteString(s.out, "\n")
				return s.finish(0)
			}
			src += "\n" + more
		}
		if stop, code := s.eval("<repl>", src); stop {
			return s.finish(code)
		}
	}
}

// finish runs at_exit handlers and returns the code.
func (s *Session) finish(code int) int {
	s.rt.CancelAll() // tasks still running do not keep the session open (05)
	s.rt.Wait()
	for _, err := range s.rt.RunExitHandlers() {
		if _, ok := err.ExitCode(); ok {
			continue
		}
		if err.Code == "E0701" || err.Code == "E0702" {
			continue
		}
		io.WriteString(s.err, diag.Format(err, ""))
	}
	s.rt.Wait()
	s.rt.ReportUnretrieved() // failures no one awaited during the session (05)
	if n, ok := s.rt.ExitCode(); ok {
		code = n
	}
	return code
}

// command handles the `:` commands (08: :help, :quit, :load only).
func (s *Session) command(line string) (stop bool, code int) {
	name, arg, _ := strings.Cut(line, " ")
	switch name {
	case ":help":
		io.WriteString(s.out, helpText)
	case ":quit":
		return true, 0
	case ":load":
		arg = strings.TrimSpace(arg)
		if arg == "" {
			fmt.Fprintln(s.err, ":load needs a file path")
			return false, 0
		}
		src, err := os.ReadFile(arg)
		if err != nil {
			fmt.Fprintf(s.err, "cannot read %s: %v\n", arg, err)
			return false, 0
		}
		return s.eval(arg, string(src))
	default:
		fmt.Fprintf(s.err, "unknown command %s\n", name)
	}
	return false, 0
}

// needsMore reports whether the text so far ends before its syntax is
// complete: the parser failed exactly at the end of input.
func needsMore(src string) bool {
	if strings.TrimSpace(src) == "" {
		return false
	}
	toks, _, err := lexer.Lex("<repl>", src)
	if err != nil {
		return false
	}
	_, perr := parser.Parse(toks)
	if perr == nil {
		return false
	}
	eof := toks[len(toks)-1].Pos
	return perr.Pos == eof
}

// statementHeads are the surface forms whose value is not printed (08).
var statementHeads = map[string]bool{"assign": true, "fn": true, "import": true, "typealias": true, "test": true, "assert": true}

// eval compiles and runs one input against the session's globals and
// prints the value of a final expression.
func (s *Session) eval(name, src string) (stop bool, code int) {
	loader := &frontend.Loader{
		Prelude: s.prelude, Natives: s.lib.Natives, Stdlib: s.lib.Stdlib,
		Resolve: resolve.Options{Globals: s.globals, Redefine: true},
	}
	b, errs := loader.LoadBuild(name, src)
	if len(errs) > 0 {
		for _, e := range errs {
			io.WriteString(s.err, diag.Format(e, b.Program.Sources[e.Pos.File]))
		}
		return false, 0
	}
	for _, m := range b.Program.Modules {
		if m.Native != nil {
			s.rt.RegisterNative(m.Key, m.Native)
		} else {
			s.rt.Register(m.Key, m.Proto)
		}
	}
	s.globals = b.Proto.GlobalNames
	v, err := s.rt.Run(b.Proto)
	if err != nil {
		if n, ok := err.ExitCode(); ok {
			return true, n
		}
		io.WriteString(s.err, diag.Format(err, b.Program.Sources[err.Pos.File]))
		return false, 0
	}
	kids := b.Surface.Kids
	if len(kids) == 0 || statementHeads[kids[len(kids)-1].Head] {
		return false, 0
	}
	fmt.Fprintln(s.out, value.Format(v))
	return false, 0
}
