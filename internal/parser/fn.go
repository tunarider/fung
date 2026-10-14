package parser

import (
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/tree"
)

// fnDef parses `fn NAME(params) [-> type] (= expr | block)` after `fn`
// (and the name, for a statement) has been consumed. Anonymous functions
// (name "") only have the block form (01).
func (p *parser) fnDef(fnTok lexer.Token, name string) (*tree.Node, *diag.Error) {
	info := &tree.FnInfo{Name: name}
	if _, err := p.expectDesc(lexer.LParen, "`(` after fn"); err != nil {
		return nil, err
	}
	defaults, err := p.params(info)
	if err != nil {
		return nil, err
	}
	if p.at(lexer.Arrow) {
		p.next()
		info.Return, err = p.typeSpec()
		if err != nil {
			return nil, err
		}
	}
	var body *tree.Node
	switch {
	case p.at(lexer.Assign) && name != "":
		eq := p.next()
		p.skipNewlinesAfterOperator()
		e, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		body = tree.New("block", eq.Pos, e)
	case p.at(lexer.Colon):
		p.next()
		body, err = p.body(fnTok.Pos)
		if err != nil {
			return nil, err
		}
		if _, err := p.expectDesc(lexer.DoubleSemi, "`;;` to close the function"); err != nil {
			return nil, err
		}
	default:
		return nil, p.unexpected()
	}
	kids := append(defaults, body)
	return tree.WithLit("fn", fnTok.Pos, info, kids...), nil
}

// params parses the parameter list after `(` up to and including `)`,
// returning the default expressions in parameter order (03: 필수 →
// 기본값 → 마지막에 ...rest 하나).
func (p *parser) params(info *tree.FnInfo) ([]*tree.Node, *diag.Error) {
	p.nest++
	defer func() { p.nest-- }()
	var defaults []*tree.Node
	seen := map[string]bool{}
	sawDefault, sawRest := false, false
	for !p.at(lexer.RParen) {
		if sawRest {
			return nil, p.unexpected()
		}
		var prm tree.Param
		if p.at(lexer.Ellipsis) {
			p.next()
			prm.Rest = true
			sawRest = true
		}
		if !prm.Rest && p.patternParamAhead() {
			// A destructuring parameter (03): no type, no default.
			pat, err := p.pattern()
			if err != nil {
				return nil, err
			}
			if p.at(lexer.Colon) || p.at(lexer.Assign) {
				return nil, p.unexpected()
			}
			if sawDefault {
				return nil, diag.Errorf("E0110", pat.Pos, "required parameter after an optional one")
			}
			info.Params = append(info.Params, tree.Param{Pos: pat.Pos, Pattern: pat})
			if p.at(lexer.Comma) {
				p.next()
				continue
			}
			if !p.at(lexer.RParen) {
				return nil, p.unexpected()
			}
			continue
		}
		nameTok, err := p.expectDesc(lexer.Name, "a parameter name")
		if err != nil {
			return nil, err
		}
		if seen[nameTok.Text] {
			return nil, diag.Errorf("E0110", nameTok.Pos, "duplicate parameter `"+nameTok.Text+"`")
		}
		seen[nameTok.Text] = true
		prm.Name = nameTok.Text
		prm.Pos = nameTok.Pos
		if p.at(lexer.Colon) {
			p.next()
			prm.Type, err = p.typeSpec()
			if err != nil {
				return nil, err
			}
		}
		if p.at(lexer.Assign) {
			if prm.Rest {
				return nil, p.unexpected()
			}
			p.next()
			d, err := p.expr(0)
			if err != nil {
				return nil, err
			}
			defaults = append(defaults, d)
			prm.HasDefault = true
			sawDefault = true
		} else if sawDefault && !prm.Rest {
			return nil, diag.Errorf("E0110", nameTok.Pos, "required parameter `"+nameTok.Text+"` after an optional one")
		}
		info.Params = append(info.Params, prm)
		if p.at(lexer.Comma) {
			p.next()
			continue
		}
		if !p.at(lexer.RParen) {
			return nil, p.unexpected()
		}
	}
	p.next() // )
	return defaults, nil
}

