package lexer

import (
	"strconv"
	"strings"

	"github.com/tunarider/fung/internal/diag"
)

// str lexes a "..." string into text and ${code} parts. Escapes are the
// closed list from 01; anything else is E0103. A literal newline inside
// the quotes is E0104 (use a raw string or \n).
func (lx *lexer) str() *diag.Error {
	p := lx.pos()
	start := lx.off
	lx.advance() // opening quote
	tok := Token{Kind: String, Pos: p}
	var text strings.Builder
	flush := func() {
		if text.Len() > 0 {
			tok.Parts = append(tok.Parts, Part{Text: text.String()})
			text.Reset()
		}
	}
	for {
		c := lx.peek()
		switch {
		case c == eof || c == '\n':
			return diag.Errorf("E0104", p)
		case c == '"':
			lx.advance()
			flush()
			tok.Text = lx.src[start:lx.off]
			tok.Off, tok.End = lx.base+start, lx.base+lx.off
			lx.toks = append(lx.toks, tok)
			return nil
		case c == '\\':
			r, err := lx.escape()
			if err != nil {
				return err
			}
			text.WriteRune(r)
		case c == '$' && lx.peekAt(1) == '{':
			lx.advance()
			lx.advance()
			flush()
			codePos := lx.pos()
			codeStart := lx.off
			if err := lx.skipInterpolation(p); err != nil {
				return err
			}
			tok.Parts = append(tok.Parts, Part{IsCode: true, Text: lx.src[codeStart : lx.off-1], Pos: codePos, Off: lx.base + codeStart})
		default:
			text.WriteRune(lx.advance())
		}
	}
}

// escape decodes one backslash escape; lx.off is at the backslash.
func (lx *lexer) escape() (rune, *diag.Error) {
	p := lx.pos()
	lx.advance() // backslash
	c := lx.advance()
	switch c {
	case 'n':
		return '\n', nil
	case 't':
		return '\t', nil
	case 'r':
		return '\r', nil
	case '\\':
		return '\\', nil
	case '"':
		return '"', nil
	case '$':
		return '$', nil
	case 'u':
		if lx.peek() != '{' {
			return 0, diag.Errorf("E0103", p, "u")
		}
		lx.advance()
		hexStart := lx.off
		for isHexDigit(lx.peek()) {
			lx.advance()
		}
		hex := lx.src[hexStart:lx.off]
		if lx.peek() != '}' || len(hex) == 0 || len(hex) > 6 {
			return 0, diag.Errorf("E0103", p, "u{"+hex)
		}
		lx.advance()
		n, err := strconv.ParseUint(hex, 16, 32)
		if err != nil || n > 0x10FFFF || (n >= 0xD800 && n <= 0xDFFF) {
			return 0, diag.Errorf("E0103", p, "u{"+hex+"}")
		}
		return rune(n), nil
	}
	return 0, diag.Errorf("E0103", p, string(c))
}

// skipInterpolation consumes the code of a ${...} up to and including its
// closing brace, tracking nested braces and skipping over nested string
// literals so a quote inside them does not end the outer string.
func (lx *lexer) skipInterpolation(strPos diag.Pos) *diag.Error {
	depth := 1
	for {
		switch c := lx.peek(); c {
		case eof:
			return diag.Errorf("E0104", strPos)
		case '{':
			depth++
			lx.advance()
		case '}':
			depth--
			lx.advance()
			if depth == 0 {
				return nil
			}
		case '"':
			if err := lx.skipQuoted(strPos); err != nil {
				return err
			}
		case '`':
			lx.advance()
			for lx.peek() != '`' {
				if lx.peek() == eof {
					return diag.Errorf("E0104", strPos)
				}
				lx.advance()
			}
			lx.advance()
		default:
			lx.advance()
		}
	}
}

// skipQuoted skips a nested "..." literal inside an interpolation,
// honoring escapes and its own ${} parts.
func (lx *lexer) skipQuoted(strPos diag.Pos) *diag.Error {
	lx.advance() // opening quote
	for {
		switch c := lx.peek(); c {
		case eof, '\n':
			return diag.Errorf("E0104", strPos)
		case '\\':
			lx.advance()
			lx.advance()
		case '"':
			lx.advance()
			return nil
		case '$':
			lx.advance()
			if lx.peek() == '{' {
				lx.advance()
				if err := lx.skipInterpolation(strPos); err != nil {
					return err
				}
			}
		default:
			lx.advance()
		}
	}
}

// rawStr lexes a `...` string: no escapes, no interpolation, newlines allowed.
func (lx *lexer) rawStr() *diag.Error {
	p := lx.pos()
	lx.advance()
	start := lx.off
	for lx.peek() != '`' {
		if lx.peek() == eof {
			return diag.Errorf("E0104", p)
		}
		lx.advance()
	}
	text := lx.src[start:lx.off]
	lx.advance()
	lx.emit(RawString, text, p)
	return nil
}

// symbol lexes 'name.
func (lx *lexer) symbol() *diag.Error {
	p := lx.pos()
	lx.advance()
	if !isIdentStart(lx.peek()) {
		return diag.Errorf("E0101", p, "'")
	}
	start := lx.off
	for isIdentPart(lx.peek()) {
		lx.advance()
	}
	lx.emit(Symbol, lx.src[start:lx.off], p)
	return nil
}
