// Package lexer turns fung source text into tokens. It implements all of
// the lexical rules in 01. 문법 (글자 수준 규칙): newlines are tokens that
// the parser interprets, comments are kept aside as trivia for the
// formatter, and only the first error is reported.
package lexer

import (
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/tunarider/fung/internal/diag"
)

type lexer struct {
	file     string
	src      string
	off      int // byte offset of the next rune
	start    int // byte offset where the token being lexed began
	base     int // offset of src inside the whole file (interpolated code), added to spans
	line     int
	col      int // rune column of the next rune, 1-based
	toks     []Token
	comments []Comment
	keywords map[string]Kind
}

// eof is what peek returns past the end of input. It is outside the rune
// range so a literal U+0000 in the source is not mistaken for the end.
const eof rune = -1

// Lex turns source text into tokens. The last token is always EOF. Only
// the first error is reported (08: 문법 에러는 첫 번째 하나만).
// A leading byte order mark (U+FEFF, written by some Windows editors) is
// skipped and does not count as a column.
func Lex(file, src string) ([]Token, []Comment, *diag.Error) {
	src = strings.TrimPrefix(src, "\ufeff")
	return LexAt(file, src, diag.Pos{File: file, Line: 1, Col: 1}, 0)
}

// LexAt lexes text that sits inside a larger file, such as the code of a
// ${...} interpolation, so that positions refer to the enclosing file.
func LexAt(file, src string, start diag.Pos, base int) ([]Token, []Comment, *diag.Error) {
	lx := &lexer{file: file, src: src, line: start.Line, col: start.Col, base: base, keywords: Keywords()}
	if err := lx.run(); err != nil {
		return nil, nil, err
	}
	return lx.toks, lx.comments, nil
}

func (lx *lexer) pos() diag.Pos { return diag.Pos{File: lx.file, Line: lx.line, Col: lx.col} }

// peek returns the next rune without consuming it, or eof at end of input.
func (lx *lexer) peek() rune { return lx.peekAt(0) }

// peekAt returns the n-th rune ahead (0 = next), or eof past the end.
func (lx *lexer) peekAt(n int) rune {
	off := lx.off
	for {
		if off >= len(lx.src) {
			return eof
		}
		r, size := utf8.DecodeRuneInString(lx.src[off:])
		if n == 0 {
			return r
		}
		off += size
		n--
	}
}

func (lx *lexer) advance() rune {
	r, size := utf8.DecodeRuneInString(lx.src[lx.off:])
	lx.off += size
	if r == '\n' {
		lx.line++
		lx.col = 1
	} else {
		lx.col++
	}
	return r
}

func (lx *lexer) emit(kind Kind, text string, pos diag.Pos) *Token {
	lx.toks = append(lx.toks, Token{Kind: kind, Text: text, Pos: pos, Off: lx.base + lx.start, End: lx.base + lx.off})
	return &lx.toks[len(lx.toks)-1]
}

func (lx *lexer) run() *diag.Error {
	for {
		c := lx.peek()
		lx.start = lx.off
		switch {
		case c == eof:
			lx.emit(EOF, "", lx.pos())
			return nil
		case c == '\r' && lx.peekAt(1) == '\n':
			// Windows line ending: one newline token at the \r.
			p := lx.pos()
			lx.advance()
			lx.advance()
			lx.emit(Newline, "\n", p)
		case c == ' ' || c == '\t' || c == '\r':
			lx.advance()
		case c == '\n':
			p := lx.pos()
			lx.advance()
			lx.emit(Newline, "\n", p)
		case c == '#':
			lx.comment()
		case isDigit(c):
			if err := lx.number(); err != nil {
				return err
			}
		case c == '"':
			if err := lx.str(); err != nil {
				return err
			}
		case c == '`':
			if err := lx.rawStr(); err != nil {
				return err
			}
		case c == '\'':
			if err := lx.symbol(); err != nil {
				return err
			}
		case isIdentStart(c):
			lx.ident()
		default:
			if err := lx.punct(); err != nil {
				return err
			}
		}
	}
}

func (lx *lexer) comment() {
	p := lx.pos()
	start := lx.off
	for lx.peek() != eof && lx.peek() != '\n' && lx.peek() != '\r' {
		lx.advance()
	}
	lx.comments = append(lx.comments, Comment{Pos: p, Text: lx.src[start:lx.off]})
}

func (lx *lexer) ident() {
	p := lx.pos()
	start := lx.off
	first := lx.advance()
	for isIdentPart(lx.peek()) {
		lx.advance()
	}
	text := lx.src[start:lx.off]
	kind := Name
	if k, ok := lx.keywords[text]; ok {
		kind = k
	} else if unicode.IsUpper(first) {
		kind = Tag
	}
	lx.emit(kind, text, p)
}

// punct tries the longest operator first so that `..=` wins over `..`
// and `;;` over `;` (01: `;;`는 렉서가 항상 가장 길게 자른다).
func (lx *lexer) punct() *diag.Error {
	p := lx.pos()
	r0, r1, r2 := lx.peek(), lx.peekAt(1), lx.peekAt(2)
	if k, ok := punct3[string([]rune{r0, r1, r2})]; ok {
		lx.advance()
		lx.advance()
		lx.advance()
		lx.emit(k, string([]rune{r0, r1, r2}), p)
		return nil
	}
	if k, ok := punct2[string([]rune{r0, r1})]; ok {
		lx.advance()
		lx.advance()
		lx.emit(k, string([]rune{r0, r1}), p)
		return nil
	}
	if k, ok := punct1[r0]; ok {
		lx.advance()
		lx.emit(k, string(r0), p)
		return nil
	}
	return diag.Errorf("E0101", p, string(r0))
}

var punct3 = map[string]Kind{"..=": DotDotEq, "...": Ellipsis}

var punct2 = map[string]Kind{
	"==": Eq, "!=": Ne, "<=": Le, ">=": Ge, "=>": FatArrow, "->": Arrow,
	"//": SlashSlash, "**": StarStar, "|>": Pipe, "??": Coalesce, "..": DotDot, ";;": DoubleSemi,
}

var punct1 = map[rune]Kind{
	'(': LParen, ')': RParen, '[': LBracket, ']': RBracket, '{': LBrace, '}': RBrace,
	',': Comma, ':': Colon, ';': Semi, '.': Dot, '=': Assign, '<': Lt, '>': Gt,
	'+': Plus, '-': Minus, '*': Star, '/': Slash, '%': Percent, '!': Bang, '?': Question, '|': Bar,
}

func isDigit(c rune) bool      { return c >= '0' && c <= '9' }
func isIdentStart(c rune) bool { return c == '_' || unicode.IsLetter(c) }
func isIdentPart(c rune) bool  { return c == '_' || unicode.IsLetter(c) || unicode.IsDigit(c) }
