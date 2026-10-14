package parser

import (
	"math"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/tree"
)

// Binding powers from 07. 우선순위 (낮음 → 높음), scaled by 10 so that
// right-associative and non-associative operators can be expressed by
// giving the right side a different power than the left.
const (
	bpThen     = 10
	bpOr       = 20
	bpAnd      = 30
	bpNot      = 40
	bpCompare  = 50
	bpPipe     = 60
	bpCoalesce = 70
	bpRange    = 80
	bpAdd      = 90
	bpMul      = 100
	bpNeg      = 110
	bpPow      = 120
	bpPostfix  = 130
)

type infixInfo struct {
	left, right int
	text        string
	compare     bool
}

// infix lists the binary operators M1 understands. Later milestones add
// |>, ??, .., ..=, in, then here.
var infix = map[lexer.Kind]infixInfo{
	lexer.KwOr:       {bpOr, bpOr + 1, "or", false},
	lexer.KwAnd:      {bpAnd, bpAnd + 1, "and", false},
	lexer.Eq:         {bpCompare, bpCompare + 1, "==", true},
	lexer.Ne:         {bpCompare, bpCompare + 1, "!=", true},
	lexer.Lt:         {bpCompare, bpCompare + 1, "<", true},
	lexer.Gt:         {bpCompare, bpCompare + 1, ">", true},
	lexer.Le:         {bpCompare, bpCompare + 1, "<=", true},
	lexer.Ge:         {bpCompare, bpCompare + 1, ">=", true},
	lexer.Plus:       {bpAdd, bpAdd + 1, "+", false},
	lexer.Minus:      {bpAdd, bpAdd + 1, "-", false},
	lexer.Star:       {bpMul, bpMul + 1, "*", false},
	lexer.Slash:      {bpMul, bpMul + 1, "/", false},
	lexer.SlashSlash: {bpMul, bpMul + 1, "//", false},
	lexer.Percent:    {bpMul, bpMul + 1, "%", false},
	lexer.StarStar:   {bpPow, bpPow, "**", false}, // right-assoc: same power on the right
	lexer.Pipe:       {bpPipe, bpPipe + 1, "|>", false},
	lexer.Coalesce:   {bpCoalesce, bpCoalesce + 1, "??", false},
	lexer.DotDot:     {bpRange, bpRange + 1, "..", false},
	lexer.DotDotEq:   {bpRange, bpRange + 1, "..=", false},
	lexer.KwIn:       {bpCompare, bpCompare + 1, "in", true},
}

// continuesLine lists tokens that may start a continuation line (01).
var continuesLine = map[lexer.Kind]bool{
	lexer.Pipe: true, lexer.Dot: true, lexer.Coalesce: true,
	lexer.KwAnd: true, lexer.KwOr: true, lexer.KwThen: true, lexer.KwElse: true,
}

// peekInfix looks past a newline when the next line starts with a
// continuation token, without consuming anything otherwise.
func (p *parser) peekInfix() lexer.Token {
	t := p.peek()
	if t.Kind != lexer.Newline {
		return t
	}
	j := p.i
	for p.toks[j].Kind == lexer.Newline {
		j++
	}
	if continuesLine[p.toks[j].Kind] {
		p.i = j
		return p.toks[j]
	}
	return t
}

