package resolve

import (
	"sort"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
)

// Warnings (08, W01xx) are collected alongside errors and shown only by
// `fung check` and `fung test`.

// warnShadow reports a new name that hides a prelude function (W0103).
// The rule is the same however a file is loaded; the loader hides the
// embedded stdlib's own warnings.
func (r *resolver) warnShadow(name string, pos diag.Pos) {
	if r.prelude[name] {
		r.warnings = append(r.warnings, diag.Errorf("W0103", pos, name).Span(runeLen(name)))
	}
}

// warnUnused reports the block-local names that were never read (W0102):
// not parameters, not `_`-prefixed, not hidden desugar names.
func (r *resolver) warnUnused(sc *scope) {
	for name := range sc.names {
		if sc.used[name] || sc.param[name] || strings.HasPrefix(name, "_") || strings.HasPrefix(name, "$") {
			continue
		}
		r.warnings = append(r.warnings, diag.Errorf("W0102", sc.pos[name], name).Span(runeLen(name)))
	}
}

// warnUnusedImports reports import-bound names never read in the file (W0101).
func (r *resolver) warnUnusedImports(sc *scope) {
	for _, name := range r.globals {
		if r.importBound[name] && !sc.used[name] {
			r.warnings = append(r.warnings, diag.Errorf("W0101", sc.pos[name], name).Span(runeLen(name)))
		}
	}
}

// sortWarnings orders warnings by position so output is stable.
func sortWarnings(ws []*diag.Error) {
	sort.SliceStable(ws, func(i, j int) bool {
		a, b := ws[i].Pos, ws[j].Pos
		if a.Line != b.Line {
			return a.Line < b.Line
		}
		return a.Col < b.Col
	})
}

// warnBlockValue reports a block in a value position (the right side of
// an assignment, a call argument, a return) whose last statement is an
// assignment: its value is nil, which is almost never what was meant
// (W0104, 08). Only if/match/do blocks are inspected.
func (r *resolver) warnBlockValue(n *tree.Node) {
	switch n.Head {
	case "if", "match", "do":
	default:
		return
	}
	if a := lastAssign(n); a != nil {
		name := "this pattern"
		if t := a.Kids[0]; t.Head == "name" {
			name = t.Lit.(string)
		}
		r.warnings = append(r.warnings, diag.Errorf("W0104", a.Pos, name).Span(runeLen(name)))
	}
}

// lastAssign finds an assignment that would be the last statement of n
// when n is evaluated as a value: the end of a do, of any if branch, or
// of any match arm.
func lastAssign(n *tree.Node) *tree.Node {
	switch n.Head {
	case "let":
		return n
	case "do":
		if len(n.Kids) > 0 {
			return lastAssign(n.Kids[len(n.Kids)-1])
		}
	case "if":
		for _, k := range n.Kids[1:] {
			if a := lastAssign(k); a != nil {
				return a
			}
		}
	case "match":
		for _, arm := range n.Kids[1:] {
			if len(arm.Kids) > 0 {
				if a := lastAssign(arm.Kids[len(arm.Kids)-1]); a != nil {
					return a
				}
			}
		}
	}
	return nil
}
