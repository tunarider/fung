package compiler

import (
	"time"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/tree"
	"github.com/tunarider/fung/internal/value"
)

// buildPattern converts a resolved pattern subtree into the constant
// pattern tree the VM interprets (09. 패턴 매칭).
func buildPattern(n *tree.Node) (*bytecode.Pattern, *diag.Error) {
	switch n.Head {
	case "pwild":
		return &bytecode.Pattern{Kind: bytecode.PatWild}, nil
	case "pbind", "prest":
		if len(n.Kids) == 0 {
			if n.Head == "prest" {
				return &bytecode.Pattern{Kind: bytecode.PatWild}, nil
			}
			return nil, diag.Errorf("E0801", n.Pos, "compiler: unresolved pattern name")
		}
		ref := n.Kids[0]
		return &bytecode.Pattern{Kind: bytecode.PatBind, Global: ref.Head == "global", Index: int(ref.Lit.(int64))}, nil
	case "plit":
		var lit value.Value
		switch v := n.Lit.(type) {
		case int64:
			lit = value.Int(v)
		case float64:
			lit = value.Float(v)
		case string:
			lit = value.Str(v)
		case bool:
			lit = value.Bool(v)
		case time.Duration:
			lit = value.Dur(v)
		default:
			return nil, diag.Errorf("E0801", n.Pos, "compiler: unknown pattern literal")
		}
		return &bytecode.Pattern{Kind: bytecode.PatLit, Lit: lit}, nil
	case "pnil":
		return &bytecode.Pattern{Kind: bytecode.PatLit, Lit: value.Nil{}}, nil
	case "psym":
		return &bytecode.Pattern{Kind: bytecode.PatLit, Lit: value.Intern(n.Lit.(string))}, nil
	case "ptag":
		p := &bytecode.Pattern{Kind: bytecode.PatTag, Name: n.Lit.(string)}
		return buildKids(p, n.Kids)
	case "ptuple":
		return buildKids(&bytecode.Pattern{Kind: bytecode.PatTuple}, n.Kids)
	case "plist":
		p := &bytecode.Pattern{Kind: bytecode.PatList}
		kids := n.Kids
		if len(kids) > 0 && kids[len(kids)-1].Head == "prest" {
			rest, err := buildPattern(kids[len(kids)-1])
			if err != nil {
				return nil, err
			}
			p.Rest = rest
			kids = kids[:len(kids)-1]
		}
		return buildKids(p, kids)
	case "pmap":
		p := &bytecode.Pattern{Kind: bytecode.PatMap}
		for _, entry := range n.Kids {
			k, err := buildPattern(entry.Kids[0])
			if err != nil {
				return nil, err
			}
			p.Keys = append(p.Keys, entry.Lit.(string))
			p.Kids = append(p.Kids, k)
		}
		return p, nil
	case "palt":
		return buildKids(&bytecode.Pattern{Kind: bytecode.PatAlt}, n.Kids)
	}
	return nil, diag.Errorf("E0801", n.Pos, "compiler: unknown pattern "+n.Head)
}

func buildKids(p *bytecode.Pattern, kids []*tree.Node) (*bytecode.Pattern, *diag.Error) {
	for _, k := range kids {
		kp, err := buildPattern(k)
		if err != nil {
			return nil, err
		}
		p.Kids = append(p.Kids, kp)
	}
	return p, nil
}

// addPattern registers a pattern constant and returns its index.
func (c *compiler) addPattern(n *tree.Node) (int, *diag.Error) {
	p, err := buildPattern(n)
	if err != nil {
		return 0, err
	}
	c.proto.Patterns = append(c.proto.Patterns, p)
	return len(c.proto.Patterns) - 1, nil
}
