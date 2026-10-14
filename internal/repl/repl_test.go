package repl

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/tunarider/fung/internal/frontend"
	"github.com/tunarider/fung/internal/native"
	"github.com/tunarider/fung/internal/prelude"
	"github.com/tunarider/fung/internal/vm"
)

// testLib is a one-module library: the REPL imports from whatever
// registry it is given (the command passes the built-in extras).
func testLib() frontend.Registry {
	return frontend.Registry{
		Natives: native.Bridges(),
		Stdlib: func(name string) (string, bool) {
			if name == "fs" {
				return "import _native.fs as _n\nexists = _n.exists\n", true
			}
			return "", false
		},
	}
}

func session(t *testing.T, input string) (string, string, int) {
	t.Helper()
	var out, errOut bytes.Buffer
	rt := &vm.Runtime{Stdout: &out, Stderr: &errOut}
	s := New(PlainReader(strings.NewReader(input), &out), &out, &errOut, prelude.Test(), testLib(), rt)
	code := s.Run()
	return out.String(), errOut.String(), code
}

func TestReplEvaluatesAndKeepsDefinitions(t *testing.T) {
	out, errOut, code := session(t, "x = 1\nx + 1\nfn f(n) = n * x\nf(5)\n[1, \"a\"]\nnil\n")
	if code != 0 || errOut != "" {
		t.Fatalf("code=%d err=%q", code, errOut)
	}
	if out != "> > 2\n> > 5\n> [1, \"a\"]\n> nil\n> \n" {
		t.Fatalf("out=%q", out)
	}
}

func TestReplRedefinesFunctionsAndContinuesLines(t *testing.T) {
	out, errOut, _ := session(t, "fn g() = 1\nfn f() = g()\nfn g() = 2\nf()\nfn h(a):\n  a + 1\n;;\nh(1)\n1 +\n2\nimport fs\nimport fs\nfs.exists(\"/\")\n")
	if errOut != "" {
		t.Fatalf("err=%q", errOut)
	}
	for _, want := range []string{"> 2\n", "> . . > 2\n", "> . 3\n", "> > > true\n"} {
		if !strings.Contains(out, want) {
			t.Errorf("missing %q in %q", want, out)
		}
	}
}

func TestReplReportsErrorsAndGoesOn(t *testing.T) {
	out, errOut, code := session(t, "print(y)\n1 / 0\n1 + 1\n")
	if code != 0 || !strings.Contains(errOut, "E0204") || !strings.Contains(errOut, "E0502") || !strings.HasSuffix(out, "> 2\n> \n") {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
	_, errOut, _ = session(t, "x = )\n")
	if !strings.Contains(errOut, "E0110") {
		t.Fatalf("a syntax error that is not at EOF is reported: %q", errOut)
	}
	_, errOut, _ = session(t, "y = 1 / 0\ny\n")
	if !strings.Contains(errOut, "E0502") || !strings.Contains(errOut, "E0808") || strings.Contains(errOut, "internal error") {
		t.Fatalf("a global whose assignment failed reads as E0808, not an internal error: %q", errOut)
	}
	_, errOut, _ = session(t, "fn f():\n")
	if errOut != "" {
		t.Fatalf("an unfinished block at the end of input is dropped quietly: %q", errOut)
	}
}

func TestReplCommandsAndExit(t *testing.T) {
	out, errOut, code := session(t, ":help\n:bogus\n:quit\nprint(1)\n")
	if code != 0 || !strings.Contains(out, ":load") || !strings.Contains(errOut, "unknown command :bogus") || strings.Contains(out, "1\n") {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
	out, _, code = session(t, "at_exit(() => print(\"bye\"))\nexit(4)\n")
	if code != 4 || !strings.Contains(out, "bye\n") {
		t.Fatalf("exit code = %d out=%q", code, out)
	}
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "lib.fung"), []byte("fn twice(x) = x * 2\nk = 10\n"), 0o644)
	out, errOut, _ = session(t, ":load "+filepath.Join(dir, "lib.fung")+"\ntwice(k)\n")
	if errOut != "" || !strings.Contains(out, "> 20\n") {
		t.Fatalf(":load: out=%q err=%q", out, errOut)
	}
}

func TestReplQuitCancelsTasks(t *testing.T) {
	var out, errOut bytes.Buffer
	rt := &vm.Runtime{Stdout: &out, Stderr: &errOut}
	prelude := prelude.Test()
	for k, v := range vm.Concurrency() {
		prelude[k] = v
	}
	s := New(PlainReader(strings.NewReader("t = spawn(() => sleep(10s))\n:quit\n"), &out), &out, &errOut, prelude, testLib(), rt)
	done := make(chan int, 1)
	go func() { done <- s.Run() }()
	select {
	case code := <-done:
		if code != 0 || errOut.String() != "" {
			t.Fatalf("code=%d err=%q", code, errOut.String())
		}
	case <-time.After(2 * time.Second):
		t.Fatal(":quit must not wait for a sleeping task")
	}
}
