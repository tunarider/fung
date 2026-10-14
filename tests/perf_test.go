package tests

import (
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

// TestCollectionsStayLinear: the accumulation and tail-recursion shapes
// that were quadratic before M10 must finish 20,000 elements in well
// under 0.3 s (13–60 ms now; 0.74–7.5 s with full copies). The bound is
// loose for slow machines and tight enough that every shape fails if it
// goes quadratic again.
func TestCollectionsStayLinear(t *testing.T) {
	cases := []struct{ name, src string }{
		{"warm-up", "print(1)"}, // the first run pays for loading the binary; not judged
		{"map put fold", "m = fold(0..20000, {}, (acc, i) => put(acc, str(i), i))\nprint(len(m))"},
		{"map put again", "m = fold(0..20000, {}, (acc, i) => put(acc, str(i % 5000), i))\nprint(len(m))"},
		{"list append", "xs = fold(0..20000, [], (acc, i) => acc + [i])\nprint(len(xs))"},
		{"rest pattern", "fn count(xs, acc) = match xs:\n  [] => acc\n  [_, ...rest] => count(rest, acc + 1)\n;;\nprint(count(map(0..20000, x => x), 0))"},
		{"slice tail", "fn go(ys, acc) = len(ys) == 0 then acc else go(ys[1..], acc + 1)\nprint(go(map(0..20000, x => x), 0))"},
	}
	dir := t.TempDir()
	for _, c := range cases {
		name, src := c.name, c.src
		f := filepath.Join(dir, "p.fung")
		if err := os.WriteFile(f, []byte(src+"\n"), 0o644); err != nil {
			t.Fatal(err)
		}
		start := time.Now()
		out, err := exec.Command(fungBinary, f).CombinedOutput()
		took := time.Since(start)
		if err != nil {
			t.Fatalf("%s: %v\n%s", name, err, out)
		}
		t.Logf("%-14s %s", name, took)
		if name != "warm-up" && took > 300*time.Millisecond {
			t.Errorf("%s took %s", name, took)
		}
	}
}
