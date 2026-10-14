package diag

import "fmt"

// Pos is a source position. Line and Col are 1-based; Col counts runes.
// The zero Pos means "unknown", which value operations use before the VM
// fills in the position of the instruction that failed.
type Pos struct {
	File      string
	Line, Col int
}

func (p Pos) String() string { return fmt.Sprintf("%s:%d:%d", p.File, p.Line, p.Col) }

// IsZero reports whether the position is unknown.
func (p Pos) IsZero() bool { return p.Line == 0 }
