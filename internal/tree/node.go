// Package tree is the uniform tree every front-end stage works on. The
// parser produces surface forms, desugar rewrites them into core forms,
// and the compiler reads core forms. One node shape covers all of them
// ("겉은 Go, 속은 Lisp"): a head word, a position, an optional literal
// payload, and children.
package tree

import (
	"fmt"
	"strings"
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

type Node struct {
	Head string
	Pos  diag.Pos
	Lit  any // int64, float64, bool, string, time.Duration, or nil
	Kids []*Node
	Off  int // byte span in the source, filled by the parser for expressions and statements
	End  int // (End == 0 means unknown)
}

// Text returns the node's source text, or "" when its span is unknown.
func (n *Node) Text(src string) string {
	if n.End == 0 || n.End > len(src) || n.Off > n.End {
		return ""
	}
	return src[n.Off:n.End]
}

func New(head string, pos diag.Pos, kids ...*Node) *Node {
	return &Node{Head: head, Pos: pos, Kids: kids}
}

func Leaf(head string, pos diag.Pos, lit any) *Node {
	return &Node{Head: head, Pos: pos, Lit: lit}
}

func WithLit(head string, pos diag.Pos, lit any, kids ...*Node) *Node {
	return &Node{Head: head, Pos: pos, Lit: lit, Kids: kids}
}

// bareLeaves print as their value alone; other leaves print as (head lit).
var bareLeaves = map[string]bool{
	"int": true, "float": true, "bool": true, "name": true,
	"string": true, "symbol": true, "duration": true,
}

// String renders the node as an S-expression, the format of `fung dump`.
// Literals print exactly as `print` would show them (value.Format), so
// the dump reads back as fung code.
func (n *Node) String() string {
	var b strings.Builder
	n.write(&b)
	return b.String()
}

func (n *Node) write(b *strings.Builder) {
	if len(n.Kids) == 0 && n.Lit != nil {
		lit := n.litString()
		if bareLeaves[n.Head] {
			b.WriteString(lit)
			return
		}
		b.WriteString("(" + n.Head + " " + lit + ")")
		return
	}
	b.WriteString("(" + n.Head)
	if n.Lit != nil {
		b.WriteString(" " + n.litString())
	}
	for _, k := range n.Kids {
		b.WriteByte(' ')
		k.write(b)
	}
	b.WriteByte(')')
}

func (n *Node) litString() string {
	switch v := n.Lit.(type) {
	case *FnInfo:
		return v.String()
	case *TypeAlias:
		return v.String()
	case *Import:
		return v.String()
	case float64:
		return value.Format(value.Float(v))
	case time.Duration:
		return value.Format(value.Dur(v))
	case string:
		switch n.Head {
		case "string", "plit", "import", "test": // a resolved import holds its module key
			return value.Format(value.Str(v))
		case "symbol":
			return "'" + v
		}
		return v
	}
	return fmt.Sprint(n.Lit)
}