// expr parses an expression whose operators bind at least as tightly as minBP.
func (p *parser) expr(minBP int) (*tree.Node, *diag.Error) {
	if p.depth >= maxDepth {
		return nil, diag.Errorf("E0802", p.peek().Pos, maxDepth)
	}
	p.depth++
	defer func() { p.depth-- }()
	startOff := p.peek().Off
	lhs, err := p.prefix()
	if err != nil {
		return nil, err
	}
	lastWasCompare := false
	for {
		p.span(lhs, startOff)
		t := p.peekInfix()
		// then … else: the lowest operator; both branches are full expressions.
		if t.Kind == lexer.KwThen && bpThen >= minBP {
			p.next()
			p.skipNewlinesAfterOperator()
			a, err := p.expr(0)
			if err != nil {
				return nil, err
			}
			p.peekInfix() // an `else` on the next line continues the expression
			if _, err := p.expectDesc(lexer.KwElse, "`else` after then"); err != nil {
				return nil, err
			}
			p.skipNewlinesAfterOperator()
			b, err := p.expr(0)
			if err != nil {
				return nil, err
			}
			return p.span(tree.New("then", t.Pos, lhs, a, b), startOff), nil
		}
		// Postfix forms bind tightest: call and index only on the same
		// line, `.` also as a continuation (01), `!` and `?` right after.
		if bpPostfix >= minBP {
			switch t.Kind {
			case lexer.LParen:
				lhs, err = p.call(lhs)
			case lexer.LBracket:
				lhs, err = p.indexOrSlice(lhs)
			case lexer.Dot:
				p.next()
				if !p.atNameLike() {
					return nil, diag.Errorf("E0112", p.peek().Pos, "a field name after `.`", describe(p.peek()))
				}
				name := p.next()
				lhs = tree.WithLit("field", t.Pos, name.Text, lhs)
			case lexer.Bang, lexer.Question:
				p.next()
				lhs = tree.WithLit("unwrap", t.Pos, t.Kind.String(), lhs)
			default:
				goto infixOp
			}
			if err != nil {
				return nil, err
			}
			continue
		}
	infixOp:
		notIn := t.Kind == lexer.KwNot && p.toks[p.i+1].Kind == lexer.KwIn
		info, ok := infix[t.Kind]
		if notIn {
			info, ok = infixInfo{bpCompare, bpCompare + 1, "not_in", true}, true
		}
		if !ok || info.left < minBP {
			return lhs, nil
		}
		if info.compare && lastWasCompare {
			return nil, diag.Errorf("E0111", t.Pos)
		}
		p.next()
		if notIn {
			p.next()
		}
		p.skipNewlinesAfterOperator()
		rhs, err := p.expr(info.right)
		if err != nil {
			return nil, err
		}
		switch info.text {
		case "|>":
			lhs = tree.New("pipe", t.Pos, lhs, rhs)
		case "??":
			lhs = tree.New("coalesce", t.Pos, lhs, rhs)
		case "..", "..=":
			lhs = tree.WithLit("range", t.Pos, info.text, lhs, rhs)
		default:
			lhs = tree.WithLit("binary", t.Pos, info.text, lhs, rhs)
		}
		lastWasCompare = info.compare
	}
}

// skipNewlinesAfterOperator implements "a line ending in an operator continues".
func (p *parser) skipNewlinesAfterOperator() {
	for p.toks[p.i].Kind == lexer.Newline {
		p.i++
	}
}

