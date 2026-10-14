package parser

import (
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/tree"
)

// pattern = pat_term { "|" pat_term }   (01 EBNF)
func (p *parser) pattern() (*tree.Node, *diag.Error) {
	first, err := p.patTerm()
	if err != nil {
		return nil, err
	}
	if !p.at(lexer.Bar) {
		return first, nil
	}
	alt := tree.New("palt", first.Pos, first)
	for p.at(lexer.Bar) {
		p.next()
		term, err := p.patTerm()
		if err != nil {
			return nil, err
		}
		alt.Kids = append(alt.Kids, term)
	}
	return alt, nil
}

func (p *parser) patTerm() (*tree.Node, *diag.Error) {
	if p.depth >= maxDepth {
		return nil, diag.Errorf("E0802", p.peek().Pos, maxDepth)
	}
	p.depth++
	defer func() { p.depth-- }()
	t := p.peek()
	switch t.Kind {
	case lexer.Name:
		p.next()
		if t.Text == "_" {
			return tree.New("pwild", t.Pos), nil
		}
		return tree.Leaf("pbind", t.Pos, t.Text), nil
	case lexer.Int:
		p.next()
		return tree.Leaf("plit", t.Pos, t.Int), nil
	case lexer.Float:
		p.next()
		return tree.Leaf("plit", t.Pos, t.Float), nil
	case lexer.Minus:
		p.next()
		n := p.peek()
		switch n.Kind {
		case lexer.Int:
			p.next()
			return tree.Leaf("plit", t.Pos, -n.Int), nil
		case lexer.Float:
			p.next()
			return tree.Leaf("plit", t.Pos, -n.Float), nil
		}
		return nil, p.unexpected()
	case lexer.String:
		p.next()
		var text strings.Builder
		for _, part := range t.Parts {
			if part.IsCode {
				return nil, diag.Errorf("E0110", t.Pos, "interpolation in a pattern")
			}
			text.WriteString(part.Text)
		}
		return tree.Leaf("plit", t.Pos, text.String()), nil
	case lexer.RawString:
		p.next()
		return tree.Leaf("plit", t.Pos, t.Text), nil
	case lexer.Symbol:
		p.next()
		return tree.Leaf("psym", t.Pos, t.Text), nil
	case lexer.Duration:
		p.next()
		return tree.Leaf("plit", t.Pos, t.Dur), nil
	case lexer.KwTrue, lexer.KwFalse:
		p.next()
		return tree.Leaf("plit", t.Pos, t.Kind == lexer.KwTrue), nil
	case lexer.KwNil:
		p.next()
		return tree.New("pnil", t.Pos), nil
	case lexer.Tag:
		p.next()
		if !p.at(lexer.LParen) || p.peek().Pos.Line != t.Pos.Line {
			// A bare tag matches the symbol only (03의 함정).
			return tree.Leaf("psym", t.Pos, t.Text), nil
		}
		p.next()
		return p.patList(tree.WithLit("ptag", t.Pos, t.Text), lexer.RParen)
	case lexer.LParen:
		p.next()
		if p.at(lexer.RParen) {
			p.next()
			return tree.New("ptuple", t.Pos), nil
		}
		first, err := p.pattern()
		if err != nil {
			return nil, err
		}
		if !p.at(lexer.Comma) {
			return nil, diag.Errorf("E0110", p.peek().Pos, "`"+describe(p.peek())+"` (a parenthesized pattern is not a pattern; write (p,) for a tuple)")
		}
		p.next()
		return p.patList(tree.New("ptuple", t.Pos, first), lexer.RParen)
	case lexer.LBracket:
		p.next()
		n := tree.New("plist", t.Pos)
		for !p.at(lexer.RBracket) {
			if p.at(lexer.Ellipsis) {
				p.next()
				rest := tree.New("prest", t.Pos)
				if p.at(lexer.Name) {
					rest.Lit = p.next().Text
				}
				n.Kids = append(n.Kids, rest)
				if p.at(lexer.Comma) {
					p.next()
				}
				if !p.at(lexer.RBracket) {
					return nil, p.unexpected()
				}
				break
			}
			k, err := p.pattern()
			if err != nil {
				return nil, err
			}
			n.Kids = append(n.Kids, k)
			if p.at(lexer.Comma) {
				p.next()
			} else if !p.at(lexer.RBracket) {
				return nil, p.unexpected()
			}
		}
		p.next()
		return n, nil
	case lexer.LBrace:
		p.next()
		n := tree.New("pmap", t.Pos)
		for !p.at(lexer.RBrace) {
			if !p.atNameLike() {
				return nil, diag.Errorf("E0112", p.peek().Pos, "a map key", describe(p.peek()))
			}
			key := p.next()
			var v *tree.Node
			if (key.Kind == lexer.Name || key.Kind == lexer.Tag) && (p.at(lexer.Comma) || p.at(lexer.RBrace)) {
				// {a, b} binds a and b (02); keywords take no shorthand.
				v = tree.Leaf("pbind", key.Pos, key.Text)
			} else {
				if _, err := p.expectDesc(lexer.Colon, "`:`"); err != nil {
					return nil, err
				}
				var err *diag.Error
				if v, err = p.pattern(); err != nil {
					return nil, err
				}
			}
			n.Kids = append(n.Kids, tree.WithLit("pentry", key.Pos, key.Text, v))
			if p.at(lexer.Comma) {
				p.next()
			} else if !p.at(lexer.RBrace) {
				return nil, p.unexpected()
			}
		}
		p.next()
		return n, nil
	}
	return nil, p.unexpected()
}

