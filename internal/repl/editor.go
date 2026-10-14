package repl

// KeyKind classifies one key press for the line editor.
type KeyKind int

const (
	KeyRune KeyKind = iota
	KeyEnter
	KeyBackspace
	KeyDelete
	KeyLeft
	KeyRight
	KeyHome
	KeyEnd
	KeyUp
	KeyDown
	KeyCtrlC
	KeyCtrlD
	KeyCtrlL
)

// Key is one decoded key press.
type Key struct {
	Kind KeyKind
	Rune rune
}

// Editor is the terminal-independent state of one input line plus the
// session history (08): a rune buffer, a cursor, and history navigation.
// Terminal I/O lives in the readers; this type is pure so it is testable.
type Editor struct {
	buf     []rune
	cur     int
	history []string
	hist    int    // index into history while browsing; len(history) = the draft
	draft   []rune // the line being typed before browsing started
}

// Key applies one key. done is true on Enter, with the finished line.
func (e *Editor) Key(k Key) (done bool, line string) {
	switch k.Kind {
	case KeyRune:
		e.buf = append(e.buf[:e.cur], append([]rune{k.Rune}, e.buf[e.cur:]...)...)
		e.cur++
	case KeyEnter:
		line = string(e.buf)
		if line != "" && (len(e.history) == 0 || e.history[len(e.history)-1] != line) {
			e.history = append(e.history, line)
		}
		e.buf, e.cur, e.draft = nil, 0, nil
		e.hist = len(e.history)
		return true, line
	case KeyBackspace:
		if e.cur > 0 {
			e.buf = append(e.buf[:e.cur-1], e.buf[e.cur:]...)
			e.cur--
		}
	case KeyDelete, KeyCtrlD:
		if e.cur < len(e.buf) {
			e.buf = append(e.buf[:e.cur], e.buf[e.cur+1:]...)
		}
	case KeyLeft:
		if e.cur > 0 {
			e.cur--
		}
	case KeyRight:
		if e.cur < len(e.buf) {
			e.cur++
		}
	case KeyHome:
		e.cur = 0
	case KeyEnd:
		e.cur = len(e.buf)
	case KeyUp:
		if e.hist == 0 || len(e.history) == 0 {
			return false, ""
		}
		if e.hist == len(e.history) {
			e.draft = append([]rune(nil), e.buf...)
		}
		e.hist--
		e.set(e.history[e.hist])
	case KeyDown:
		if e.hist >= len(e.history) {
			return false, ""
		}
		e.hist++
		if e.hist == len(e.history) {
			e.buf = append([]rune(nil), e.draft...)
			e.cur = len(e.buf)
		} else {
			e.set(e.history[e.hist])
		}
	case KeyCtrlC:
		e.buf, e.cur = nil, 0
		e.hist = len(e.history)
	}
	return false, ""
}

// SetHistory seeds the history (from the history file, 08) before the
// first line is read.
func (e *Editor) SetHistory(lines []string) {
	e.history = append([]string(nil), lines...)
	e.hist = len(e.history)
}

func (e *Editor) set(s string) {
	e.buf = []rune(s)
	e.cur = len(e.buf)
}

// Render returns the line text and the cursor position in runes.
func (e *Editor) Render() (string, int) { return string(e.buf), e.cur }

// Empty reports whether nothing has been typed (Ctrl-D then ends input).
func (e *Editor) Empty() bool { return len(e.buf) == 0 }
