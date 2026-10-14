package repl

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// The history file keeps one input line per line, is loaded at start,
// appended after each accepted line, and trimmed to maxHistory (08).
func TestHistoryFileLoadAppendTrim(t *testing.T) {
	p := filepath.Join(t.TempDir(), "hist")
	os.WriteFile(p, []byte("a = 1\nprint(a)\n"), 0o600)
	h := openHistory(p)
	if got := h.lines; strings.Join(got, "|") != "a = 1|print(a)" {
		t.Fatalf("load: %q", got)
	}
	h.append("b = 2")
	h.append("b = 2") // consecutive duplicate: not stored
	h.append("")      // empty: not stored
	b, _ := os.ReadFile(p)
	if string(b) != "a = 1\nprint(a)\nb = 2\n" {
		t.Errorf("file after append: %q", b)
	}
	var many strings.Builder
	for i := 0; i < maxHistory+50; i++ {
		many.WriteString("x\n")
	}
	os.WriteFile(p, []byte(many.String()), 0o600)
	h = openHistory(p)
	if len(h.lines) != maxHistory {
		t.Errorf("trim: %d lines, want %d", len(h.lines), maxHistory)
	}
	b, _ = os.ReadFile(p)
	if strings.Count(string(b), "\n") != maxHistory {
		t.Errorf("the trimmed file must be rewritten: %d lines", strings.Count(string(b), "\n"))
	}
	if h := openHistory(filepath.Join(t.TempDir(), "missing", "hist")); h != nil {
		h.append("ok") // an unwritable path disables history silently
	}
}

// An editor seeded with the loaded history browses it with Up.
func TestEditorSeededHistory(t *testing.T) {
	var e Editor
	e.SetHistory([]string{"first", "second"})
	e.Key(Key{Kind: KeyUp})
	if text, _ := e.Render(); text != "second" {
		t.Errorf("Up after seeding = %q", text)
	}
	e.Key(Key{Kind: KeyUp})
	if text, _ := e.Render(); text != "first" {
		t.Errorf("Up twice = %q", text)
	}
}
