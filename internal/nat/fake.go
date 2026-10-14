package nat

import (
	"bufio"
	"bytes"
	"io"
	"strings"

	"github.com/tunarider/fung/internal/value"
)

// FakeContext is a value.Context for unit tests of Go functions that
// need no VM: output is captured, input comes from a string, Call fails.
type FakeContext struct {
	Out, Err bytes.Buffer
	In       string
	inR      *bufio.Reader
}

func (c *FakeContext) Stdout() io.Writer { return &c.Out }
func (c *FakeContext) Stderr() io.Writer { return &c.Err }
func (c *FakeContext) ReadLine() (string, error) {
	if c.inR == nil {
		c.inR = bufio.NewReader(strings.NewReader(c.In))
	}
	line, err := c.inR.ReadString('\n')
	if err != nil && line == "" {
		return "", io.EOF
	}
	return line, nil
}
func (c *FakeContext) Call(value.Value, []value.Value) (value.Value, error) { return nil, nil }
func (c *FakeContext) AtExit(value.Value)                                   {}
func (c *FakeContext) Done() <-chan struct{}                                { return nil }
func (c *FakeContext) RunTask(fn value.Value, args []value.Value) (value.Value, error) {
	return c.Call(fn, args)
}
