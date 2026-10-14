// Package format is the fung formatter (08): it rewrites a file's
// whitespace, indentation, blank lines and alignment from the lexer's
// tokens and comments, keeping the user's line breaks.
package format

import (
	"strings"

	"github.com/tunarider/fung/internal/lexer"
)

// line is one source line: its tokens (never Newline), an optional
// trailing comment, and what layout decides for it. A blank line has
// neither tokens nor comment; a comment line has only the comment.
type line struct {
	toks       []lexer.Token
	comment    string // trailing comment text ("" for none), or a comment line's text
	srcLine    int
	indent     int  // in levels, decided by layout
	arrow      int  // index of the `=>` token to align, -1 for none
	opensBlock bool // the last token is a block-opening `:` (blank-line rule)
}

func (l *line) blank() bool     { return len(l.toks) == 0 && l.comment == "" }
func (l *line) isComment() bool { return len(l.toks) == 0 && l.comment != "" }

// splitLines groups tokens by the source line they start on and attaches
// comments: a comment on a line that has tokens is its trailing comment,
// otherwise it is a comment line. Lines inside a multi-line token (a raw
// string) are not lines of their own, and a final newline makes no line.
func splitLines(src string, toks []lexer.Token, comments []lexer.Comment) []line {
	byLine := map[int]*line{}
	covered := map[int]bool{}
	owner := map[int]int{} // a line inside a multi-line token → the token's first line
	last := 0
	add := func(n int) *line {
		if o, ok := owner[n]; ok {
			n = o // what follows a raw string on its last line belongs to the line it started on
		}
		if l, ok := byLine[n]; ok {
			return l
		}
		l := &line{srcLine: n, arrow: -1}
		byLine[n] = l
		if n > last {
			last = n
		}
		return l
	}
	for _, t := range toks {
		if t.Kind == lexer.Newline || t.Kind == lexer.EOF {
			continue
		}
		l := add(t.Pos.Line)
		l.toks = append(l.toks, t)
		for i := 1; i <= strings.Count(src[t.Off:t.End], "\n"); i++ {
			covered[t.Pos.Line+i] = true
			owner[t.Pos.Line+i] = l.srcLine
		}
	}
	for _, c := range comments {
		add(c.Pos.Line).comment = c.Text
	}
	total := strings.Count(src, "\n")
	if !strings.HasSuffix(src, "\n") && src != "" {
		total++ // the last line has no newline but has content
	}
	if total < last {
		total = last
	}
	var out []line
	for n := 1; n <= total; n++ {
		if covered[n] {
			continue
		}
		if l, ok := byLine[n]; ok {
			out = append(out, *l)
		} else {
			out = append(out, line{srcLine: n, arrow: -1})
		}
	}
	return out
}
