// Command fung is the entry point of the fung language toolchain.
//
// Subcommands are dispatched here; the actual work lives in internal
// packages that later milestones add. The process exit code tells the
// caller what kind of failure happened (see the exit* constants).
package main

import (
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/tunarider/fung/internal/diag"
)

// Exit codes are defined in internal/diag so the test runner shares them.
const (
	exitOK           = diag.ExitOK
	exitRuntimeError = diag.ExitRuntimeError
	exitCompileError = diag.ExitCompileError
	exitUsage        = diag.ExitUsage
)

const usage = `usage:
  fung <script.fung> [args...]            run a script (same as: fung run <script.fung>)
  fung                                    start the REPL
  fung -e '<code>'                        run one line of code
  fung check <script.fung>                check syntax and names without running
  fung fmt [--check] <path>...|-          format files in place (--check: list only; -: stdin)
  fung test [path]                        run tests
  fung dump ast|core|bytecode <script>    print an internal stage
  fung version                            print the version
  fung help                               print this text
`

// main keeps the OS main thread free (05, 09: GUI later) and runs fung
// on a worker goroutine.
func main() {
	code := make(chan int, 1)
	go func() { code <- run(os.Args[1:], os.Stdin, os.Stdout, os.Stderr) }()
	os.Exit(<-code)
}

// run executes the CLI with the given arguments and streams and returns the
// exit code. It is separated from main so tests can drive it in-process.
func run(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		return startRepl(stdin, stdout, stderr)
	}

	switch args[0] {
	case "version":
		fmt.Fprintf(stdout, "fung %s\n", version)
		return exitOK
	case "help", "-h", "--help":
		fmt.Fprint(stdout, usage)
		return exitOK
	case "run":
		return runFile(args[1:], stdin, stdout, stderr)
	case "-e":
		if len(args) < 2 {
			fmt.Fprintf(stderr, "error: -e needs code\n\n%s", usage)
			return exitUsage
		}
		return execute("-e", args[1], args[2:], stdin, stdout, stderr)
	case "check":
		return runCheck(args[1:], stdout, stderr)
	case "fmt":
		return runFmt(args[1:], stdin, stdout, stderr)
	case "test":
		return runTest(args[1:], stdout, stderr)
	case "dump":
		return runDump(args[1:], stdout, stderr)
	}

	if isScriptPath(args[0]) {
		return runFile(args, stdin, stdout, stderr)
	}

	fmt.Fprintf(stderr, "error: unknown command `%s`\n\n%s", args[0], usage)
	return exitUsage
}

// runFile reads a script and executes it. args[0] is the path; the rest
// are the script's arguments (prelude value `args`, 06).
func runFile(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		fmt.Fprintf(stderr, "error: run needs a script path\n\n%s", usage)
		return exitUsage
	}
	src, err := os.ReadFile(args[0])
	if err != nil {
		fmt.Fprintf(stderr, "error: cannot read %s: %v\n", args[0], err)
		return exitUsage
	}
	return execute(args[0], string(src), args[1:], stdin, stdout, stderr)
}

// runDump handles `fung dump STAGE FILE` and `fung dump STAGE -e CODE`.
func runDump(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		fmt.Fprintf(stderr, "error: dump needs a stage: ast, core or bytecode\n\n%s", usage)
		return exitUsage
	}
	stage := args[0]
	switch stage {
	case "ast", "core", "bytecode":
	default:
		fmt.Fprintf(stderr, "error: unknown dump stage `%s` (want ast, core or bytecode)\n\n%s", stage, usage)
		return exitUsage
	}
	rest := args[1:]
	switch {
	case len(rest) >= 1 && rest[0] == "-e":
		if len(rest) < 2 {
			fmt.Fprintf(stderr, "error: -e needs code\n\n%s", usage)
			return exitUsage
		}
		return dump(stage, "-e", rest[1], stdout, stderr)
	case len(rest) == 1:
		src, err := os.ReadFile(rest[0])
		if err != nil {
			fmt.Fprintf(stderr, "error: cannot read %s: %v\n", rest[0], err)
			return exitUsage
		}
		return dump(stage, rest[0], string(src), stdout, stderr)
	}
	fmt.Fprintf(stderr, "error: dump needs a script path or -e code\n\n%s", usage)
	return exitUsage
}

// isScriptPath reports whether a first argument should be treated as a
// script to run rather than a subcommand name. A bare word such as
// `frobnicate` is a (possibly misspelled) subcommand; a script must either
// carry the .fung extension or be written as a path. `fung run <file>`
// is always available as the explicit form.
func isScriptPath(arg string) bool {
	if arg == "" || arg[0] == '-' {
		return false
	}
	return strings.HasSuffix(arg, ".fung") || strings.ContainsAny(arg, `/\`)
}
