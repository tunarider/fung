//go:build !darwin && !linux

package repl

import (
	"io"
	"os"
)

// TermReader has no raw-mode editor on this platform; the REPL reads
// plain lines (08).
func TermReader(in *os.File, out io.Writer) (LineReader, bool) { return nil, false }