func (p *parser) prefix() (*tree.Node, *diag.Error) {
	t := p.peek()
	switch t.Kind {
	case lexer.Int:
		if isMinIntMagnitude(t) {
			return nil, diag.Errorf("E0102", t.Pos, t.Text)
		}
		p.next()
		return tree.Leaf("int", t.Pos, t.Int), nil
	case lexer.KwNil:
		p.next()
		return tree.New("nil", t.Pos), nil
	case lexer.String:
		p.next()
		return p.stringLiteral(t)
	case lexer.RawString:
		p.next()
		return tree.Leaf("string", t.Pos, t.Text), nil
	case lexer.Symbol:
		p.next()
		return tree.Leaf("symbol", t.Pos, t.Text), nil
	case lexer.Duration:
		p.next()
		return tree.Leaf("duration", t.Pos, t.Dur), nil
	case lexer.KwIf:
		return p.ifExpr()
	case lexer.KwMatch:
		return p.matchExpr()
	case lexer.KwFor:
		return p.forExpr()
	case lexer.LBracket:
		return p.listLiteral()
	case lexer.LBrace:
		return p.mapLiteral()
	case lexer.Tag:
		return p.tagExpr()
	case lexer.KwFn:
		p.next()
		return p.fnDef(t, "")
	case lexer.KwReturn:
		p.next()
		switch p.peek().Kind {
		case lexer.Newline, lexer.Semi, lexer.DoubleSemi, lexer.RParen, lexer.KwElse, lexer.EOF, lexer.Comma:
			return tree.New("return", t.Pos, tree.New("nil", t.Pos)), nil
		}
		e, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		return tree.New("return", t.Pos, e), nil
	case lexer.Float:
		p.next()
		return tree.Leaf("float", t.Pos, t.Float), nil
	case lexer.KwTrue:
		p.next()
		return tree.Leaf("bool", t.Pos, true), nil
	case lexer.KwFalse:
		p.next()
		return tree.Leaf("bool", t.Pos, false), nil
	case lexer.Name:
		if p.toks[p.i+1].Kind == lexer.FatArrow && p.bareLambdaOK() {
			return p.lambda()
		}
		p.next()
		return tree.Leaf("name", t.Pos, t.Text), nil
	case lexer.Minus:
		// -9223372036854775808: the lexer cannot know a minus precedes the
		// magnitude, so the parser folds exactly this case (01).
		if nt := p.toks[p.i+1]; nt.Kind == lexer.Int && isMinIntMagnitude(nt) {
			p.next()
			p.next()
			return tree.Leaf("int", t.Pos, int64(math.MinInt64)), nil
		}
		p.next()
		p.skipNewlinesAfterOperator()
		operand, err := p.expr(bpNeg)
		if err != nil {
			return nil, err
		}
		return tree.WithLit("unary", t.Pos, "-", operand), nil
	case lexer.KwNot:
		p.next()
		p.skipNewlinesAfterOperator()
		operand, err := p.expr(bpNot)
		if err != nil {
			return nil, err
		}
		return tree.WithLit("unary", t.Pos, "not", operand), nil
	case lexer.LParen:
		if p.lambdaParamsAhead() && p.bareLambdaOK() {
			return p.lambda()
		}
		return p.parenOrTuple()
	}
	return nil, p.unexpected()
}

// call parses `(args)` after a callee.
func (p *parser) call(callee *tree.Node) (*tree.Node, *diag.Error) {
	open := p.peek()
	return p.callArgs(tree.New("call", open.Pos, callee))
}

// callArgs parses `(args)` into node's kids: positional, `_` holes
// (partial application, 03), then named arguments. Inside the
// parentheses newlines are whitespace and a trailing comma is allowed.
func (p *parser) callArgs(node *tree.Node) (*tree.Node, *diag.Error) {
	p.next() // (
	p.nest++
	defer func() { p.nest-- }()
	sawNamed := false
	for !p.at(lexer.RParen) {
		switch {
		case p.at(lexer.Name) && p.toks[p.i+1].Kind == lexer.Colon:
			// Named argument: positional ones must come first (03).
			nameTok := p.next()
			p.next()
			if p.at(lexer.Name) && p.peek().Text == "_" && isArgEnd(p.toks[p.i+1].Kind) {
				return nil, diag.Errorf("E0110", p.peek().Pos, "`_` in a named argument")
			}
			e, err := p.expr(0)
			if err != nil {
				return nil, err
			}
			node.Kids = append(node.Kids, tree.WithLit("named", nameTok.Pos, nameTok.Text, e))
			sawNamed = true
		case p.at(lexer.Name) && p.peek().Text == "_" && isArgEnd(p.toks[p.i+1].Kind):
			if sawNamed {
				return nil, p.unexpected()
			}
			node.Kids = append(node.Kids, tree.New("hole", p.next().Pos))
		default:
			if sawNamed {
				return nil, p.unexpected()
			}
			arg, err := p.expr(0)
			if err != nil {
				return nil, err
			}
			node.Kids = append(node.Kids, arg)
		}
		if p.at(lexer.Comma) {
			p.next()
			continue
		}
		if !p.at(lexer.RParen) {
			return nil, p.unexpected()
		}
	}
	p.next() // )
	return node, nil
}

