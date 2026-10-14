package format

import (
	"strings"
	"unicode/utf8"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/parser"
)

// Format rewrites src in the single fung style (08). A syntax error comes
// back as is and nothing is formatted; an output whose tokens differ from
// the input is an internal error, never returned as text.
func Format(file, src string) (string, *diag.Error) {
	toks, comments, err := lexer.Lex(file, src)
	if err != nil {
		return "", err
	}
	if _, perr := parser.Parse(toks); perr != nil {
		return "", perr
	}
	lines := splitLines(src, toks, comments)
	layout(lines)
	lines = tidyBlank(lines)
	out := emit(src, lines)
	if !same(file, src, toks, comments, out) {
		return "", diag.Errorf("E0801", diag.Pos{}, "formatter changed the code of "+file)
	}
	return out, nil
}

// render joins a line's tokens with the spacing table. arrow is the rune
// offset of the line's alignable `=>` in the result, -1 for none.
func render(src string, ln *line) (code string, arrow int) {
	var b strings.Builder
	arrow = -1
	sign := false
	importLine := len(ln.toks) > 0 && ln.toks[0].Kind == lexer.KwImport
	for i, t := range ln.toks {
		if i > 0 {
			for n := gap(ln.toks[i-1], t, sign, importLine); n > 0; n-- {
				b.WriteByte(' ')
			}
		}
		if i == ln.arrow {
			arrow = utf8.RuneCountInString(b.String())
		}
		b.WriteString(src[t.Off:t.End])
		sign = t.Kind == lexer.Minus && (i == 0 || prefixContext(ln.toks[i-1].Kind))
		if sign && i >= 2 && ln.toks[i-1].Kind.IsKeyword() && ln.toks[i-2].Kind == lexer.Dot {
			sign = false // m.type - 1: the keyword is a field name here (01)
		}
	}
	return b.String(), arrow
}

// trimComment drops trailing whitespace from a comment (spec: 줄 끝 공백은 없다).
func trimComment(c string) string { return strings.TrimRight(c, " \t") }

// emit assembles the lines: indentation, code, trailing comment.
func emit(src string, lines []line) string {
	rs := make([]rendered, len(lines))
	for i := range lines {
		ln := &lines[i]
		code, arrow := render(src, ln)
		closing := len(ln.toks) > 0 && isClosing(ln.toks[0].Kind)
		rs[i] = rendered{indent: ln.indent, code: code, comment: trimComment(ln.comment), arrow: arrow, blank: ln.blank(), closing: closing}
	}
	align(rs)
	var b strings.Builder
	for _, r := range rs {
		if r.blank {
			b.WriteByte('\n')
			continue
		}
		b.WriteString(strings.Repeat("  ", r.indent))
		b.WriteString(r.code)
		if r.comment != "" {
			if r.code != "" {
				b.WriteString("  ")
			}
			b.WriteString(r.comment)
		}
		b.WriteByte('\n')
	}
	return b.String()
}

// same checks the safety property: the output lexes to the same tokens
// and comments as the input, newlines aside.
func same(file, src string, toks []lexer.Token, comments []lexer.Comment, out string) bool {
	toks2, comments2, err := lexer.Lex(file, out)
	if err != nil || len(comments) != len(comments2) {
		return false
	}
	for i := range comments {
		if trimComment(comments[i].Text) != trimComment(comments2[i].Text) {
			return false
		}
	}
	a, b := code(src, toks), code(out, toks2)
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func code(src string, toks []lexer.Token) []string {
	var out []string
	for _, t := range toks {
		if t.Kind != lexer.Newline && t.Kind != lexer.EOF {
			out = append(out, src[t.Off:t.End])
		}
	}
	return out
}

// rendered is a line ready for assembly.
type rendered struct {
	indent  int
	code    string
	comment string
	arrow   int // rune offset of a `=>` to align, -1 for none
	blank   bool
	closing bool // starts with `;;` or a closing bracket
}

// tidyBlank collapses runs of blank lines to one and removes them at the
// start and end of the file, right after a block-opening `:` and right
// before a `;;` (spec: 빈 줄과 파일 끝).
func tidyBlank(lines []line) []line {
	var out []line
	for i := range lines {
		ln := lines[i]
		if ln.blank() {
			if len(out) == 0 || out[len(out)-1].blank() || out[len(out)-1].opensBlock {
				continue
			}
			if next := nextCode(lines, i); next != nil && next.toks[0].Kind == lexer.DoubleSemi {
				continue
			}
		}
		out = append(out, ln)
	}
	for len(out) > 0 && out[len(out)-1].blank() {
		out = out[:len(out)-1]
	}
	return out
}

// nextCode is the next code line after index from, skipping blank lines
// only; a comment line ends the search.
func nextCode(lines []line, from int) *line {
	for j := from + 1; j < len(lines); j++ {
		if len(lines[j].toks) > 0 {
			return &lines[j]
		}
		if !lines[j].blank() {
			return nil
		}
	}
	return nil
}
