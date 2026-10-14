package repl

import "testing"

func typeKeys(e *Editor, s string) {
	for _, r := range s {
		e.Key(Key{Kind: KeyRune, Rune: r})
	}
}

func TestEditorInsertMoveDelete(t *testing.T) {
	e := &Editor{}
	typeKeys(e, "참치회")
	e.Key(Key{Kind: KeyLeft})
	e.Key(Key{Kind: KeyBackspace})
	typeKeys(e, "x")
	if text, cur := e.Render(); text != "참x회" || cur != 2 {
		t.Fatalf("text=%q cur=%d", text, cur)
	}
	e.Key(Key{Kind: KeyHome})
	e.Key(Key{Kind: KeyDelete})
	e.Key(Key{Kind: KeyEnd})
	typeKeys(e, "!")
	done, line := e.Key(Key{Kind: KeyEnter})
	if !done || line != "x회!" {
		t.Fatalf("done=%v line=%q", done, line)
	}
	if text, _ := e.Render(); text != "" {
		t.Fatal("enter clears the buffer")
	}
	e.Key(Key{Kind: KeyLeft}) // no-ops at the edges
	e.Key(Key{Kind: KeyBackspace})
	e.Key(Key{Kind: KeyDelete})
	if text, cur := e.Render(); text != "" || cur != 0 {
		t.Fatalf("edge moves: %q %d", text, cur)
	}
}

func TestEditorHistory(t *testing.T) {
	e := &Editor{}
	for _, l := range []string{"a", "b"} {
		typeKeys(e, l)
		e.Key(Key{Kind: KeyEnter})
	}
	typeKeys(e, "c")
	e.Key(Key{Kind: KeyUp})
	if text, _ := e.Render(); text != "b" {
		t.Fatalf("up = %q", text)
	}
	e.Key(Key{Kind: KeyUp})
	e.Key(Key{Kind: KeyUp}) // stays at the oldest
	if text, _ := e.Render(); text != "a" {
		t.Fatalf("up up = %q", text)
	}
	e.Key(Key{Kind: KeyDown})
	e.Key(Key{Kind: KeyDown})
	if text, _ := e.Render(); text != "c" {
		t.Fatalf("down restores the draft: %q", text)
	}
	e.Key(Key{Kind: KeyCtrlC})
	if text, _ := e.Render(); text != "" {
		t.Fatal("ctrl-c clears")
	}
	e.Key(Key{Kind: KeyEnter}) // empty lines are not recorded
	if len(e.history) != 2 {
		t.Fatalf("history = %v", e.history)
	}
	typeKeys(e, "b")
	e.Key(Key{Kind: KeyEnter}) // a repeat of the last entry is not recorded twice
	if len(e.history) != 2 {
		t.Fatalf("history = %v", e.history)
	}
}
