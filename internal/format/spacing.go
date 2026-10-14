package format

import "github.com/tunarider/fung/internal/lexer"

func isOpener(k lexer.Kind) bool {
	return k == lexer.LParen || k == lexer.LBracket || k == lexer.LBrace
}

func isCloser(k lexer.Kind) bool {
	return k == lexer.RParen || k == lexer.RBracket || k == lexer.RBrace
}

// isValueEnd reports a token an index may follow directly: xs[0],
// "s"[0], f(x)[0].
func isValueEnd(k lexer.Kind) bool {
	switch k {
	case lexer.Name, lexer.Tag, lexer.RParen, lexer.RBracket, lexer.String, lexer.RawString:
		return true
	}
	return false
}

// isCallee reports a token a call may follow directly: f(x), Ok(x),
// f(x)(y), xs[0](y). Strings are not callable, so `import "./x" (a)`
// keeps its space.
func isCallee(k lexer.Kind) bool {
	switch k {
	case lexer.Name, lexer.Tag, lexer.RParen, lexer.RBracket:
		return true
	}
	return false
}

// prefixContext reports a token after which `-` is a sign, not a
// subtraction: operators, openers, separators, arrows, keywords.
func prefixContext(k lexer.Kind) bool {
	if isOpener(k) || k.IsKeyword() {
		return true
	}
	switch k {
	case lexer.Comma, lexer.Colon, lexer.Semi, lexer.DoubleSemi, lexer.FatArrow, lexer.Arrow, lexer.Assign,
		lexer.Eq, lexer.Ne, lexer.Lt, lexer.Gt, lexer.Le, lexer.Ge, lexer.Plus, lexer.Minus, lexer.Star,
		lexer.StarStar, lexer.Slash, lexer.SlashSlash, lexer.Percent, lexer.Pipe, lexer.Bar, lexer.Coalesce,
		lexer.DotDot, lexer.DotDotEq:
		return true
	}
	return false
}

// gap is the number of spaces between two adjacent tokens on a line
// (spec: 토큰 사이 공백). prevIsSign says prev is a prefix `-`; importLine
// says the line is an import, whose name list is not a call.
func gap(prev, next lexer.Token, prevIsSign, importLine bool) int {
	p, n := prev.Kind, next.Kind
	switch {
	case importLine && n == lexer.LParen:
		return 1
	case isOpener(p), isCloser(n):
		return 0
	case n == lexer.Comma, n == lexer.Semi, n == lexer.Colon:
		return 0
	case p == lexer.Dot, n == lexer.Dot, p == lexer.DotDot, n == lexer.DotDot, p == lexer.DotDotEq, n == lexer.DotDotEq:
		return 0
	case p == lexer.Ellipsis:
		return 0
	case n == lexer.Bang, n == lexer.Question:
		return 0
	case n == lexer.LParen && (isCallee(p) || p == lexer.KwFn):
		return 0
	case n == lexer.LBracket && isValueEnd(p):
		return 0
	case prevIsSign:
		return 0
	case p == lexer.DoubleSemi && isCloser(n):
		return 0
	}
	return 1
}
