// Package value holds the Go representation of fung values and the
// primitive operations on them. Values are immutable by convention: every
// operation returns a new value. Errors carry no position; the VM fills
// in the position of the failing instruction.
package value

import "io"

type Value interface {
	TypeName() string
}

type Int int64
type Float float64
type Bool bool

// Nil is fung's nil. It is distinct from a Go nil interface, which the VM
// uses to mean "slot not yet assigned".
type Nil struct{}

// Context is what a native function may ask the VM for.
type Context interface {
	Stdout() io.Writer
	Stderr() io.Writer
	ReadLine() (string, error)                     // one line of standard input (io.EOF at the end), shared by all tasks
	Call(fn Value, args []Value) (Value, error)    // re-enter the VM (map, at_exit, …)
	AtExit(fn Value)                               // register an exit handler
	Done() <-chan struct{}                         // this task's cancellation signal (nil: never)
	RunTask(fn Value, args []Value) (Value, error) // run fn on a fresh child task, on this goroutine, and wait for it
}

// NativeParam declares one parameter of a native function (06: 내장
// 함수도 fung 함수와 같은 호출 규약). A nil Default means required.
type NativeParam struct {
	Name    string
	Default Value
	Rest    bool
}

// Opaque is a value without a literal form that compares by identity
// (02): tasks, channels, atoms. Display is what print and the REPL show.
type Opaque interface {
	Value
	Display() string
}

// Iterable is a value the collection functions can iterate besides lists
// and ranges (02): a channel drains until it is closed.
type Iterable interface {
	Value
	Drain(ctx Context) ([]Value, error)
}

// Fn is any callable value: natives and closures.
type Fn interface {
	Value
	FnName() string // "" for anonymous functions
}

// Native is a function implemented in Go. The VM lays out its arguments
// exactly as for a fung function and passes one slot per parameter.
type Native struct {
	Name   string
	Params []NativeParam
	Fn     func(ctx Context, args []Value) (Value, error)
}

func (n *Native) FnName() string { return n.Name }

func (Int) TypeName() string     { return "int" }
func (Float) TypeName() string   { return "float" }
func (Bool) TypeName() string    { return "bool" }
func (Nil) TypeName() string     { return "nil" }
func (*Native) TypeName() string { return "fn" }

// Str is a fung string: immutable UTF-8 text indexed by rune (02).
type Str string

func (Str) TypeName() string { return "str" }
