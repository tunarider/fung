package parser

import (
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/tree"
)

// statement parses `NAME = expr` or an expression statement. Assignment
// is a statement, not an expression (01), so `x = y = 1` is a syntax error.
func (p *parser) statement() (*tree.Node, *diag.Error) {
	t := p.peek()
	if t.Kind == lexer.KwType {
		return p.typeAlias()
	}
	if t.Kind == lexer.KwImport {
		return p.importStmt()
	}
	if t.Kind == lexer.KwTest {
		return p.testBlock()
	}
	if t.Kind == lexer.KwAssert {
		p.next()
		if p.at(lexer.EOF) || p.at(lexer.Newline) || p.at(lexer.Semi) || p.at(lexer.DoubleSemi) {
			return nil, p.unexpected()
		}
		e, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		return tree.New("assert", t.Pos, e), nil
	}
	// `_ = e` evaluates e and discards it (03): the wildcard pattern.
	if t.Kind == lexer.Name && t.Text == "_" && p.toks[p.i+1].Kind == lexer.Assign {
		p.next()
		p.next()
		p.skipNewlinesAfterOperator()
		rhs, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		return tree.New("assign", t.Pos, tree.New("pwild", t.Pos), rhs), nil
	}
	// Destructuring: try `pattern =` first when the statement starts like a
	// pattern (01: 문장 시작에서 패턴 =를 먼저 시도).
	if t.Kind == lexer.LParen || t.Kind == lexer.LBracket || t.Kind == lexer.LBrace || t.Kind == lexer.Tag {
		if n, ok, err := p.tryDestructure(); ok || err != nil {
			return n, err
		}
	}
	if t.Kind == lexer.KwFn && p.toks[p.i+1].Kind == lexer.Name {
		p.next()
		name := p.next()
		return p.fnDef(t, name.Text)
	}
	if t.Kind == lexer.Name && p.toks[p.i+1].Kind == lexer.Assign {
		p.next()
		p.next()
		p.skipNewlinesAfterOperator()
		rhs, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		return tree.New("assign", t.Pos, tree.Leaf("name", t.Pos, t.Text), rhs), nil
	}
	return p.expr(0)
}

// expectDesc is expect with an E0112 "expected X, found Y" message.
func (p *parser) expectDesc(k lexer.Kind, what string) (lexer.Token, *diag.Error) {
	if !p.at(k) {
		t := p.peek()
		return lexer.Token{}, diag.Errorf("E0112", t.Pos, what, describe(t))
	}
	return p.next(), nil
}

// body parses statements up to (not including) `;;` or `else`. Inside a
// block newlines separate statements even when the block sits inside
// parentheses (01: 괄호 안에서 블록이 열리면 그 ;;까지는 줄바꿈이 구분자).
func (p *parser) body(pos diag.Pos) (*tree.Node, *diag.Error) {
	saved := p.nest
	p.nest = 0
	defer func() { p.nest = saved }()

	p.blockDepth++
	defer func() { p.blockDepth-- }()
	blk := tree.New("block", pos)
	p.skipSeps()
	for !p.at(lexer.DoubleSemi) && !p.at(lexer.KwElse) && !p.at(lexer.EOF) {
		stmt, err := p.spannedStatement()
		if err != nil {
			return nil, err
		}
		blk.Kids = append(blk.Kids, stmt)
		switch p.peek().Kind {
		case lexer.Newline, lexer.Semi, lexer.DoubleSemi, lexer.KwElse, lexer.EOF:
		default:
			return nil, p.unexpected()
		}
		p.skipSeps()
	}
	return blk, nil
}

// ifExpr parses a whole if/else-if/else chain closed by one `;;`.
func (p *parser) ifExpr() (*tree.Node, *diag.Error) {
	n, err := p.ifChain()
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.DoubleSemi, "`;;` to close the if"); err != nil {
		return nil, err
	}
	return n, nil
}

// ifChain parses `if cond: body` plus an optional else part, without the
// closing `;;` (the whole chain shares one).
func (p *parser) ifChain() (*tree.Node, *diag.Error) {
	t := p.next() // if
	cond, err := p.expr(0)
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.Colon, "`:` after the if condition"); err != nil {
		return nil, err
	}
	thenB, err := p.body(t.Pos)
	if err != nil {
		return nil, err
	}
	var elseB *tree.Node
	if p.at(lexer.KwElse) {
		e := p.next()
		switch {
		case p.at(lexer.KwIf):
			elseB, err = p.ifChain()
		case p.at(lexer.Colon):
			p.next()
			elseB, err = p.body(e.Pos)
		default:
			err = diag.Errorf("E0112", p.peek().Pos, "`:` or `if` after else", describe(p.peek()))
		}
		if err != nil {
			return nil, err
		}
	} else {
		elseB = tree.New("nil", t.Pos)
	}
	return tree.New("if", t.Pos, cond, thenB, elseB), nil
}

