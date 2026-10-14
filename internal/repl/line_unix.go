//go:build darwin || linux

package repl

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"strings"
	"syscall"
	"unsafe"
)

// termReader is the raw-mode line editor for interactive terminals (08):
// the terminal delivers keys one at a time and this code draws the line.
type termReader struct {
	f      *os.File
	in     *bufio.Reader
	out    io.Writer
	editor Editor
	hist   *historyFile // nil: history stays in memory
}

// UseHistoryFile loads path into the editor's history and records every
// accepted line there (08). An unusable path leaves history in memory.
func (t *termReader) UseHistoryFile(path string) {
	if h := openHistory(path); h != nil {
		t.hist = h
		t.editor.SetHistory(h.lines)
	}
}

// TermReader returns a line editor when in is a terminal, else nil, false.
func TermReader(in *os.File, out io.Writer) (LineReader, bool) {
	info, err := in.Stat()
	if err != nil || info.Mode()&os.ModeCharDevice == 0 {
		return nil, false
	}
	return &termReader{f: in, in: bufio.NewReader(in), out: out}, true
}

// rawMode switches the terminal to raw input and returns the restore
// function. Output processing stays on so "\n" still moves to column 0.
func rawMode(fd uintptr) (func(), error) {
	var old syscall.Termios
	if _, _, errno := syscall.Syscall(syscall.SYS_IOCTL, fd, ioctlGetTermios, uintptr(unsafe.Pointer(&old))); errno != 0 {
		return nil, errno
	}
	raw := old
	raw.Iflag &^= syscall.ICRNL | syscall.IXON
	raw.Lflag &^= syscall.ECHO | syscall.ICANON | syscall.ISIG | syscall.IEXTEN
	raw.Cc[syscall.VMIN] = 1
	raw.Cc[syscall.VTIME] = 0
	if _, _, errno := syscall.Syscall(syscall.SYS_IOCTL, fd, ioctlSetTermios, uintptr(unsafe.Pointer(&raw))); errno != 0 {
		return nil, errno
	}
	return func() {
		syscall.Syscall(syscall.SYS_IOCTL, fd, ioctlSetTermios, uintptr(unsafe.Pointer(&old)))
	}, nil
}

func (t *termReader) ReadLine(prompt string) (string, error) {
	restore, err := rawMode(t.f.Fd())
	if err != nil {
		// Not a usable terminal after all: fall back to plain reading.
		io.WriteString(t.out, prompt)
		line, rerr := t.in.ReadString('\n')
		if rerr != nil && line == "" {
			return "", io.EOF
		}
		return strings.TrimRight(line, "\r\n"), nil
	}
	defer restore()
	t.draw(prompt)
	for {
		k, err := t.readKey()
		if err != nil {
			io.WriteString(t.out, "\n")
			return "", io.EOF
		}
		switch k.Kind {
		case KeyCtrlD:
			if t.editor.Empty() {
				io.WriteString(t.out, "\n")
				return "", io.EOF
			}
		case KeyCtrlC:
			t.editor.Key(k)
			io.WriteString(t.out, "^C\n")
			t.draw(prompt)
			continue
		case KeyCtrlL:
			io.WriteString(t.out, "\x1b[2J\x1b[H")
			t.draw(prompt)
			continue
		}
		done, line := t.editor.Key(k)
		if done {
			t.hist.append(line)
			io.WriteString(t.out, "\n")
			return line, nil
		}
		t.draw(prompt)
	}
}

// draw repaints the prompt and the line, then places the cursor. Every
// rune is assumed one column wide (wide characters are an alpha item).
func (t *termReader) draw(prompt string) {
	text, cur := t.editor.Render()
	back := len([]rune(text)) - cur
	fmt.Fprintf(t.out, "\r%s%s\x1b[K", prompt, text)
	if back > 0 {
		fmt.Fprintf(t.out, "\x1b[%dD", back)
	}
}

// readKey decodes one key press: control characters, escape sequences
// for arrows/Home/End/Delete, or a UTF-8 rune.
func (t *termReader) readKey() (Key, error) {
	r, _, err := t.in.ReadRune()
	if err != nil {
		return Key{}, err
	}
	switch r {
	case '\r', '\n':
		return Key{Kind: KeyEnter}, nil
	case 0x7f, 0x08:
		return Key{Kind: KeyBackspace}, nil
	case 0x01:
		return Key{Kind: KeyHome}, nil
	case 0x05:
		return Key{Kind: KeyEnd}, nil
	case 0x03:
		return Key{Kind: KeyCtrlC}, nil
	case 0x04:
		return Key{Kind: KeyCtrlD}, nil
	case 0x0c:
		return Key{Kind: KeyCtrlL}, nil
	case 0x1b:
		return t.readEscape()
	}
	if r < 0x20 {
		return t.readKey() // other control keys are ignored
	}
	return Key{Kind: KeyRune, Rune: r}, nil
}

func (t *termReader) readEscape() (Key, error) {
	b, err := t.in.ReadByte()
	if err != nil {
		return Key{}, err
	}
	if b != '[' && b != 'O' {
		return t.readKey()
	}
	c, err := t.in.ReadByte()
	if err != nil {
		return Key{}, err
	}
	switch c {
	case 'A':
		return Key{Kind: KeyUp}, nil
	case 'B':
		return Key{Kind: KeyDown}, nil
	case 'C':
		return Key{Kind: KeyRight}, nil
	case 'D':
		return Key{Kind: KeyLeft}, nil
	case 'H':
		return Key{Kind: KeyHome}, nil
	case 'F':
		return Key{Kind: KeyEnd}, nil
	}
	if c >= '0' && c <= '9' {
		// ESC [ n ~ : 1 home, 3 delete, 4 end, 7 home, 8 end
		n := int(c - '0')
		for {
			d, err := t.in.ReadByte()
			if err != nil {
				return Key{}, err
			}
			if d == '~' {
				break
			}
			if d >= '0' && d <= '9' {
				n = n*10 + int(d-'0')
			}
		}
		switch n {
		case 1, 7:
			return Key{Kind: KeyHome}, nil
		case 3:
			return Key{Kind: KeyDelete}, nil
		case 4, 8:
			return Key{Kind: KeyEnd}, nil
		}
	}
	return t.readKey()
}
