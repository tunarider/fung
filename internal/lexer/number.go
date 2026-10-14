package lexer

import (
	"math"
	"strconv"
	"strings"
	"time"

	"github.com/tunarider/fung/internal/diag"
)

// number lexes integers (decimal, 0x, 0o, 0b, with _ separators), floats
// (digits on both sides of the point, optional exponent) and durations
// (number immediately followed by ms/s/m/h, possibly compound: 1h30m).
func (lx *lexer) number() *diag.Error {
	p := lx.pos()
	start := lx.off

	if lx.peek() == '0' {
		switch lx.peekAt(1) {
		case 'x', 'X', 'o', 'O', 'b', 'B':
			return lx.prefixedInt(p, start)
		}
	}

	if !lx.digits() {
		return diag.Errorf("E0102", p, lx.src[start:lx.off])
	}
	isFloat := false
	if lx.peek() == '.' && isDigit(lx.peekAt(1)) {
		lx.advance()
		isFloat = true
		if !lx.digits() {
			return diag.Errorf("E0102", p, lx.src[start:lx.off])
		}
	}
	hasExp := false
	if c := lx.peek(); c == 'e' || c == 'E' {
		next := lx.peekAt(1)
		if isDigit(next) || ((next == '+' || next == '-') && isDigit(lx.peekAt(2))) {
			lx.advance()
			if lx.peek() == '+' || lx.peek() == '-' {
				lx.advance()
			}
			if !lx.digits() {
				return diag.Errorf("E0102", p, lx.src[start:lx.off])
			}
			isFloat, hasExp = true, true
		}
	}

	// A letter right after the number means either a duration unit or junk.
	if isIdentStart(lx.peek()) {
		if hasExp {
			return diag.Errorf("E0102", p, lx.src[start:lx.off]+lx.identText())
		}
		return lx.duration(p, start)
	}

	text := strings.ReplaceAll(lx.src[start:lx.off], "_", "")
	if isFloat {
		f, err := strconv.ParseFloat(text, 64)
		if err != nil {
			return diag.Errorf("E0102", p, lx.src[start:lx.off])
		}
		tok := lx.emit(Float, lx.src[start:lx.off], p)
		tok.Float = f
		return nil
	}
	n, err := strconv.ParseInt(text, 10, 64)
	if err != nil {
		if text == "9223372036854775808" {
			// The magnitude of the smallest int. Only the parser can tell
			// whether a unary minus precedes it (01).
			tok := lx.emit(Int, lx.src[start:lx.off], p)
			tok.Int = math.MinInt64
			return nil
		}
		return diag.Errorf("E0102", p, lx.src[start:lx.off])
	}
	tok := lx.emit(Int, lx.src[start:lx.off], p)
	tok.Int = n
	return nil
}

// digits consumes `digit (digit | _ digit)*` and reports whether at least
// one digit was read and no `_` was left dangling or doubled.
func (lx *lexer) digits() bool {
	if !isDigit(lx.peek()) {
		return false
	}
	for {
		lx.advance()
		if lx.peek() == '_' {
			if !isDigit(lx.peekAt(1)) {
				lx.advance() // consume so the error text shows it
				return false
			}
			lx.advance()
			continue
		}
		if !isDigit(lx.peek()) {
			return true
		}
	}
}

func (lx *lexer) prefixedInt(p diag.Pos, start int) *diag.Error {
	lx.advance() // 0
	base := 16
	switch lx.advance() {
	case 'o', 'O':
		base = 8
	case 'b', 'B':
		base = 2
	}
	digStart := lx.off
	for isHexDigit(lx.peek()) || (lx.peek() == '_' && isHexDigit(lx.peekAt(1))) {
		lx.advance()
	}
	if lx.off == digStart || isIdentStart(lx.peek()) || isDigit(lx.peek()) {
		lx.identText()
		return diag.Errorf("E0102", p, lx.src[start:lx.off])
	}
	text := strings.ReplaceAll(lx.src[digStart:lx.off], "_", "")
	n, err := strconv.ParseInt(text, base, 64)
	if err != nil {
		return diag.Errorf("E0102", p, lx.src[start:lx.off])
	}
	tok := lx.emit(Int, lx.src[start:lx.off], p)
	tok.Int = n
	return nil
}

func isHexDigit(c rune) bool {
	return isDigit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
}

// identText consumes identifier characters and returns them (used to make
// error messages show the whole bad token, e.g. "30abc").
func (lx *lexer) identText() string {
	start := lx.off
	for isIdentPart(lx.peek()) {
		lx.advance()
	}
	return lx.src[start:lx.off]
}

var durationUnits = map[string]struct {
	rank int
	unit time.Duration
}{
	"h": {3, time.Hour}, "m": {2, time.Minute}, "s": {1, time.Second}, "ms": {0, time.Millisecond},
}

// duration continues after the first numeric component has been consumed
// (lx.off points at its unit letters). Components must use descending
// units: 1h30m is fine, 30m1h is not. A first component whose suffix is
// not a unit at all (30abc, 5x) is a malformed number, not a duration.
func (lx *lexer) duration(p diag.Pos, start int) *diag.Error {
	bad := func(code string) *diag.Error {
		lx.identText()
		return diag.Errorf(code, p, lx.src[start:lx.off])
	}
	var total time.Duration
	prevRank := 4
	compStart := start
	first := true
	for {
		numText := strings.ReplaceAll(lx.src[compStart:lx.off], "_", "")
		unitStart := lx.off
		for c := lx.peek(); c >= 'a' && c <= 'z'; c = lx.peek() {
			lx.advance()
		}
		u, ok := durationUnits[lx.src[unitStart:lx.off]]
		if !ok && first {
			return bad("E0102")
		}
		if !ok || u.rank >= prevRank {
			return bad("E0105")
		}
		f, err := strconv.ParseFloat(numText, 64)
		if err != nil {
			return bad("E0105")
		}
		// Round to the nearest nanosecond (4.35h is exactly 15660s, but the
		// float product lands one ns short) and refuse anything the int64
		// duration cannot hold: Go's out-of-range float→int conversion is
		// implementation-defined, so it must never reach time.Duration.
		ns := math.Round(f * float64(u.unit))
		if ns > float64(math.MaxInt64) || ns > float64(math.MaxInt64-total) {
			return bad("E0105")
		}
		total += time.Duration(ns)
		prevRank = u.rank
		first = false
		if !isDigit(lx.peek()) {
			break
		}
		compStart = lx.off
		if !lx.digits() {
			return bad("E0105")
		}
		if lx.peek() == '.' && isDigit(lx.peekAt(1)) {
			lx.advance()
			lx.digits()
		}
		if !isIdentStart(lx.peek()) {
			return bad("E0105")
		}
	}
	if isIdentPart(lx.peek()) {
		return bad("E0105")
	}
	tok := lx.emit(Duration, lx.src[start:lx.off], p)
	tok.Dur = total
	return nil
}