// tryDestructure parses `pattern = expr` at the statement start. When the
// text is not an assignment it restores the position and reports ok=false.
func (p *parser) tryDestructure() (*tree.Node, bool, *diag.Error) {
	save, saveNest, saveDepth := p.i, p.nest, p.depth
	pat, err := p.pattern()
	if err != nil || !p.at(lexer.Assign) {
		p.i, p.nest, p.depth = save, saveNest, saveDepth
		return nil, false, nil
	}
	p.next()
	p.skipNewlinesAfterOperator()
	rhs, err := p.expr(0)
	if err != nil {
		return nil, true, err
	}
	return tree.New("assign", pat.Pos, pat, rhs), true, nil
}

// testBlock parses `test "name": body ;;` (08). Tests live at the top
// level only; `fung run` drops them, `fung test` runs them.
func (p *parser) testBlock() (*tree.Node, *diag.Error) {
	t := p.next() // test
	if p.blockDepth > 0 {
		return nil, diag.Errorf("E0114", t.Pos)
	}
	if !p.at(lexer.String) {
		return nil, diag.Errorf("E0112", p.peek().Pos, "a test name string after `test`", describe(p.peek()))
	}
	s := p.next()
	var name strings.Builder
	for _, part := range s.Parts {
		if part.IsCode {
			return nil, diag.Errorf("E0112", s.Pos, "a plain test name", "a string with interpolation")
		}
		name.WriteString(part.Text)
	}
	if _, err := p.expectDesc(lexer.Colon, "`:` after the test name"); err != nil {
		return nil, err
	}
	body, err := p.body(t.Pos)
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.DoubleSemi, "`;;` to close the test"); err != nil {
		return nil, err
	}
	return tree.WithLit("test", t.Pos, name.String(), body), nil
}

// importStmt parses `import NAME|STRING [as NAME] [(NAME, …)]` (01 EBNF,
// 04). The path string must be plain: no interpolation.
func (p *parser) importStmt() (*tree.Node, *diag.Error) {
	t := p.next() // import
	im := &tree.Import{}
	switch p.peek().Kind {
	case lexer.Name:
		// Dotted names: `_native.fs` for the stdlib's own layer (06) and,
		// later, packages.
		im.Spec = p.next().Text
		for p.at(lexer.Dot) && p.toks[p.i+1].Kind == lexer.Name {
			p.next()
			im.Spec += "." + p.next().Text
		}
	case lexer.String:
		s := p.next()
		var text strings.Builder
		for _, part := range s.Parts {
			if part.IsCode {
				return nil, diag.Errorf("E0112", s.Pos, "a plain path string after `import`", "a string with interpolation")
			}
			text.WriteString(part.Text)
		}
		im.Spec = text.String()
		im.IsPath = true
	default:
		return nil, diag.Errorf("E0112", p.peek().Pos, "a module name or a path string after `import`", describe(p.peek()))
	}
	if p.at(lexer.KwAs) {
		p.next()
		name, err := p.expectDesc(lexer.Name, "a name after `as`")
		if err != nil {
			return nil, err
		}
		im.Alias = name.Text
	}
	if p.at(lexer.LParen) {
		p.next()
		for {
			name, err := p.expectDesc(lexer.Name, "a name to import")
			if err != nil {
				return nil, err
			}
			im.Names = append(im.Names, name.Text)
			im.NamePos = append(im.NamePos, name.Pos)
			if !p.at(lexer.Comma) {
				break
			}
			p.next()
		}
		if _, err := p.expectDesc(lexer.RParen, "`)`"); err != nil {
			return nil, err
		}
	}
	return tree.WithLit("import", t.Pos, im), nil
}

// typeAlias parses `type name = type` (07). Alias names are lowercase.
func (p *parser) typeAlias() (*tree.Node, *diag.Error) {
	t := p.next() // type
	name, err := p.expectDesc(lexer.Name, "a lowercase alias name after `type`")
	if err != nil {
		return nil, err
	}
	if _, err := p.expectDesc(lexer.Assign, "`=`"); err != nil {
		return nil, err
	}
	alts, err := p.typeSpec()
	if err != nil {
		return nil, err
	}
	return tree.WithLit("typealias", t.Pos, &tree.TypeAlias{Name: name.Text, Alts: alts}), nil
}