// patList parses comma-separated patterns into n until the closing token.
func (p *parser) patList(n *tree.Node, closer lexer.Kind) (*tree.Node, *diag.Error) {
	for !p.at(closer) {
		k, err := p.pattern()
		if err != nil {
			return nil, err
		}
		n.Kids = append(n.Kids, k)
		if p.at(lexer.Comma) {
			p.next()
		} else if !p.at(closer) {
			return nil, p.unexpected()
		}
	}
	p.next()
	return n, nil
}

// armHeadAhead reports whether `pattern [when expr] =>` parses at p.i,
// without consuming anything. A nesting-limit error inside the
// speculative parse is fatal (E0802) rather than "not an arm head", so a
// Go stack overflow cannot hide behind a syntax error.
// restoring the position either way (01: 절 판정은 시도로).
func (p *parser) armHeadAhead() (ok bool, fatal *diag.Error) {
	save, saveNest, saveDepth := p.i, p.nest, p.depth
	defer func() { p.i, p.nest, p.depth = save, saveNest, saveDepth }()
	if _, err := p.pattern(); err != nil {
		if err.Code == "E0802" {
			return false, err
		}
		return false, nil
	}
	if p.at(lexer.KwWhen) {
		p.next()
		if _, err := p.guardExpr(); err != nil {
			if err.Code == "E0802" {
				return false, err
			}
			return false, nil
		}
	}
	return p.at(lexer.FatArrow), nil
}

// guardExpr parses a `when` condition with bare lambdas disabled so the
// arm's `=>` is not read as a lambda arrow.
func (p *parser) guardExpr() (*tree.Node, *diag.Error) {
	saved := p.guard
	p.guard = true
	defer func() { p.guard = saved }()
	return p.expr(0)
}

// matchExpr parses `match e: arms ;;`. An arm's body runs until the next
// line that parses as an arm head or the closing `;;` (01).
func (p *parser) matchExpr() (*tree.Node, *diag.Error) {
	t := p.next() // match
	subject, err := p.expr(0)
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.Colon, "`:` after the match subject"); err != nil {
		return nil, err
	}
	saved := p.nest
	p.nest = 0
	defer func() { p.nest = saved }()
	n := tree.New("match", t.Pos, subject)
	p.skipSeps()
	for !p.at(lexer.DoubleSemi) {
		isArm, fatal := p.armHeadAhead()
		if fatal != nil {
			return nil, fatal
		}
		if p.at(lexer.EOF) || !isArm {
			return nil, p.unexpected()
		}
		pat, err := p.pattern()
		if err != nil {
			return nil, err
		}
		guard := tree.New("nil", pat.Pos)
		if p.at(lexer.KwWhen) {
			p.next()
			guard, err = p.guardExpr()
			if err != nil {
				return nil, err
			}
		}
		arrow := p.next() // =>
		body := tree.New("block", arrow.Pos)
		p.skipSeps()
		for !p.at(lexer.DoubleSemi) && !p.at(lexer.EOF) {
			isArm, fatal := p.armHeadAhead()
			if fatal != nil {
				return nil, fatal
			}
			if isArm {
				break
			}
			p.blockDepth++
			stmt, err := p.spannedStatement()
			p.blockDepth--
			if err != nil {
				return nil, err
			}
			body.Kids = append(body.Kids, stmt)
			switch p.peek().Kind {
			case lexer.Newline, lexer.Semi, lexer.DoubleSemi, lexer.EOF:
			default:
				return nil, p.unexpected()
			}
			p.skipSeps()
		}
		if len(body.Kids) == 0 {
			return nil, diag.Errorf("E0110", p.peek().Pos, "an empty match arm")
		}
		n.Kids = append(n.Kids, tree.New("arm", pat.Pos, pat, guard, body))
	}
	p.next() // ;;
	if len(n.Kids) == 1 {
		return nil, diag.Errorf("E0110", t.Pos, "a match without arms")
	}
	return n, nil
}

// forExpr parses `for pat in e: body ;;`.
func (p *parser) forExpr() (*tree.Node, *diag.Error) {
	t := p.next() // for
	pat, err := p.pattern()
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.KwIn, "`in` after the for pattern"); err != nil {
		return nil, err
	}
	iter, err := p.expr(0)
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.Colon, "`:` after the for header"); err != nil {
		return nil, err
	}
	body, err := p.body(t.Pos)
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.DoubleSemi, "`;;` to close the for"); err != nil {
		return nil, err
	}
	return tree.New("for", t.Pos, pat, iter, body), nil
}
