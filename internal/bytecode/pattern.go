package bytecode

import (
	"strconv"
	"strings"

	"github.com/tunarider/fung/internal/value"
)

// PatKind classifies a pattern node.
type PatKind int

const (
	PatWild  PatKind = iota // _
	PatBind                 // a name: writes the matched value
	PatLit                  // a literal compared with ==
	PatTag                  // Name(p…)
	PatTuple                // (p, …)
	PatList                 // [p, …] with an optional rest
	PatMap                  // {k: p, …}: other keys are ignored
	PatAlt                  // p | p
)

// Pattern is a pattern tree stored as a constant and interpreted by the
// VM's MATCH instruction (09: 패턴 의미가 Go 함수 하나에 모인다).
type Pattern struct {
	Kind   PatKind
	Lit    value.Value // PatLit: the literal
	Name   string      // PatTag: the tag name
	Global bool        // PatBind: write to a global instead of a slot
	Index  int         // PatBind: slot or global index
	Kids   []*Pattern  // PatTag args, PatTuple/PatList items, PatMap values, PatAlt alternatives
	Keys   []string    // PatMap: keys parallel to Kids
	Rest   *Pattern    // PatList: nil = exact length; PatWild ignores the rest; PatBind binds it
}

// String renders a pattern for disassembly: bound slots print as $n and
// globals as @n.
func (p *Pattern) String() string {
	switch p.Kind {
	case PatWild:
		return "_"
	case PatBind:
		if p.Global {
			return "@" + strconv.Itoa(p.Index)
		}
		return "$" + strconv.Itoa(p.Index)
	case PatLit:
		return value.Format(p.Lit)
	case PatTag:
		return p.Name + "(" + joinPatterns(p.Kids) + ")"
	case PatTuple:
		if len(p.Kids) == 1 {
			return "(" + p.Kids[0].String() + ",)"
		}
		return "(" + joinPatterns(p.Kids) + ")"
	case PatList:
		parts := make([]string, 0, len(p.Kids)+1)
		for _, k := range p.Kids {
			parts = append(parts, k.String())
		}
		if p.Rest != nil {
			if p.Rest.Kind == PatWild {
				parts = append(parts, "...")
			} else {
				parts = append(parts, "..."+p.Rest.String())
			}
		}
		return "[" + strings.Join(parts, ", ") + "]"
	case PatMap:
		parts := make([]string, len(p.Kids))
		for i, k := range p.Kids {
			parts[i] = p.Keys[i] + ": " + k.String()
		}
		return "{" + strings.Join(parts, ", ") + "}"
	case PatAlt:
		parts := make([]string, len(p.Kids))
		for i, k := range p.Kids {
			parts[i] = k.String()
		}
		return strings.Join(parts, " | ")
	}
	return "?"
}

func joinPatterns(ps []*Pattern) string {
	parts := make([]string, len(ps))
	for i, k := range ps {
		parts[i] = k.String()
	}
	return strings.Join(parts, ", ")
}