// typeSpec parses `term { | term }` where term = NAME [ "[" type "]" ] | TAG.
// Element types are parsed and dropped (07: 런타임은 바깥 타입만 검사).
func (p *parser) typeSpec() ([]string, *diag.Error) {
	var alts []string
	for {
		t := p.peek()
		// `nil` and `fn` are keywords but also type names (07).
		if t.Kind != lexer.Name && t.Kind != lexer.Tag && t.Kind != lexer.KwNil && t.Kind != lexer.KwFn {
			return nil, diag.Errorf("E0110", t.Pos, describe(t))
		}
		p.next()
		alts = append(alts, t.Text)
		if p.at(lexer.LBracket) {
			p.next()
			if _, err := p.typeSpec(); err != nil {
				return nil, err
			}
			if _, err := p.expectDesc(lexer.RBracket, "`]`"); err != nil {
				return nil, err
			}
		}
		if !p.at(lexer.Bar) {
			return alts, nil
		}
		p.next()
	}
}

// patternParamAhead reports whether a parameter starts like a pattern
// rather than a name: a tuple, list, map, tag, or `_`.
func (p *parser) patternParamAhead() bool {
	t := p.peek()
	switch t.Kind {
	case lexer.LParen, lexer.LBracket, lexer.LBrace, lexer.Tag:
		return true
	case lexer.Name:
		return t.Text == "_"
	}
	return false
}

// lambdaParamsAhead reports whether the tokens at p.i form `( … ) =>`
// with balanced brackets, without consuming anything. The contents are
// parsed as patterns by lambda (03).
func (p *parser) lambdaParamsAhead() bool {
	j := p.i
	if p.toks[j].Kind != lexer.LParen {
		return false
	}
	depth := 0
	for ; j < len(p.toks); j++ {
		switch p.toks[j].Kind {
		case lexer.LParen, lexer.LBracket, lexer.LBrace:
			depth++
		case lexer.RParen, lexer.RBracket, lexer.RBrace:
			depth--
			if depth == 0 {
				return j+1 < len(p.toks) && p.toks[j+1].Kind == lexer.FatArrow
			}
		case lexer.Newline, lexer.EOF:
			return false
		}
	}
	return false
}

// lambda parses `NAME => expr` or `( names ) => expr`. The body is one
// expression read as far as possible (01).
func (p *parser) lambda() (*tree.Node, *diag.Error) {
	start := p.peek()
	info := &tree.FnInfo{}
	if start.Kind == lexer.Name {
		p.next()
		if start.Text == "_" {
			info.Params = []tree.Param{{Pos: start.Pos, Pattern: tree.New("pwild", start.Pos)}}
		} else {
			info.Params = []tree.Param{{Name: start.Text, Pos: start.Pos}}
		}
	} else {
		p.next() // (
		p.nest++
		for !p.at(lexer.RParen) {
			pat, err := p.pattern()
			if err != nil {
				p.nest--
				return nil, err
			}
			if pat.Head == "pbind" {
				info.Params = append(info.Params, tree.Param{Name: pat.Lit.(string), Pos: pat.Pos})
			} else {
				info.Params = append(info.Params, tree.Param{Pos: pat.Pos, Pattern: pat})
			}
			if p.at(lexer.Comma) {
				p.next()
				continue
			}
			if !p.at(lexer.RParen) {
				p.nest--
				return nil, p.unexpected()
			}
		}
		p.nest--
		p.next() // )
	}
	arrow := p.next() // =>
	p.skipNewlinesAfterOperator()
	body, err := p.expr(0)
	if err != nil {
		return nil, err
	}
	return tree.WithLit("fn", start.Pos, info, tree.New("block", arrow.Pos, body)), nil
}
