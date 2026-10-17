package format

import "github.com/tunarider/fung/internal/lexer"

// opener is something that indents the lines after it until it is
// closed: a block (fn/if/match/for/test, `:` … `;;`) or a bracket.
type opener struct {
	kind  lexer.Kind // KwIf, KwMatch, KwFn, KwFor, KwTest for blocks; LParen/LBracket/LBrace for brackets
	block bool
	base  int // indent of the line that opened it
}

// pending is a block keyword waiting for its `:` at a bracket depth.
type pending struct {
	kind  lexer.Kind
	depth int // bracket depth where the keyword appeared
}

type layouter struct {
	stack    []opener
	pend     []pending
	thens    []int // unmatched `then` per bracket frame (index = bracket depth)
	contNext bool  // the previous code line ended with a token that cannot end a statement (01)
}

// endsOpen reports a line-final token after which the statement goes on
// (01: 줄 이어짐 규칙 1), brackets and block colons aside — those indent
// through the opener stack.
func endsOpen(k lexer.Kind) bool {
	switch k {
	case lexer.Plus, lexer.Minus, lexer.Star, lexer.Slash, lexer.SlashSlash, lexer.Percent, lexer.StarStar,
		lexer.Eq, lexer.Ne, lexer.Lt, lexer.Gt, lexer.Le, lexer.Ge, lexer.KwIn, lexer.Coalesce, lexer.Pipe,
		lexer.DotDot, lexer.DotDotEq, lexer.KwAnd, lexer.KwOr, lexer.KwNot, lexer.Assign, lexer.FatArrow,
		lexer.Arrow, lexer.Dot, lexer.KwThen, lexer.KwElse, lexer.Bar:
		return true
	}
	return false
}

// continues reports whether a line continues the statement before it:
// it starts with a continuation token, is the `else` of a `then`, or
// the previous line ended open.
func (l *layouter) continues(first lexer.Kind) bool {
	return isContinuation(first) || (first == lexer.KwElse && l.thenElse()) || l.contNext
}

func (l *layouter) brackets() int {
	n := 0
	for _, o := range l.stack {
		if !o.block {
			n++
		}
	}
	return n
}

func (l *layouter) innermost() *opener {
	if len(l.stack) == 0 {
		return nil
	}
	return &l.stack[len(l.stack)-1]
}

// thenElse reports whether an `else` here continues a `then` expression
// of the current statement rather than an if-chain.
func (l *layouter) thenElse() bool { return l.thens[l.brackets()] > 0 }

func isContinuation(k lexer.Kind) bool {
	switch k {
	case lexer.Pipe, lexer.Dot, lexer.Coalesce, lexer.KwAnd, lexer.KwOr, lexer.KwThen:
		return true
	}
	return false
}

func blockKeyword(k lexer.Kind) bool {
	switch k {
	case lexer.KwFn, lexer.KwIf, lexer.KwMatch, lexer.KwFor, lexer.KwTest:
		return true
	}
	return false
}

// fnHead reports whether the `fn` at toks[i] starts a function, which is
// followed by its name or `(` (01). Otherwise it is the type name `fn`
// (07: `h: fn`, `-> fn`), which waits for no `:` — a `:` of a later
// annotation or the function's own block colon must not be taken for it.
func fnHead(toks []lexer.Token, i int) bool {
	if i+1 >= len(toks) {
		return false
	}
	k := toks[i+1].Kind
	return k == lexer.Name || k == lexer.LParen
}

func isClosing(k lexer.Kind) bool { return k == lexer.DoubleSemi || isCloser(k) }

// patternStart reports a token a match pattern can begin with (01).
func patternStart(k lexer.Kind) bool {
	switch k {
	case lexer.Name, lexer.Tag, lexer.Int, lexer.Float, lexer.String, lexer.RawString, lexer.Symbol,
		lexer.Duration, lexer.Minus, lexer.LParen, lexer.LBracket, lexer.LBrace,
		lexer.KwTrue, lexer.KwFalse, lexer.KwNil:
		return true
	}
	return false
}

// hasTopArrow reports whether the line reads as a match arm head, the
// parser's rule (01): `pattern [when expr] =>` — a pattern-shaped start,
// a `=>` outside brackets, and no `=` before it (`g = x => x` is a
// statement that happens to hold a lambda).
func hasTopArrow(toks []lexer.Token) (bool, int) {
	if len(toks) == 0 || !patternStart(toks[0].Kind) {
		return false, -1
	}
	depth := 0
	for i, t := range toks {
		switch {
		case isOpener(t.Kind):
			depth++
		case isCloser(t.Kind):
			depth--
		case t.Kind == lexer.Assign && depth == 0:
			return false, -1
		case t.Kind == lexer.FatArrow && depth == 0:
			return true, i
		}
	}
	return false, -1
}

