package native

import (
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"

	"github.com/tunarider/fung/internal/nat"
	"github.com/tunarider/fung/internal/prelude"
	"github.com/tunarider/fung/internal/value"
)

func fsFn(t *testing.T, name string) *value.Native {
	t.Helper()
	v, ok := Bridges()["_native.fs"].Get(name)
	if !ok {
		t.Fatalf("no %s", name)
	}
	return v.(*value.Native)
}

func str(s string) value.Value { return value.Str(s) }

func TestFsReadWriteListStat(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "a.txt")
	c := &nat.FakeContext{}
	if r, _ := fsFn(t, "write").Fn(c, []value.Value{str(p), str("hi\n")}); value.Format(r) != "Ok(nil)" {
		t.Fatalf("write: %s", value.Format(r))
	}
	fsFn(t, "append").Fn(c, []value.Value{str(p), str("there\n")})
	if r, _ := fsFn(t, "read").Fn(c, []value.Value{str(p)}); value.Format(r) != `Ok("hi\nthere\n")` {
		t.Fatalf("read: %s", value.Format(r))
	}
	r, _ := fsFn(t, "read").Fn(c, []value.Value{str(filepath.Join(dir, "none"))})
	if !strings.HasPrefix(value.Format(r), "Err({kind: 'not_found, message: \"cannot read `") {
		t.Fatalf("missing: %s", value.Format(r))
	}
	fsFn(t, "mkdir").Fn(c, []value.Value{str(filepath.Join(dir, "sub/deep"))})
	fsFn(t, "write").Fn(c, []value.Value{str(filepath.Join(dir, "sub/deep/b.txt")), str("")})
	if r, _ := fsFn(t, "list").Fn(c, []value.Value{str(dir), value.Bool(false)}); value.Format(r) != `Ok(["a.txt", "sub"])` {
		t.Fatalf("list: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "walk").Fn(c, []value.Value{str(dir)}); value.Format(r) != `Ok(["`+p+`", "`+filepath.Join(dir, "sub/deep/b.txt")+`"])` {
		t.Fatalf("walk: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "glob").Fn(c, []value.Value{str(filepath.Join(dir, "*.txt"))}); value.Format(r) != `Ok(["`+p+`"])` {
		t.Fatalf("glob: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "stat").Fn(c, []value.Value{str(p)}); value.Format(r) != `Ok({name: "a.txt", size: 9, is_dir: false})` {
		t.Fatalf("stat: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "exists").Fn(c, []value.Value{str(p)}); r != value.Bool(true) {
		t.Fatal("exists")
	}
	fsFn(t, "copy").Fn(c, []value.Value{str(p), str(filepath.Join(dir, "c.txt"))})
	fsFn(t, "move").Fn(c, []value.Value{str(filepath.Join(dir, "c.txt")), str(filepath.Join(dir, "d.txt"))})
	if r, _ := fsFn(t, "read").Fn(c, []value.Value{str(filepath.Join(dir, "d.txt"))}); value.Format(r) != `Ok("hi\nthere\n")` {
		t.Fatalf("copy+move: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "remove").Fn(c, []value.Value{str(filepath.Join(dir, "sub"))}); !strings.HasPrefix(value.Format(r), "Err(") {
		t.Fatal("remove on a non-empty dir must fail")
	}
	fsFn(t, "remove_all").Fn(c, []value.Value{str(filepath.Join(dir, "sub"))})
	if r, _ := fsFn(t, "exists").Fn(c, []value.Value{str(filepath.Join(dir, "sub"))}); r != value.Bool(false) {
		t.Fatal("remove_all")
	}
	if r, _ := fsFn(t, "temp_dir").Fn(c, nil); !strings.HasPrefix(value.Format(r), "Ok(\"") {
		t.Fatalf("temp_dir: %s", value.Format(r))
	} else {
		fsFn(t, "remove_all").Fn(c, []value.Value{r.(*value.Tag).Args[0]})
	}
	if _, err := fsFn(t, "read").Fn(c, []value.Value{value.Int(1)}); err == nil {
		t.Fatal("read(1) is E0403")
	}
}

func TestFsHandles(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "h.txt")
	c := &nat.FakeContext{}
	r, _ := fsFn(t, "open").Fn(c, []value.Value{str(p), value.Intern("write")})
	h := r.(*value.Tag).Args[0].(*value.File)
	fsFn(t, "write_line").Fn(c, []value.Value{h, str("one")})
	fsFn(t, "write_line").Fn(c, []value.Value{h, str("two")})
	if r, _ := fsFn(t, "close").Fn(c, []value.Value{h}); value.Format(r) != "Ok(nil)" {
		t.Fatal("close")
	}
	if r, _ := fsFn(t, "write_line").Fn(c, []value.Value{h, str("x")}); !strings.Contains(value.Format(r), "'closed") {
		t.Fatalf("write after close: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "close").Fn(c, []value.Value{h}); !strings.Contains(value.Format(r), "'closed") {
		t.Fatalf("double close: %s", value.Format(r))
	}
	r, _ = fsFn(t, "open").Fn(c, []value.Value{str(p), value.Intern("read")})
	h = r.(*value.Tag).Args[0].(*value.File)
	if r, _ := fsFn(t, "write_line").Fn(c, []value.Value{h, str("x")}); !strings.Contains(value.Format(r), "'invalid") {
		t.Fatalf("write to a read handle: %s", value.Format(r))
	}
	rl := prelude.Test()["read_line"].(*value.Native)
	if r, _ := rl.Fn(c, []value.Value{h}); value.Format(r) != `Ok("one")` {
		t.Fatalf("read_line: %s", value.Format(r))
	}
	if r, _ := prelude.Test()["read_lines"].(*value.Native).Fn(c, []value.Value{h}); value.Format(r) != `Ok(["two"])` {
		t.Fatalf("read_lines: %s", value.Format(r))
	}
	fsFn(t, "close").Fn(c, []value.Value{h})
	if r, _ := rl.Fn(c, []value.Value{h}); !strings.Contains(value.Format(r), "'closed") {
		t.Fatalf("read after close: %s", value.Format(r))
	}
	if _, err := fsFn(t, "open").Fn(c, []value.Value{str(p), value.Intern("rw")}); err == nil {
		t.Fatal("bad mode is E0403")
	}
	if r, _ := fsFn(t, "open").Fn(c, []value.Value{str(filepath.Join(dir, "nope/x")), value.Intern("read")}); !strings.Contains(value.Format(r), "'not_found") {
		t.Fatalf("open missing: %s", value.Format(r))
	}
	r, _ = fsFn(t, "open").Fn(c, []value.Value{str(p), value.Intern("append")})
	h = r.(*value.Tag).Args[0].(*value.File)
	fsFn(t, "write_line").Fn(c, []value.Value{h, str("three")})
	fsFn(t, "close").Fn(c, []value.Value{h})
	if r, _ := fsFn(t, "read").Fn(c, []value.Value{str(p)}); value.Format(r) != `Ok("one\ntwo\nthree\n")` {
		t.Fatalf("append mode: %s", value.Format(r))
	}
	if value.Format(h) != `<file "`+p+`">` {
		t.Fatalf("format: %s", value.Format(h))
	}
}

func TestPathFunctions(t *testing.T) {
	m := Bridges()["_native.path"]
	cases := []struct {
		name string
		args []value.Value
		want string
	}{
		{"join", []value.Value{list(str("a"), str("b/c"))}, `"a/b/c"`},
		{"join", []value.Value{list()}, `""`},
		{"base", []value.Value{str("/x/y.txt")}, `"y.txt"`},
		{"dir", []value.Value{str("/x/y.txt")}, `"/x"`},
		{"ext", []value.Value{str("a/b.tar.gz")}, `".gz"`},
		{"ext", []value.Value{str("a/b")}, `""`},
		{"stem", []value.Value{str("a/b.tar.gz")}, `"b.tar"`},
	}
	for _, c := range cases {
		v, _ := m.Get(c.name)
		got, err := v.(*value.Native).Fn(&nat.FakeContext{}, c.args)
		if err != nil || value.Format(got) != c.want {
			t.Errorf("%s = %v %v, want %s", c.name, got, err, c.want)
		}
	}
	v, _ := m.Get("abs")
	got, err := v.(*value.Native).Fn(&nat.FakeContext{}, []value.Value{str("x")})
	if err != nil || !filepath.IsAbs(string(got.(value.Str))) {
		t.Errorf("abs = %v %v", got, err)
	}
}

func TestReadLineOnWriteHandle(t *testing.T) {
	dir := t.TempDir()
	c := &nat.FakeContext{}
	r, _ := fsFn(t, "open").Fn(c, []value.Value{str(filepath.Join(dir, "w.txt")), value.Intern("write")})
	h := r.(*value.Tag).Args[0].(*value.File)
	got, _ := prelude.Test()["read_line"].(*value.Native).Fn(c, []value.Value{h})
	if !strings.Contains(value.Format(got), "'invalid") || !strings.Contains(value.Format(got), "not open for reading") {
		t.Fatalf("read_line on a write handle: %s", value.Format(got))
	}
}

func list(items ...value.Value) *value.List { return value.NewList(items) }

// TestFileHandlesSerializeAcrossTasks: tasks sharing a handle get whole
// lines and lose none (06: 핸들 연산은 뮤텍스로 직렬화). Run with -race.
func TestFileHandlesSerializeAcrossTasks(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "lines.txt")
	var text strings.Builder
	for i := 0; i < 2000; i++ {
		text.WriteString("line\n")
	}
	os.WriteFile(p, []byte(text.String()), 0o644)
	c := &nat.FakeContext{}
	h, _ := fsFn(t, "open").Fn(c, []value.Value{str(p), value.Intern("read")})
	readLine := prelude.Core(nil)["read_line"].(*value.Native)
	var total atomic.Int32
	var wg sync.WaitGroup
	for g := 0; g < 8; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for {
				r, _ := readLine.Fn(c, []value.Value{h.(*value.Tag).Args[0]})
				if value.Format(r) == "Ok(nil)" {
					return
				}
				if value.Format(r) != `Ok("line")` {
					t.Errorf("torn read: %s", value.Format(r))
					return
				}
				total.Add(1)
			}
		}()
	}
	wg.Wait()
	if total.Load() != 2000 {
		t.Errorf("read %d lines, want 2000", total.Load())
	}
	out := filepath.Join(dir, "out.txt")
	wv, _ := fsFn(t, "open").Fn(c, []value.Value{str(out), value.Intern("write")})
	w := wv.(*value.Tag).Args[0]
	for g := 0; g < 8; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := 0; i < 100; i++ {
				fsFn(t, "write_line").Fn(c, []value.Value{w, str("abc")})
			}
		}()
	}
	wg.Wait()
	fsFn(t, "close").Fn(c, []value.Value{w})
	b, _ := os.ReadFile(out)
	if got := strings.Count(string(b), "abc\n"); got != 800 || len(b) != 800*4 {
		t.Errorf("wrote %d whole lines in %d bytes, want 800 in 3200", got, len(b))
	}
}

func TestFsListFull(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "a.txt"), []byte(""), 0o644)
	c := &nat.FakeContext{}
	if r, _ := fsFn(t, "list").Fn(c, []value.Value{str(dir), value.Bool(true)}); value.Format(r) != `Ok(["`+filepath.Join(dir, "a.txt")+`"])` {
		t.Errorf("list full: %s", value.Format(r))
	}
	if r, _ := fsFn(t, "list").Fn(c, []value.Value{str(dir), value.Bool(false)}); value.Format(r) != `Ok(["a.txt"])` {
		t.Errorf("list: %s", value.Format(r))
	}
}
