// Package tests runs the language golden tests: every *.fung file under
// this directory is executed with a freshly built fung binary and its
// output is compared with the `# expect:` / `# expect-error:` comments
// inside the file (see internal/golden).
package tests

import (
	"bytes"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/golden"
)

// fungBinary is the path of the binary built once in TestMain.
var fungBinary string

func TestMain(m *testing.M) {
	dir, err := os.MkdirTemp("", "fung-golden-")
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	fungBinary = filepath.Join(dir, "fung")
	if runtime.GOOS == "windows" {
		fungBinary += ".exe"
	}
	build := exec.Command("go", "build", "-o", fungBinary, "../cmd/fung")
	build.Stderr = os.Stderr
	if err := build.Run(); err != nil {
		fmt.Fprintln(os.Stderr, "building fung:", err)
		os.RemoveAll(dir)
		os.Exit(1)
	}
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}

func TestGolden(t *testing.T) {
	files := findGoldenFiles(t, ".")
	if len(files) == 0 {
		t.Skip("no golden files yet")
	}
	for _, path := range files {
		t.Run(filepath.ToSlash(path), func(t *testing.T) {
			runGolden(t, path)
		})
	}
}

func findGoldenFiles(t *testing.T, root string) []string {
	t.Helper()
	var files []string
	err := filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() && strings.HasPrefix(d.Name(), "_") {
			return fs.SkipDir // module fixtures, not tests
		}
		if !d.IsDir() && strings.HasSuffix(d.Name(), ".fung") {
			files = append(files, path)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	return files
}

func runGolden(t *testing.T, path string) {
	t.Helper()
	src, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	c, err := golden.Parse(path, src)
	if err != nil {
		t.Fatal(err)
	}

	cmd := exec.Command(fungBinary, append([]string{path}, c.Args...)...)
	var stdout, stderr bytes.Buffer
	cmd.Stdin = strings.NewReader(c.Stdin)
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err = cmd.Run()
	code := 0
	var exitErr *exec.ExitError
	if errors.As(err, &exitErr) {
		code = exitErr.ExitCode()
	} else if err != nil {
		t.Fatalf("running %s: %v", path, err)
	}

	r := golden.Result{Stdout: stdout.String(), Stderr: stderr.String(), ExitCode: code}
	if err := golden.Check(c, r); err != nil {
		t.Error(err)
	}
}
