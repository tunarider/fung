package tests

import (
	"bytes"
	"os/exec"
	"path/filepath"
	"testing"
)

// TestRepositoryIsFormatted keeps every .fung file in the repository in
// the fmt style (08): `fung fmt --check` reports nothing. The golden
// tests that are syntax errors on purpose (err_*.fung that fail to
// parse) are skipped: the formatter refuses them by design.
func TestRepositoryIsFormatted(t *testing.T) {
	fmtCheck := func(args ...string) (string, string, int) {
		cmd := exec.Command(fungBinary, append([]string{"fmt", "--check"}, args...)...)
		var out, errOut bytes.Buffer
		cmd.Stdout, cmd.Stderr = &out, &errOut
		code := 0
		if err := cmd.Run(); err != nil {
			if exit, ok := err.(*exec.ExitError); ok {
				code = exit.ExitCode()
			} else {
				t.Fatal(err)
			}
		}
		return out.String(), errOut.String(), code
	}
	if out, errOut, code := fmtCheck("../extras", "../examples"); code != 0 || out != "" {
		t.Errorf("files need formatting (run `fung fmt extras examples`):\n%s%s", out, errOut)
	}
	for _, path := range findGoldenFiles(t, "lang") {
		out, errOut, code := fmtCheck(path)
		switch {
		case code == 2:
			continue // a deliberate syntax error
		case code != 0 || out != "":
			t.Errorf("%s needs formatting (run `fung fmt %s`):\n%s%s", path, filepath.Join("tests", path), out, errOut)
		}
	}
}
