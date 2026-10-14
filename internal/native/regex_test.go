package native

import (
	"strings"
	"testing"
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/nat"
	"github.com/tunarider/fung/internal/value"
)

func reFn(t *testing.T, name string) *value.Native {
	t.Helper()
	v, ok := Bridges()["_native.regex"].Get(name)
	if !ok {
		t.Fatalf("no %s", name)
	}
	return v.(*value.Native)
}

func TestRegexFindUsesRuneIndexes(t *testing.T) {
	c := &nat.FakeContext{}
	r, err := reFn(t, "find").Fn(c, []value.Value{value.Str("가나다 나다"), value.Str("나(다)")})
	if err != nil {
		t.Fatal(err)
	}
	if got := value.Format(r); got != `{text: "나다", start: 1, end: 3, groups: ["다"], named: {}}` {
		t.Errorf("find = %s", got)
	}
	r, _ = reFn(t, "find").Fn(c, []value.Value{value.Str("abc"), value.Str("z")})
	if _, ok := r.(value.Nil); !ok {
		t.Errorf("no match must be nil, got %s", value.Format(r))
	}
	r, _ = reFn(t, "find").Fn(c, []value.Value{value.Str("k=v"), value.Str(`(?P<key>\w+)=(?P<val>\w+)(x)?`)})
	if got := value.Format(r); got != `{text: "k=v", start: 0, end: 3, groups: ["k", "v", nil], named: {key: "k", val: "v"}}` {
		t.Errorf("named groups = %s", got)
	}
}

func TestRegexOthers(t *testing.T) {
	c := &nat.FakeContext{}
	if r, _ := reFn(t, "is_match").Fn(c, []value.Value{value.Str("file.log"), value.Str(`\.log$`)}); r != value.Bool(true) {
		t.Error("match")
	}
	if r, _ := reFn(t, "is_match").Fn(c, []value.Value{value.Str("file.txt"), value.Str(`\.log$`)}); r != value.Bool(false) {
		t.Error("match false")
	}
	r, _ := reFn(t, "find_all").Fn(c, []value.Value{value.Str("a1 b22 c333"), value.Str(`\d+`)})
	if got := value.Format(r); got != `[{text: "1", start: 1, end: 2, groups: [], named: {}}, {text: "22", start: 4, end: 6, groups: [], named: {}}, {text: "333", start: 8, end: 11, groups: [], named: {}}]` {
		t.Errorf("find_all = %s", got)
	}
	r, _ = reFn(t, "find_all").Fn(c, []value.Value{value.Str("none"), value.Str(`\d`)})
	if value.Format(r) != "[]" {
		t.Errorf("find_all none = %s", value.Format(r))
	}
	r, _ = reFn(t, "replace").Fn(c, []value.Value{value.Str("2026-10-04"), value.Str(`(\d+)-(\d+)-(\d+)`), value.Str("$3/$2/$1")})
	if r != value.Str("04/10/2026") {
		t.Errorf("replace = %s", value.Format(r))
	}
	r, _ = reFn(t, "split").Fn(c, []value.Value{value.Str("a, b,c ,"), value.Str(`\s*,\s*`)})
	if value.Format(r) != `["a", "b", "c", ""]` {
		t.Errorf("split = %s", value.Format(r))
	}
	_, err := reFn(t, "is_match").Fn(c, []value.Value{value.Str("x"), value.Str("(")})
	if err == nil || err.(*diag.Error).Code != "E0515" {
		t.Errorf("bad pattern must be E0515, got %v", err)
	}
	_, err = reFn(t, "is_match").Fn(c, []value.Value{value.Int(1), value.Str("x")})
	if err == nil || err.(*diag.Error).Code != "E0403" {
		t.Errorf("non-string subject must be E0403, got %v", err)
	}
}

func TestRegexCache(t *testing.T) {
	a, _ := compilePattern(`\d+`)
	b, _ := compilePattern(`\d+`)
	if a != b {
		t.Error("the same pattern must come from the cache")
	}
	for i := 0; i < maxCachedPatterns+5; i++ {
		compilePattern(`x{` + string(rune('0'+i%10)) + `}` + string(rune('a'+i%26)))
	}
	if n := len(patternCache); n > maxCachedPatterns {
		t.Errorf("cache grew to %d", n)
	}
}

// TestRegexFindAllIsLinear: rune positions must not be recounted from
// the start of the subject for every match (a 300KB log would take
// seconds).
func TestRegexFindAllIsLinear(t *testing.T) {
	s := strings.Repeat("가 1 ", 50_000) // 300KB, 50k matches, multibyte text
	start := time.Now()
	r, err := reFn(t, "find_all").Fn(&nat.FakeContext{}, []value.Value{value.Str(s), value.Str(`\d`)})
	if err != nil {
		t.Fatal(err)
	}
	if took := time.Since(start); took > time.Second {
		t.Fatalf("find_all over 300KB took %s", took)
	}
	items := r.(*value.List).ToSlice()
	last := items[len(items)-1].(*value.Map)
	end, _ := last.Get("end")
	if len(items) != 50_000 || end != value.Int(50_000*4-1) {
		t.Errorf("got %d matches, last end %s", len(items), value.Format(end))
	}
}
