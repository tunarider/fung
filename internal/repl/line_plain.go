package repl

import (
	"bufio"
	"io"
	"strings"
)

// LineReader supplies one line per call; io.EOF ends the session.
type LineReader interface {
	ReadLine(prompt string) (string, error)
}

type plainReader struct {
	in  *bufio.Reader
	out io.Writer
}

// PlainReader reads lines without editing: for pipes, tests and
// platforms without a raw-mode terminal (08). The prompt is echoed.
func PlainReader(r io.Reader, w io.Writer) LineReader {
	return &plainReader{in: bufio.NewReader(r), out: w}
}

func (p *plainReader) ReadLine(prompt string) (string, error) {
	io.WriteString(p.out, prompt)
	line, err := p.in.ReadString('\n')
	if err != nil && line == "" {
		return "", io.EOF
	}
	return strings.TrimSuffix(strings.TrimSuffix(line, "\n"), "\r"), nil
}