// isArgEnd reports whether a token ends a call argument.
func isArgEnd(k lexer.Kind) bool { return k == lexer.Comma || k == lexer.RParen || k == lexer.Newline }

// isMinIntMagnitude reports whether an Int token is the lexer's special
// 9223372036854775808 token (see lexer.number).
func isMinIntMagnitude(t lexer.Token) bool {
	return t.Int == math.MinInt64 && strings.ReplaceAll(t.Text, "_", "") == "9223372036854775808"
}

// stringLiteral turns a "..." token into a string leaf or an (interp ...)
// node whose code parts are parsed with positions in the enclosing file.
func (p *parser) stringLiteral(t lexer.Token) (*tree.Node, *diag.Error) {
	hasCode := false
	for _, part := range t.Parts {
		hasCode = hasCode || part.IsCode
	}
	if !hasCode {
		var text strings.Builder
		for _, part := range t.Parts {
			text.WriteString(part.Text)
		}
		return tree.Leaf("string", t.Pos, text.String()), nil
	}
	n := tree.New("interp", t.Pos)
	for _, part := range t.Parts {
		if !part.IsCode {
			n.Kids = append(n.Kids, tree.Leaf("string", t.Pos, part.Text))
			continue
		}
		toks, _, err := lexer.LexAt(part.Pos.File, part.Text, part.Pos, part.Off)
		if err != nil {
			return nil, err
		}
		sub := &parser{toks: toks, depth: p.depth}
		e, err := sub.expr(0)
		if err != nil {
			return nil, err
		}
		sub.skipSeps()
		if !sub.at(lexer.EOF) {
			return nil, sub.unexpected()
		}
		n.Kids = append(n.Kids, e)
	}
	return n, nil
}

// indexOrSlice parses `[e]`, `[a..b]`, `[a..=b]`, `[..b]`, `[a..]` after an
// expression (01). The bracket holds a full expression; a range at its
// top level means a slice, so `xs[i ?? 0]` indexes and `xs[1..=3]` slices.
func (p *parser) indexOrSlice(x *tree.Node) (*tree.Node, *diag.Error) {
	open := p.next() // [
	p.nest++
	defer func() { p.nest-- }()
	nilNode := func() *tree.Node { return tree.New("nil", open.Pos) }
	closeBracket := func() *diag.Error {
		_, err := p.expectDesc(lexer.RBracket, "`]`")
		return err
	}
	if p.at(lexer.DotDot) { // [..b] and [..]
		p.next()
		if p.at(lexer.RBracket) {
			p.next()
			return tree.New("slice", open.Pos, x, nilNode(), nilNode()), nil
		}
		end, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		if err := closeBracket(); err != nil {
			return nil, err
		}
		return tree.New("slice", open.Pos, x, nilNode(), end), nil
	}
	if p.at(lexer.DotDotEq) { // [..=b]
		p.next()
		end, err := p.expr(0)
		if err != nil {
			return nil, err
		}
		if err := closeBracket(); err != nil {
			return nil, err
		}
		return tree.WithLit("slice", open.Pos, "..=", x, nilNode(), end), nil
	}
	// [a..] has no expression after `..`, so the general expression parser
	// cannot read it: try the open-ended form first and back out otherwise.
	save, saveDepth := p.i, p.depth
	if start, err := p.expr(bpRange + 1); err == nil && p.at(lexer.DotDot) && p.toks[p.i+1].Kind == lexer.RBracket {
		p.next()
		p.next()
		return tree.New("slice", open.Pos, x, start, nilNode()), nil
	}
	p.i, p.depth = save, saveDepth
	e, err := p.expr(0)
	if err != nil {
		return nil, err
	}
	if err := closeBracket(); err != nil {
		return nil, err
	}
	if e.Head == "range" {
		if e.Lit == "..=" {
			return tree.WithLit("slice", open.Pos, "..=", x, e.Kids[0], e.Kids[1]), nil
		}
		return tree.New("slice", open.Pos, x, e.Kids[0], e.Kids[1]), nil
	}
	return tree.New("index", open.Pos, x, e), nil
}
