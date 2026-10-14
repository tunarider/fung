package main

import (
	"io"
	"os"
	"path/filepath"

	"github.com/tunarider/fung/internal/repl"
	"github.com/tunarider/fung/internal/vm"
)

// startRepl runs the REPL (08) on the given streams. A terminal gets the
// line editor; a pipe gets plain line reading.
func startRepl(stdin io.Reader, stdout, stderr io.Writer) int {
	rt := &vm.Runtime{Stdout: stdout, Stderr: stderr, Stdin: stdin}
	prelude, errs := bootPrelude(rt, nil)
	if len(errs) > 0 {
		reportErrors(errs, nil, stderr)
		return exitCompileError
	}
	reader := replReader(stdin, stdout)
	return repl.New(reader, stdout, stderr, prelude, registry(), rt).Run()
}

// replReader picks the raw-mode editor for a terminal and plain line
// reading for everything else (pipes, tests, other platforms).
func replReader(stdin io.Reader, stdout io.Writer) repl.LineReader {
	if f, ok := stdin.(*os.File); ok {
		if r, ok := repl.TermReader(f, stdout); ok {
			// The history file lives in the home directory (08); without
			// one, history stays in memory.
			if h, ok := r.(interface{ UseHistoryFile(string) }); ok {
				if home, err := os.UserHomeDir(); err == nil {
					h.UseHistoryFile(filepath.Join(home, ".fung_history"))
				}
			}
			return r
		}
	}
	return repl.PlainReader(stdin, stdout)
}
