package value

import "testing"

func TestModuleValue(t *testing.T) {
	m := NewModule("fs", "fs")
	m.Set("read", Int(1))
	m.Set("write", Int(2))
	if m.TypeName() != "module" || Format(m) != "<module fs>" {
		t.Fatalf("type %s format %s", m.TypeName(), Format(m))
	}
	if got := m.Names(); len(got) != 2 || got[0] != "read" || got[1] != "write" {
		t.Fatalf("names = %v", got)
	}
	if v, err := Index(m, Str("read")); err != nil || v != Int(1) {
		t.Fatalf("read = %v %v", v, err)
	}
	if _, err := Index(m, Str("nope")); err == nil || err.Code != "E0303" {
		t.Fatalf("missing: %v", err)
	}
	if _, err := Index(m, Str("_hidden")); err == nil || err.Code != "E0304" {
		t.Fatalf("private: %v", err)
	}
	if _, err := Index(m, Int(0)); err == nil || err.Code != "E0501" {
		t.Fatalf("int key: %v", err)
	}
	other := NewModule("fs", "fs")
	if !Equal(m, m) || Equal(m, other) {
		t.Fatal("modules compare by identity")
	}
	if _, err := Compare(m, other); err == nil || err.Code != "E0501" {
		t.Fatal("modules are not ordered")
	}
}

func TestFileValue(t *testing.T) {
	f := &File{Path: "/tmp/x"}
	if f.TypeName() != "file" || Format(f) != `<file "/tmp/x">` {
		t.Fatalf("type %s format %s", f.TypeName(), Format(f))
	}
	if !Equal(f, f) || Equal(f, &File{Path: "/tmp/x"}) {
		t.Fatal("files compare by identity")
	}
}
