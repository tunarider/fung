package main

import (
	"fmt"
	"io"
	"os"

	"github.com/tunarider/fung/internal/vm"
)

// runCheck handles `fung check <file>` and `fung check -e CODE` (08):
// compile the whole import graph, run nothing. Errors exit 2; warnings
// are printed and exit 0; a clean file prints nothing.
func runCheck(args []string, stdout, stderr io.Writer) int {
	var file, src string
	switch {
	case len(args) >= 2 && args[0] == "-e":
		file, src = "-e", args[1]
	case len(args) == 1 && args[0] != "-e":
		b, err := os.ReadFile(args[0])
		if err != nil {
			fmt.Fprintf(stderr, "error: cannot read %s: %v\n", args[0], err)
			return exitUsage
		}
		file, src = args[0], string(b)
	default:
		fmt.Fprintf(stderr, "error: check needs a script path or -e code\n\n%s", usage)
		return exitUsage
	}
	prelude, errs := bootPrelude(&vm.Runtime{}, nil)
	if len(errs) > 0 {
		reportErrors(errs, nil, stderr)
		return exitCompileError
	}
	loader := loaderFor(prelude)
	loader.Tests = true
	prog, errs := loader.Load(file, src)
	if len(errs) > 0 {
		reportErrors(errs, prog.Sources, stderr)
		return exitCompileError
	}
	reportErrors(prog.Warnings, prog.Sources, stderr)
	return exitOK
}
