// Package frontend runs the compile pipeline — lex, parse, desugar,
// resolve, compile — in one place, so the command, the REPL (M6), module
// loading (M5) and tests all share it.
package frontend

import (
	"sort"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

// Stage says how far Compile should go.
type Stage int

const (
	StageAST  Stage = iota // lex and parse only
	StageCore              // through desugar and resolve
	StageAll               // through the compiler
)

// Build holds whatever stages ran. Fields past the requested stage, or
// past the first failing stage, are nil. Program lists the modules the
// entry imports (loaded in full whatever the stage).
type Build struct {
	Surface *tree.Node
	Core    *tree.Node
	Proto   *bytecode.Proto
	Program Program
}

// Registry is a library a program may import from: the fung sources of
// named modules and the hidden `_native.*` bridges they build on (06).
// The core defines the shape; the built-in extras and the command fill
// it in.
type Registry struct {
	Stdlib  func(name string) (string, bool)
	Natives map[string]*value.Module
}

// Compile runs the front end on one entry file with no library: only
// the prelude and files imported by path. Syntax errors stop at the
// first; name errors are all collected (08).
func Compile(file, src string, prelude map[string]value.Value, upTo Stage) (Build, []*diag.Error) {
	l := &Loader{Prelude: prelude}
	return l.Compile(file, src, upTo)
}

// Compile runs the front end with this loader's library up to a stage.
func (l *Loader) Compile(file, src string, upTo Stage) (Build, []*diag.Error) {
	return l.load(file, src, upTo)
}

// preludeNames lists the prelude in a stable order for resolve.
func preludeNames(prelude map[string]value.Value) []string {
	names := make([]string, 0, len(prelude))
	for n := range prelude {
		names = append(names, n)
	}
	sort.Strings(names)
	return names
}
