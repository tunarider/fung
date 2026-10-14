package resolve

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

// binding says where a name lives.
type binding struct {
	global   bool
	index    int         // global index or local slot
	constant bool        // a hoisted top-level fn or an import: cannot be reassigned
	module   *ModuleInfo // an imported module name: cannot be shadowed either (04)
}

// scope is one lexical block: the file (globals) or a block (slots).
type scope struct {
	names map[string]binding
	file  bool
	base  int // first slot this block may use; restored on exit
	// For warnings (08): where each name was bound, which were read, and
	// which are parameters (never reported as unused).
	pos   map[string]diag.Pos
	used  map[string]bool
	param map[string]bool
}

func newScope(file bool, base int) *scope {
	return &scope{names: map[string]binding{}, file: file, base: base,
		pos: map[string]diag.Pos{}, used: map[string]bool{}, param: map[string]bool{}}
}

// fnScope is one function being resolved: its block scopes, slot counter,
// the values it captures from enclosing functions, and its own name when
// it is a named local fn (reachable as (self)).
type fnScope struct {
	parent   *fnScope
	blocks   []*scope
	nextSlot int
	maxSlot  int
	captures []tree.Capture
	capIndex map[string]int
	self     string
	forBody  bool              // the synthetic lambda of a `for`: return inside is E0213
	file     bool              // the root pseudo-function whose first block holds globals
	closed   map[string]bool   // names defined in blocks of this fn that have ended (E0206)
	later    []map[string]bool // per block: local fn names declared later in that block (E0208)
}
