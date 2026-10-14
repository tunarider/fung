package diag

import (
	"fmt"
	"strconv"
	"strings"
)

// Error is every error fung reports to the user: a catalog number, where
// it happened, how many characters to highlight, the values that fill the
// message template, and an optional per-error help line. The message text
// itself lives in the catalog so it can be translated and kept stable.
type Error struct {
	Code     string
	Pos      Pos
	Len      int // characters to underline; 0 means 1
	Args     []any
	Help     string     // overrides the catalog hint when set
	CallPath []CallSite // innermost first; set by the VM for runtime errors
	Notes    []string   // extra lines after the excerpt (assert shows operand values, 08)
}

// CallSite is one frame of the call path shown with runtime errors (08).
type CallSite struct {
	File   string
	Line   int
	Fn     string // "<main>" for the entry file
	Elided int    // tail calls that reused this frame
}

// Errorf builds an Error for a catalog code.
func Errorf(code string, pos Pos, args ...any) *Error {
	return &Error{Code: code, Pos: pos, Args: args}
}

func (e *Error) Error() string {
	if e.Code == CodeExit {
		return fmt.Sprintf("exit(%d)", e.Args[0])
	}
	if strings.HasPrefix(e.Code, "W") {
		return fmt.Sprintf("warning[%s]: %s", e.Code, Message(e.Code, e.Args...))
	}
	return fmt.Sprintf("error[%s]: %s", e.Code, Message(e.Code, e.Args...))
}

// CodeExit marks the exit signal that exit(n) raises (06): it unwinds
// like an error but is not one, so it has no catalog entry, no excerpt,
// and no call path in reports.
const CodeExit = "exit"

// ExitSignal builds the signal for exit(code).
func ExitSignal(code int) *Error { return &Error{Code: CodeExit, Args: []any{code}} }

// ExitCode reports the code when the error is an exit signal.
func (e *Error) ExitCode() (int, bool) {
	if e.Code != CodeExit {
		return 0, false
	}
	return e.Args[0].(int), true
}

// WithPos fills in the position if the error does not have one yet.
// Value operations raise errors without knowing where they were called
// from; the VM adds the position of the current instruction.
func (e *Error) WithPos(p Pos) *Error {
	if e.Pos.IsZero() {
		e.Pos = p
	}
	return e
}

// Span sets how many characters the excerpt underlines.
func (e *Error) Span(n int) *Error {
	e.Len = n
	return e
}

// WithHelp sets a help line specific to this error.
func (e *Error) WithHelp(h string) *Error {
	e.Help = h
	return e
}

// WithCallPath records the call path if none was recorded yet. The
// innermost run loop records it; outer loops leave it alone.
func (e *Error) WithCallPath(p []CallSite) *Error {
	if len(e.CallPath) == 0 {
		e.CallPath = p
	}
	return e
}

// Format renders an error the way the fung command prints it (08):
//
//	error[E0204]: undefined name `fiels`
//	  --> rename.fung:12:12
//	   |
//	12 |   for f in fiels:
//	   |            ^^^^^ not found in this scope
//	   |
//	 help: a similar name exists: `files`
//
// When src (the file's text) is given and the position is known, the
// offending line is shown with the span underlined. Call paths are added
// in a later milestone.
func Format(e *Error, src string) string {
	var b strings.Builder
	b.WriteString(e.Error())
	b.WriteByte('\n')
	if !e.Pos.IsZero() {
		fmt.Fprintf(&b, "  --> %s\n", e.Pos)
		if src != "" {
			writeExcerpt(&b, e, src)
		}
	}
	for _, note := range e.Notes {
		fmt.Fprintf(&b, "  %s\n", note)
	}
	if len(e.CallPath) > 0 {
		writeCallPath(&b, e.CallPath)
	}
	help := e.Help
	if help == "" {
		help = Hint(e.Code)
	}
	if help != "" {
		fmt.Fprintf(&b, " help: %s\n", help)
	}
	return b.String()
}

const tabWidth = 4

// writeExcerpt appends the gutter, the source line, and the caret line.
// Tabs are expanded so the caret lands under the right character.
func writeExcerpt(b *strings.Builder, e *Error, src string) {
	lines := strings.Split(src, "\n")
	if e.Pos.Line < 1 || e.Pos.Line > len(lines) {
		return
	}
	line := []rune(strings.TrimSuffix(lines[e.Pos.Line-1], "\r"))
	num := strconv.Itoa(e.Pos.Line)
	gutter := strings.Repeat(" ", len(num))

	var shown strings.Builder
	col := -1 // printed column of the caret, measured in runes
	for i, r := range line {
		if i == e.Pos.Col-1 {
			col = len([]rune(shown.String()))
		}
		if r == '\t' {
			shown.WriteString(strings.Repeat(" ", tabWidth))
		} else {
			shown.WriteRune(r)
		}
	}
	if col < 0 {
		col = len([]rune(shown.String()))
	}
	n := e.Len
	if n < 1 {
		n = 1
	}
	fmt.Fprintf(b, "%s |\n", gutter)
	fmt.Fprintf(b, "%s | %s\n", num, shown.String())
	fmt.Fprintf(b, "%s | %s%s", gutter, strings.Repeat(" ", col), strings.Repeat("^", n))
	if l := Label(e.Code); l != "" {
		b.WriteString(" " + l)
	}
	b.WriteByte('\n')
	fmt.Fprintf(b, "%s |\n", gutter)
}

// Long call paths (deep recursion) show the innermost and outermost frames.
const callPathHead, callPathTail = 8, 2

func writeCallPath(b *strings.Builder, path []CallSite) {
	b.WriteString(" call path (innermost first):\n")
	write := func(c CallSite) {
		fn := c.Fn
		if fn == "<main>" {
			fn = "<entry>"
		}
		fmt.Fprintf(b, "   %-10s %s", fmt.Sprintf("%s:%d", c.File, c.Line), fn)
		if c.Elided > 0 {
			fmt.Fprintf(b, " (%d tail calls elided)", c.Elided)
		}
		b.WriteByte('\n')
	}
	if len(path) <= callPathHead+callPathTail+2 {
		for _, c := range path {
			write(c)
		}
		return
	}
	for _, c := range path[:callPathHead] {
		write(c)
	}
	fmt.Fprintf(b, "   … %d more\n", len(path)-callPathHead-callPathTail)
	for _, c := range path[len(path)-callPathTail:] {
		write(c)
	}
}

// IsCompileError reports whether a code belongs to the compile-time
// categories (syntax E01xx, names E02xx, modules E03xx). The fung command
// uses it to pick the exit code.
func IsCompileError(code string) bool {
	return strings.HasPrefix(code, "E01") || strings.HasPrefix(code, "E02") || strings.HasPrefix(code, "E03")
}
