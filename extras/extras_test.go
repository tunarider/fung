package extras

import (
	"strings"
	"testing"
)

func TestSourceReturnsEmbeddedModules(t *testing.T) {
	for _, name := range []string{"prelude", "fs", "path", "cli", "time", "json", "regex", "http"} {
		src, ok := Source(name)
		if !ok || src == "" {
			t.Errorf("Source(%q) missing", name)
		}
	}
	if _, ok := Source("nope"); ok {
		t.Error("Source(nope) should be absent")
	}
	if src, _ := Source("prelude"); !strings.Contains(src, "import _native.process") {
		t.Error("prelude.fung should re-export the process bridge")
	}
}

func TestDefaultBundlesSourcesAndBridges(t *testing.T) {
	r := Default()
	if r.Stdlib == nil {
		t.Fatal("Default().Stdlib is nil")
	}
	if _, ok := r.Stdlib("fs"); !ok {
		t.Error("Default().Stdlib cannot find fs")
	}
	for _, name := range []string{"_native.fs", "_native.path", "_native.process", "_native.time", "_native.json", "_native.regex", "_native.http"} {
		if r.Natives[name] == nil {
			t.Errorf("Default().Natives lacks %s", name)
		}
	}
	if r.Natives["_native.conc"] != nil {
		t.Error("the concurrency bridge belongs to the VM, not to extras")
	}
}
