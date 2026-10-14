package parser

import (
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/tree"
)

// listLiteral parses `[e, …]`; newlines inside are whitespace.
func (p *parser) listLiteral() (*tree.Node, *diag.Error) {
	open := p.next() // [
	p.nest++
	defer func() { p.nest-- }()
	n := tree.New("list", open.Pos)
	for !p.at(lexer.RBracket) {
		e, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		n.Kids = append(n.Kids, e)
		if p.at(lexer.Comma) {
			p.next()
		} else if !p.at(lexer.RBracket) {
			return nil, p.unexpected()
		}
	}
	p.next()
	return n, nil
}

// mapLiteral parses `{key: e, …}`; a key is a name or a plain string (02).
func (p *parser) mapLiteral() (*tree.Node, *diag.Error) {
	open := p.next() // {
	p.nest++
	defer func() { p.nest-- }()
	n := tree.New("map", open.Pos)
	seen := map[string]bool{}
	for !p.at(lexer.RBrace) {
		t := p.peek()
		var key string
		switch {
		case p.atNameLike():
			// An identifier-shaped key is a string whatever its case (02);
			// keywords are plain names here (01).
			key = t.Text
		case t.Kind == lexer.String:
			var text strings.Builder
			for _, part := range t.Parts {
				if part.IsCode {
					return nil, diag.Errorf("E0110", t.Pos, "interpolation in a map key")
				}
				text.WriteString(part.Text)
			}
			key = text.String()
		default:
			return nil, diag.Errorf("E0112", t.Pos, "a map key", describe(t))
		}
		if seen[key] {
			return nil, diag.Errorf("E0113", t.Pos, key)
		}
		seen[key] = true
		p.next()
		var v *tree.Node
		if (t.Kind == lexer.Name || t.Kind == lexer.Tag) && (p.at(lexer.Comma) || p.at(lexer.RBrace)) {
			// {a, b} is {a: a, b: b} (02). Keywords and strings have no
			// variable to stand for, so they take no shorthand.
			v = tree.Leaf("name", t.Pos, t.Text)
		} else {
			if _, err := p.expectDesc(lexer.Colon, "`:` after the map key"); err != nil {
				return nil, err
			}
			var err *diag.Error
			if v, err = p.expr(0); err != nil {
				return nil, err
			}
		}
		n.Kids = append(n.Kids, tree.WithLit("entry", t.Pos, key, v))
		if p.at(lexer.Comma) {
			p.next()
		} else if !p.at(lexer.RBrace) {
			return nil, p.unexpected()
		}
	}
	p.next()
	return n, nil
}

// parenOrTuple parses `()`, `(e)`, `(e,)`, `(a, b, …)` (01: 튜플).
func (p *parser) parenOrTuple() (*tree.Node, *diag.Error) {
	open := p.next() // (
	p.nest++
	defer func() { p.nest-- }()
	if p.at(lexer.RParen) {
		p.next()
		return tree.New("tuple", open.Pos), nil
	}
	first, err := p.expr(0)
	if err != nil {
		return nil, err
	}
	if p.at(lexer.RParen) {
		p.next()
		return first, nil
	}
	if !p.at(lexer.Comma) {
		return nil, p.unexpected()
	}
	n := tree.New("tuple", open.Pos, first)
	for p.at(lexer.Comma) {
		p.next()
		if p.at(lexer.RParen) {
			break
		}
		e, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		n.Kids = append(n.Kids, e)
	}
	if _, err := p.expectDesc(lexer.RParen, "`)`"); err != nil {
		return nil, err
	}
	return n, nil
}

// tagExpr parses TAG or TAG(args) (02): a bare tag is its symbol.
func (p *parser) tagExpr() (*tree.Node, *diag.Error) {
	t := p.next()
	if !p.at(lexer.LParen) || p.peek().Pos.Line != t.Pos.Line {
		return tree.Leaf("symbol", t.Pos, t.Text), nil
	}
	n, err := p.callArgs(tree.WithLit("tag", t.Pos, t.Text))
	if err != nil {
		return nil, err
	}
	for _, k := range n.Kids {
		if k.Head == "named" {
			return nil, diag.Errorf("E0110", k.Pos, "named argument in a tag (tags take values by position)")
		}
	}
	return n, nil
}
