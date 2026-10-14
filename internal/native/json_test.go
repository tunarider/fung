package native

import (
	"strings"
	"testing"
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/nat"
	"github.com/tunarider/fung/internal/value"
)

func jsonFn(t *testing.T, name string) *value.Native {
	t.Helper()
	v, ok := Bridges()["_native.json"].Get(name)
	if !ok {
		t.Fatalf("no %s", name)
	}
	return v.(*value.Native)
}

func TestJSONParse(t *testing.T) {
	cases := map[string]string{
		`{"b": 1, "a": [true, null, "x"], "n": 1.5, "e": 1e2, "one": 1.0, "big": 12345678901}`: `Ok({b: 1, a: [true, nil, "x"], n: 1.5, e: 100.0, one: 1.0, big: 12345678901})`,
		`"가\n"`:                   `Ok("가\n")`,
		`[]`:                      `Ok([])`,
		`{}`:                      `Ok({})`,
		`-0.0`:                    `Ok(-0.0)`,
		`{"a": {"b": {"c": []}}}`: `Ok({a: {b: {c: []}}})`,
	}
	for in, want := range cases {
		r, err := jsonFn(t, "parse").Fn(&nat.FakeContext{}, []value.Value{value.Str(in)})
		if err != nil {
			t.Fatalf("parse(%s): %v", in, err)
		}
		if got := value.Format(r); got != want {
			t.Errorf("parse(%s) = %s, want %s", in, got, want)
		}
	}
	bad := map[string]string{
		"{\"a\": 1,}":         "line: 1, col: 8",
		"[1, 2":               "line: 1, col: 6",
		"{\n  \"a\": tru\n}":  "line: 2, col: 11",
		"9223372036854775808": "line: 1, col: 1",
		"1 2":                 "line: 1, col: 3",
		"":                    "line: 1, col: 1",
	}
	for in, want := range bad {
		r, _ := jsonFn(t, "parse").Fn(&nat.FakeContext{}, []value.Value{value.Str(in)})
		got := value.Format(r)
		if !strings.HasPrefix(got, "Err({kind: 'parse, ") || !strings.Contains(got, want) {
			t.Errorf("parse(%q) = %s, want Err with %s", in, got, want)
		}
	}
}

func TestJSONStringify(t *testing.T) {
	m := value.NewMapBuilder()
	m.Set("b", value.Int(1))
	m.Set("a", value.NewList([]value.Value{value.Bool(true), value.Nil{}, value.Str("x<y"), value.Float(2)}))
	m.Set("t", &value.Tuple{Items: []value.Value{value.Int(1), value.Int(2)}})
	m.Set("s", value.Intern("ok"))
	m.Set("when", value.NewTime(time.Date(2026, 10, 4, 0, 0, 0, 0, time.UTC)))
	r, err := jsonFn(t, "stringify").Fn(&nat.FakeContext{}, []value.Value{m.Map(), value.Nil{}})
	if err != nil {
		t.Fatal(err)
	}
	want := `{"b":1,"a":[true,null,"x<y",2.0],"t":[1,2],"s":"ok","when":"2026-10-04T00:00:00Z"}`
	if string(r.(value.Str)) != want {
		t.Errorf("stringify = %s\nwant        %s", r, want)
	}
	r, _ = jsonFn(t, "stringify").Fn(&nat.FakeContext{}, []value.Value{value.NewList([]value.Value{value.Int(1), value.NewMap()}), value.Int(2)})
	if string(r.(value.Str)) != "[\n  1,\n  {}\n]" {
		t.Errorf("indent: %q", r)
	}
	_, err = jsonFn(t, "stringify").Fn(&nat.FakeContext{}, []value.Value{value.Dur(1), value.Nil{}})
	if err == nil || err.(*diag.Error).Code != "E0513" {
		t.Errorf("stringify(dur) must be E0513, got %v", err)
	}
	_, err = jsonFn(t, "stringify").Fn(&nat.FakeContext{}, []value.Value{value.Int(1), value.Str("x")})
	if err == nil || err.(*diag.Error).Code != "E0403" {
		t.Errorf("indent must be int | nil, got %v", err)
	}
}

func TestJSONRoundTripKeepsOrder(t *testing.T) {
	in := `{"z":1,"y":{"b":2,"a":3},"x":[{"k":"v"}]}`
	r, _ := jsonFn(t, "parse").Fn(&nat.FakeContext{}, []value.Value{value.Str(in)})
	out, _ := jsonFn(t, "stringify").Fn(&nat.FakeContext{}, []value.Value{r.(*value.Tag).Args[0], value.Nil{}})
	if string(out.(value.Str)) != in {
		t.Errorf("round trip changed the text: %s", out)
	}
}

// Columns point at the character the message names, for every kind of
// syntax error (not only "invalid character").
func TestJSONErrorColumns(t *testing.T) {
	cases := map[string]string{
		`{"a": [1, {"b": }]}`:       "line: 1, col: 17", // the `}` where a value was expected
		"{\"a\":\n  1\n  \"b\": 2}": "line: 3, col: 3",  // the `"` where `,` or `}` was expected
		`[1 2]`:                     "line: 1, col: 4",
	}
	for in, want := range cases {
		r, _ := jsonFn(t, "parse").Fn(&nat.FakeContext{}, []value.Value{value.Str(in)})
		if got := value.Format(r); !strings.Contains(got, want) {
			t.Errorf("parse(%q) = %s, want %s", in, got, want)
		}
	}
}
