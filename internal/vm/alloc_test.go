package vm

import (
	"io"
	"strings"
	"testing"
)

// allocsPerStep runs src with N replaced by 100 and by 200 and returns the
// extra heap allocations per step. Numbers stay below 256 so boxing an
// int in an interface does not allocate (Go keeps those preallocated);
// what is left is the VM's own cost per step.
func allocsPerStep(t *testing.T, src string) float64 {
	t.Helper()
	at := func(n string) float64 {
		p := compileWith(t, strings.ReplaceAll(src, "N", n), concPrelude())
		return testing.AllocsPerRun(20, func() {
			rt := &Runtime{Stdout: io.Discard, Stderr: io.Discard}
			if _, err := rt.Run(p); err != nil {
				t.Fatal(err)
			}
		})
	}
	return (at("200") - at("100")) / 100
}

// A call between fung functions with positional arguments lays the
// arguments out in the callee's frame on the VM stack: no heap allocation.
func TestFungCallsDoNotAllocate(t *testing.T) {
	src := "fn add(a, b) = a + b\nfn spin(n, acc) = n == 0 then acc else spin(n - 1, add(acc, 0))\nprint(spin(N, 1))"
	if a := allocsPerStep(t, src); a > 0.05 {
		t.Fatalf("%.2f allocations per call step, want 0", a)
	}
}

// A native call needs one fresh argument slice (the native may keep it)
// and nothing else.
func TestNativeCallsAllocateOnlyTheirArguments(t *testing.T) {
	src := "fn spin(n, acc) = n == 0 then acc else spin(n - 1, acc + len(\"ab\") - 2)\nprint(spin(N, 1))"
	if a := allocsPerStep(t, src); a > 1.05 {
		t.Fatalf("%.2f allocations per native call, want at most 1", a)
	}
}

// fold over a range walks it without building a list, and calling the
// lambda reuses one argument buffer.
func TestFoldOverARangeDoesNotAllocatePerItem(t *testing.T) {
	src := "print(fold(0..N, 0, (a, x) => a))"
	if a := allocsPerStep(t, src); a > 0.05 {
		t.Fatalf("%.2f allocations per item, want 0", a)
	}
}