// layout decides every code line's indent, then gives comment lines the
// indent of the code line that follows them (inside a closer's block
// when that line closes one), or of the last code line after it.
func layout(lines []line) {
	l := &layouter{thens: []int{0}}
	for i := range lines {
		ln := &lines[i]
		if len(ln.toks) == 0 {
			continue
		}
		ln.indent = l.lineIndent(ln)
		l.consume(ln)
		l.endLine(ln)
	}
	last := -1
	for i := range lines {
		if len(lines[i].toks) > 0 {
			last = i
		}
	}
	next := 0
	if last >= 0 {
		next = lines[last].indent
	}
	for i := len(lines) - 1; i >= 0; i-- {
		if len(lines[i].toks) > 0 {
			next = lines[i].indent
			if isClosing(lines[i].toks[0].Kind) {
				next++
			}
			continue
		}
		if lines[i].isComment() {
			lines[i].indent = next
		}
	}
}

// lineIndent decides the indent of a code line from the state at its start.
func (l *layouter) lineIndent(ln *line) int {
	first := ln.toks[0]
	in := l.innermost()
	// A closing token drops to the line that opened what it closes.
	if isClosing(first.Kind) {
		if in != nil {
			return in.base
		}
		return 0
	}
	// An if-chain `else` sits with its `if`.
	if first.Kind == lexer.KwElse && !l.thenElse() {
		if in != nil && in.block && in.kind == lexer.KwIf {
			return in.base
		}
	}
	indent := 0
	if in != nil {
		indent = in.base + 1
	}
	// A continuation line goes one deeper than its statement — unless a
	// bracket already indents it (every line inside brackets sits at +1).
	inBracket := in != nil && !in.block
	if l.continues(first.Kind) && !inBracket {
		return indent + 1
	}
	// Inside a match block a line that is not an arm belongs to the
	// previous arm's body.
	if in != nil && in.block && in.kind == lexer.KwMatch {
		if arm, _ := hasTopArrow(ln.toks); !arm {
			return indent + 1
		}
	}
	return indent
}

// consume updates the state with a line's tokens.
func (l *layouter) consume(ln *line) {
	first := ln.toks[0]
	if !l.continues(first.Kind) {
		// A new statement: forget block keywords that never got their `:`
		// (fn f() = e) and unmatched thens at this bracket depth.
		d := l.brackets()
		for len(l.pend) > 0 && l.pend[len(l.pend)-1].depth == d {
			l.pend = l.pend[:len(l.pend)-1]
		}
		l.thens[d] = 0
	}
	if arm, i := hasTopArrow(ln.toks); arm {
		if in := l.innermost(); in != nil && in.block && in.kind == lexer.KwMatch {
			ln.arrow = i
		}
	}
	for i, t := range ln.toks {
		d := l.brackets()
		switch {
		case isOpener(t.Kind):
			l.stack = append(l.stack, opener{kind: t.Kind, base: ln.indent})
			l.thens = append(l.thens, 0)
		case isCloser(t.Kind):
			l.popBracket()
		case t.Kind == lexer.DoubleSemi:
			l.popBlock()
		case t.Kind == lexer.KwThen:
			l.thens[d]++
		case t.Kind == lexer.KwElse:
			if l.thens[d] > 0 {
				l.thens[d]--
				continue
			}
			// if-chain: the if block ends here and `else …:` reopens one.
			if in := l.innermost(); in != nil && in.block && in.kind == lexer.KwIf {
				l.stack = l.stack[:len(l.stack)-1]
			}
			l.pend = append(l.pend, pending{kind: lexer.KwIf, depth: d})
		case blockKeyword(t.Kind) && !(t.Kind == lexer.KwFn && !fnHead(ln.toks, i)):
			l.pend = append(l.pend, pending{kind: t.Kind, depth: d})
		case t.Kind == lexer.Colon:
			n := len(l.pend)
			if n == 0 || l.pend[n-1].depth != d {
				continue
			}
			p := l.pend[n-1]
			l.pend = l.pend[:n-1]
			// `else if c:` leaves the `else` pending behind: drop it.
			for len(l.pend) > 0 && l.pend[len(l.pend)-1].depth == d && l.pend[len(l.pend)-1].kind == lexer.KwIf && p.kind == lexer.KwIf {
				l.pend = l.pend[:len(l.pend)-1]
			}
			l.stack = append(l.stack, opener{kind: p.kind, block: true, base: ln.indent})
			if i == len(ln.toks)-1 {
				ln.opensBlock = true
			}
		}
	}
}

// endLine records whether the next line continues this one.
func (l *layouter) endLine(ln *line) {
	l.contNext = endsOpen(ln.toks[len(ln.toks)-1].Kind)
}

func (l *layouter) popBracket() {
	for n := len(l.stack); n > 0; n-- {
		if !l.stack[n-1].block {
			l.stack = l.stack[:n-1]
			if len(l.thens) > 1 {
				l.thens = l.thens[:len(l.thens)-1]
			}
			return
		}
	}
}

func (l *layouter) popBlock() {
	for n := len(l.stack); n > 0; n-- {
		if l.stack[n-1].block {
			l.stack = l.stack[:n-1]
			return
		}
	}
}
