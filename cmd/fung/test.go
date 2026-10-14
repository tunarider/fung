package main

import (
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/vm"
)

// runTest handles `fung test [path]` (08): every .fung file under path
// (default .) runs as an entry file on its own runtime, then its test
// blocks run in order. Exit codes: 2 if any file failed to compile, 1 if
// any test failed, else 0. An exit() inside a test file is a failure:
// nothing may end the run silently.
func runTest(args []string, stdout, stderr io.Writer) int {
	root := "."
	if len(args) > 1 {
		fmt.Fprintf(stderr, "error: test takes at most one path\n\n%s", usage)
		return exitUsage
	}
	if len(args) == 1 {
		root = args[0]
	}
	files, err := testFiles(root)
	if err != nil {
		fmt.Fprintf(stderr, "error: %v\n", err)
		return exitUsage
	}
	passed, failed := 0, 0
	compileFailed := false
	seen := map[string]bool{} // errors already printed: a broken library is reported once
	for _, path := range files {
		r := runTestFile(path, stdout, stderr, seen)
		passed += r.passed
		failed += r.failed
		if r.compileFailed {
			compileFailed = true
		}
	}
	fmt.Fprintf(stdout, "%d passed, %d failed\n", passed, failed)
	switch {
	case compileFailed:
		return exitCompileError
	case failed > 0:
		return exitRuntimeError
	}
	return exitOK
}

// testFiles lists the .fung files to test: the file itself, or every
// .fung under a directory, skipping `_`-prefixed directories (fixtures).
func testFiles(root string) ([]string, error) {
	info, err := os.Stat(root)
	if err != nil {
		return nil, err
	}
	if !info.IsDir() {
		return []string{root}, nil
	}
	var files []string
	err = filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() && path != root && strings.HasPrefix(d.Name(), "_") {
			return fs.SkipDir
		}
		if !d.IsDir() && strings.HasSuffix(d.Name(), ".fung") {
			files = append(files, path)
		}
		return nil
	})
	sort.Strings(files)
	return files, err
}

type testResult struct {
	passed, failed int
	compileFailed  bool
}

// runTestFile compiles one file with its tests, runs its top level, then
// each test. A failing test prints FAIL with the indented report; an
// exit() stops the file and carries its code out.
func runTestFile(path string, stdout, stderr io.Writer, seen map[string]bool) testResult {
	var r testResult
	src, err := os.ReadFile(path)
	if err != nil {
		fmt.Fprintf(stderr, "error: cannot read %s: %v\n", path, err)
		r.compileFailed = true
		return r
	}
	rt := &vm.Runtime{Stdout: stdout, Stderr: stderr}
	prelude, errs := bootPrelude(rt, nil)
	if len(errs) > 0 {
		reportErrors(errs, nil, stderr)
		r.compileFailed = true
		return r
	}
	loader := loaderFor(prelude)
	loader.Tests = true
	prog, errs := loader.Load(path, string(src))
	if len(errs) > 0 {
		// A file that cannot load is a failure in the summary; an error
		// shared with other files (a broken import) is printed once.
		fmt.Fprintf(stdout, "FAIL %s: <load>\n", path)
		var fresh []*diag.Error
		for _, e := range errs {
			key := fmt.Sprintf("%s:%d:%d:%s", e.Pos.File, e.Pos.Line, e.Pos.Col, e.Code)
			if !seen[key] {
				seen[key] = true
				fresh = append(fresh, e)
			}
		}
		reportErrors(fresh, prog.Sources, stderr)
		r.compileFailed = true
		r.failed++
		return r
	}
	if !hasTests(prog.Entry) {
		return r // a plain script: not run, not listed (08)
	}
	reportErrors(prog.Warnings, prog.Sources, stderr)
	register(rt, prog)
	report := func(name string, err *diag.Error) {
		fmt.Fprintf(stdout, "FAIL %s: %s\n", path, name)
		if n, ok := err.ExitCode(); ok {
			fmt.Fprintf(stdout, "  exit(%d) called inside a test file; the rest of the file was skipped\n", n)
		} else {
			io.WriteString(stdout, indent(diag.Format(err, prog.Sources[err.Pos.File])))
		}
		r.failed++
	}
	rt.Sources = prog.Sources
	if _, err := rt.Run(prog.Entry); err != nil {
		report("<top level>", err)
	} else {
		for _, tc := range rt.Tests() {
			err := rt.CallTest(tc.Fn)
			if err == nil {
				fmt.Fprintf(stdout, "ok   %s: %s\n", path, tc.Name)
				r.passed++
				continue
			}
			report(tc.Name, err)
			if _, ok := err.ExitCode(); ok {
				break
			}
		}
	}
	rt.CancelAll()
	rt.Wait()
	for _, err := range rt.RunExitHandlers() {
		if _, ok := err.ExitCode(); ok {
			continue
		}
		if err.Code == "E0701" || err.Code == "E0702" {
			continue
		}
		reportErrors([]*diag.Error{err}, prog.Sources, stderr)
	}
	// The file is done: a task failure nobody retrieved fails it (05).
	rt.Wait()
	rt.ReportUnretrieved()
	if rt.Failed() {
		fmt.Fprintf(stdout, "FAIL %s: <tasks>\n  a task failed and nothing awaited it (see standard error)\n", path)
		r.failed++
	}
	return r
}

// hasTests reports whether the entry body registers any test (tests are
// top-level only, so the entry's code is enough).
func hasTests(p *bytecode.Proto) bool {
	for _, in := range p.Code {
		if in.Op() == bytecode.OpTest {
			return true
		}
	}
	return false
}

// indent prefixes every line with two spaces.
func indent(s string) string {
	lines := strings.Split(strings.TrimSuffix(s, "\n"), "\n")
	for i, l := range lines {
		lines[i] = "  " + l
	}
	return strings.Join(lines, "\n") + "\n"
}
