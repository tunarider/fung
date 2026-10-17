package main

import (
	"bytes"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/format"
)

// runFmt formats files in place (08): `fung fmt <path>...` rewrites and
// names the changed files, `--check` only names them (exit 1 if any),
// `-` formats standard input to standard output (with `--check`, names
// `<stdin>` if it needs formatting and exits 1). A file with a syntax
// error is reported and left alone (exit 2).
func runFmt(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	check := false
	var paths []string
	for _, a := range args {
		switch {
		case a == "--check":
			check = true
		case strings.HasPrefix(a, "-") && a != "-":
			fmt.Fprintf(stderr, "error: unknown option %s\n\n%s", a, usage)
			return exitUsage
		default:
			paths = append(paths, a)
		}
	}
	if len(paths) == 0 {
		fmt.Fprintf(stderr, "error: fmt needs a path, or - for standard input\n\n%s", usage)
		return exitUsage
	}
	if len(paths) == 1 && paths[0] == "-" {
		src, err := io.ReadAll(stdin)
		if err != nil {
			fmt.Fprintf(stderr, "error: %v\n", err)
			return exitRuntimeError
		}
		out, ferr := format.Format("<stdin>", string(src))
		if ferr != nil {
			if ferr.Code == "E0801" {
				fmt.Fprintln(stderr, "error: formatter changed the code of <stdin>; nothing was written")
				return exitRuntimeError
			}
			io.WriteString(stderr, diag.Format(ferr, string(src)))
			return exitCompileError
		}
		if !check {
			io.WriteString(stdout, out)
			return exitOK
		}
		if out == string(src) {
			return exitOK
		}
		fmt.Fprintln(stdout, "<stdin>")
		return exitRuntimeError
	}
	files, err := fungFiles(paths)
	if err != nil {
		fmt.Fprintf(stderr, "error: %v\n", err)
		return exitUsage
	}
	code := exitOK
	worse := func(c int) {
		if c > code {
			code = c
		}
	}
	for _, path := range files {
		src, rerr := os.ReadFile(path)
		if rerr != nil {
			fmt.Fprintf(stderr, "error: cannot read %s: %v\n", path, rerr)
			worse(exitRuntimeError)
			continue
		}
		out, ferr := format.Format(path, string(src))
		if ferr != nil {
			if ferr.Code == "E0801" {
				fmt.Fprintf(stderr, "error: formatter changed the code of %s; nothing was written\n", path)
				worse(exitRuntimeError)
			} else {
				io.WriteString(stderr, diag.Format(ferr, string(src)))
				worse(exitCompileError)
			}
			continue
		}
		if bytes.Equal([]byte(out), src) {
			continue
		}
		fmt.Fprintln(stdout, path)
		if check {
			worse(exitRuntimeError)
			continue
		}
		mode := fs.FileMode(0o644)
		if info, serr := os.Stat(path); serr == nil {
			mode = info.Mode().Perm()
		}
		if werr := os.WriteFile(path, []byte(out), mode); werr != nil {
			fmt.Fprintf(stderr, "error: cannot write %s: %v\n", path, werr)
			worse(exitRuntimeError)
		}
	}
	return code
}

// fungFiles expands paths: a file as is, a directory recursively, skipping
// directories whose name starts with `_` (fixtures) or `.` (.git, .claude).
func fungFiles(paths []string) ([]string, error) {
	var files []string
	for _, p := range paths {
		info, err := os.Stat(p)
		if err != nil {
			return nil, err
		}
		if !info.IsDir() {
			files = append(files, p)
			continue
		}
		// WalkDir does not follow a symlinked root; a trailing separator
		// makes the OS resolve it.
		root := p
		if l, lerr := os.Lstat(p); lerr == nil && l.Mode()&os.ModeSymlink != 0 {
			root = p + string(filepath.Separator)
		}
		err = filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if d.IsDir() && path != root && (strings.HasPrefix(d.Name(), "_") || strings.HasPrefix(d.Name(), ".")) {
				return fs.SkipDir
			}
			if !d.IsDir() && strings.HasSuffix(d.Name(), ".fung") {
				files = append(files, path)
			}
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	sort.Strings(files)
	return files, nil
}
