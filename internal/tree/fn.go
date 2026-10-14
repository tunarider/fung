package tree

import (
	"strconv"
	"strings"

	"github.com/tunarider/fung/internal/diag"
)

// Param is one declared parameter of a function.
type Param struct {
	Name       string
	Pos        diag.Pos
	Type       []string // union alternatives, nil = unchecked
	HasDefault bool
	Rest       bool  // ...name: collects surplus positional arguments
	Pattern    *Node // a destructuring parameter (03): desugar gives it a hidden name
}

// CaptureKind says where a closure copies a captured value from when it
// is created (09: 클로저는 값을 복사한다).
type CaptureKind int

const (
	CaptureLocal    CaptureKind = iota // the creating frame's slot Index
	CaptureCaptured                    // the creating closure's Captured[Index]
	CaptureSelf                        // the creating frame's own closure
	CaptureGlobal                      // the file variable globals[Index], as it is right now
)

// Capture is one value a function copies from its enclosing function.
type Capture struct {
	Kind  CaptureKind
	Index int
}

// Import is the Lit of an (import …) node (04).
type Import struct {
	Spec    string     // "fs" or "./lib/rename"
	IsPath  bool       // written as a string: a file path
	Alias   string     // after `as`, "" if none
	Names   []string   // the (a, b) list, nil if none
	NamePos []diag.Pos // one per Names entry, for errors
}

func (im *Import) String() string {
	var b strings.Builder
	if im.IsPath {
		b.WriteString(strconv.Quote(im.Spec))
	} else {
		b.WriteString(im.Spec)
	}
	if im.Alias != "" {
		b.WriteString(" as " + im.Alias)
	}
	if im.Names != nil {
		b.WriteString(" (" + strings.Join(im.Names, ", ") + ")")
	}
	return b.String()
}

// TypeAlias is the Lit of a (typealias …) node (07).
type TypeAlias struct {
	Name string
	Alts []string
}

func (a *TypeAlias) String() string { return a.Name }

// FnInfo is the Lit of an (fn …) node. The parser fills the declaration;
// resolve fills NumLocals and Captures.
type FnInfo struct {
	Name      string // "" for lambdas and anonymous fns
	Params    []Param
	Return    []string // nil = unchecked
	ForBody   bool     // the synthetic lambda of a `for`: return inside is E0213
	NumLocals int
	Captures  []Capture
}

// String renders the signature for dumps: add(a, b=?)->int, (x) for lambdas.
func (f *FnInfo) String() string {
	var b strings.Builder
	b.WriteString(f.Name)
	b.WriteByte('(')
	for i, p := range f.Params {
		if i > 0 {
			b.WriteString(", ")
		}
		if p.Rest {
			b.WriteString("...")
		}
		if p.Pattern != nil {
			b.WriteString(p.Pattern.String())
		}
		b.WriteString(p.Name)
		if p.Type != nil {
			b.WriteString(":" + strings.Join(p.Type, "|"))
		}
		if p.HasDefault {
			b.WriteString("=?")
		}
	}
	b.WriteByte(')')
	if f.Return != nil {
		b.WriteString("->" + strings.Join(f.Return, "|"))
	}
	return b.String()
}
