// Package parser builds the surface tree from tokens. Statements use
// recursive descent; expressions use a Pratt loop (expr.go). Newlines are
// tokens, and the parser decides from context whether one ends a
// statement (01. 줄 이어짐 규칙).
package parser

import (
	"fmt"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/tree"
)

type parser struct {
	toks  []lexer.Token
	i     int
	nest  int // depth of ( [ { — newlines are whitespace inside
	depth int // recursion depth of expr(), bounded by maxDepth
	last  int // index of the last token consumed by next(), for spans
	// blockDepth counts open bodies (fn, if, match arms…): test blocks are
	// only allowed at depth 0 (E0114).
	blockDepth int
	// guard is set while parsing a match arm's `when` condition: there a
	// bare `name =>` is the arm's arrow, not a lambda (lambdas need
	// parentheses around them, as in a call argument, where nest > 0).
	guard bool
}

// bareLambdaOK reports whether `x => …` / `(x) => …` may start a lambda
// here (not at the top level of a match guard).
func (p *parser) bareLambdaOK() bool { return !p.guard || p.nest > 0 }

// maxDepth bounds expression nesting so absurd input yields a fung error
// instead of a Go stack overflow, which recover() cannot catch.
const maxDepth = 10000

// Parse returns the program as (program stmt...). Only the first syntax
// error is reported (08).
func Parse(toks []lexer.Token) (*tree.Node, *diag.Error) {
	p := &parser{toks: toks}
	return p.program()
}

// span records n's byte range: from startOff to the end of the last
// consumed token (assert and dbg show source text, 08).
func (p *parser) span(n *tree.Node, startOff int) *tree.Node {
	n.Off = startOff
	n.End = p.toks[p.last].End
	return n
}

// spannedStatement parses a statement and records its span.
func (p *parser) spannedStatement() (*tree.Node, *diag.Error) {
	start := p.peek().Off
	n, err := p.statement()
	if err != nil {
		return nil, err
	}
	return p.span(n, start), nil
}

// peek returns the next significant token. Inside brackets, newlines are
// skipped here so every caller sees them as whitespace.
func (p *parser) peek() lexer.Token {
	for p.nest > 0 && p.toks[p.i].Kind == lexer.Newline {
		p.i++
	}
	return p.toks[p.i]
}

func (p *parser) next() lexer.Token {
	t := p.peek()
	if t.Kind != lexer.EOF {
		p.last = p.i
		p.i++
	}
	return t
}

func (p *parser) at(k lexer.Kind) bool { return p.peek().Kind == k }

// atNameLike reports a token usable as a name where nothing else can
// appear (01): a name, a tag, or a keyword (`m.type`, `{match: 1}`).
func (p *parser) atNameLike() bool {
	k := p.peek().Kind
	return k == lexer.Name || k == lexer.Tag || k.IsKeyword()
}

func (p *parser) expect(k lexer.Kind) (lexer.Token, *diag.Error) {
	if !p.at(k) {
		return lexer.Token{}, p.unexpected()
	}
	return p.next(), nil
}

// unexpected builds E0110 for the current token.
func (p *parser) unexpected() *diag.Error {
	t := p.peek()
	return diag.Errorf("E0110", t.Pos, describe(t))
}

func describe(t lexer.Token) string {
	switch t.Kind {
	case lexer.EOF, lexer.Newline:
		return t.Kind.String()
	case lexer.Int, lexer.Float, lexer.Duration, lexer.String, lexer.RawString, lexer.Name, lexer.Tag:
		return fmt.Sprintf("%v `%s`", t.Kind, t.Text)
	case lexer.Symbol:
		return fmt.Sprintf("symbol `'%s`", t.Text)
	}
	return "`" + t.Kind.String() + "`"
}

func (p *parser) skipSeps() {
	for p.at(lexer.Newline) || p.at(lexer.Semi) {
		p.next()
	}
}

// program = body at nesting 0, terminated by EOF.
func (p *parser) program() (*tree.Node, *diag.Error) {
	prog := tree.New("program", p.peek().Pos)
	p.skipSeps()
	for !p.at(lexer.EOF) {
		stmt, err := p.spannedStatement()
		if err != nil {
			return nil, err
		}
		prog.Kids = append(prog.Kids, stmt)
		if !p.at(lexer.EOF) && !p.at(lexer.Newline) && !p.at(lexer.Semi) {
			return nil, p.unexpected()
		}
		p.skipSeps()
	}
	return prog, nil
}
