// Package diag holds diagnostics shared across the toolchain: error
// numbers and the message catalog (catalog.go), the Error type and its
// output format (error.go), source positions (pos.go), and the process
// exit codes below.
package diag

// Process exit codes of the fung command. Scripts and the golden test
// runner rely on these to tell failure kinds apart, so they are part of
// the CLI contract and must not change casually.
const (
	ExitOK           = 0 // finished normally
	ExitRuntimeError = 1 // the program ran and failed (uncaught error, `!` on Err, ...)
	ExitCompileError = 2 // the program could not be compiled (syntax or semantic error)
	ExitUsage        = 3 // the fung command itself was used incorrectly or cannot start
)
