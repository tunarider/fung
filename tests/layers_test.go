package tests

import (
	"os/exec"
	"strings"
	"testing"
)

const module = "github.com/tunarider/fung"

// deps lists the non-test dependencies of the given packages.
func deps(t *testing.T, pattern string) []string {
	t.Helper()
	out, err := exec.Command("go", "list", "-deps", pattern).Output()
	if err != nil {
		t.Fatalf("go list -deps %s: %v", pattern, err)
	}
	pkgs := strings.Fields(string(out))
	if len(pkgs) == 0 {
		t.Fatalf("go list -deps %s: no packages", pattern)
	}
	return pkgs
}

// TestLayers pins the dependency direction of 06 (네 층): the core
// (internal/...) never knows the extras, and the extras never reach into
// the VM. Only cmd/fung assembles the layers.
func TestLayers(t *testing.T) {
	for _, d := range deps(t, "../internal/...") {
		if strings.HasPrefix(d, module+"/extras") {
			t.Errorf("internal/... depends on %s", d)
		}
	}
	for _, d := range deps(t, "../extras/...") {
		if d == module+"/internal/vm" || d == module+"/internal/stdlib" {
			t.Errorf("extras depends on %s", d)
		}
	}
}
