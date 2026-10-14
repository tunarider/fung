package native

import (
	"strings"
	"testing"
	"time"

	"github.com/tunarider/fung/internal/nat"
	"github.com/tunarider/fung/internal/value"
)

func timeFn(t *testing.T, name string) *value.Native {
	t.Helper()
	v, ok := Bridges()["_native.time"].Get(name)
	if !ok {
		t.Fatalf("no %s", name)
	}
	return v.(*value.Native)
}

func call(t *testing.T, fn *value.Native, args ...value.Value) value.Value {
	t.Helper()
	v, err := fn.Fn(&nat.FakeContext{}, args)
	if err != nil {
		t.Fatalf("%s: %v", fn.Name, err)
	}
	return v
}

func TestTimeModule(t *testing.T) {
	iso := value.Str("2026-10-04T09:05:07+09:00")
	parsed := call(t, timeFn(t, "parse"), iso, value.Nil{})
	if value.Format(parsed) != "Ok(2026-10-04T09:05:07+09:00)" {
		t.Fatalf("parse default: %s", value.Format(parsed))
	}
	at := parsed.(*value.Tag).Args[0]
	if r := call(t, timeFn(t, "format"), at, value.Nil{}); r != iso {
		t.Errorf("format default: %s", value.Format(r))
	}
	if r := call(t, timeFn(t, "format"), at, value.Str("YYYY/MM/DD HH:mm")); r != value.Str("2026/10/04 09:05") {
		t.Errorf("format tokens: %s", value.Format(r))
	}
	if r := call(t, timeFn(t, "parse"), value.Str("2026/10/04 09:05"), value.Str("YYYY/MM/DD HH:mm")); !strings.HasPrefix(value.Format(r), "Ok(2026-10-04T09:05:00") {
		t.Errorf("parse tokens: %s", value.Format(r))
	}
	if r := call(t, timeFn(t, "parse"), value.Str("nope"), value.Nil{}); !strings.HasPrefix(value.Format(r), "Err({kind: 'parse, message: ") {
		t.Errorf("parse error: %s", value.Format(r))
	}
	if r := call(t, timeFn(t, "utc"), at); value.Format(r) != "2026-10-04T00:05:07Z" {
		t.Errorf("utc: %s", value.Format(r))
	}
	if r := call(t, timeFn(t, "unix"), at); r != value.Int(1791072307) {
		t.Errorf("unix: %s", value.Format(r))
	}
	if r := call(t, timeFn(t, "from_unix"), value.Int(1791072307)); !value.Equal(r, at) {
		t.Errorf("from_unix: %s", value.Format(r))
	}
	if r := call(t, timeFn(t, "parts"), at); value.Format(r) != "{year: 2026, month: 10, day: 4, hour: 9, minute: 5, second: 7, weekday: 'sun}" {
		t.Errorf("parts: %s", value.Format(r))
	}
	now := call(t, timeFn(t, "now"))
	if _, ok := now.(value.Time); !ok || time.Since(now.(value.Time).T) > time.Minute {
		t.Errorf("now: %s", value.Format(now))
	}
	if r := call(t, timeFn(t, "local"), call(t, timeFn(t, "utc"), at)); !value.Equal(r, at) || r.(value.Time).T.Location() != time.Local {
		t.Errorf("local: %s", value.Format(r))
	}
	if _, err := timeFn(t, "format").Fn(&nat.FakeContext{}, []value.Value{value.Int(1), value.Nil{}}); err == nil {
		t.Error("format(1) must be E0403")
	}
	if _, err := timeFn(t, "parse").Fn(&nat.FakeContext{}, []value.Value{iso, value.Int(1)}); err == nil {
		t.Error("parse(s, 1) must be E0403")
	}
	if _, err := timeFn(t, "from_unix").Fn(&nat.FakeContext{}, []value.Value{value.Int(1 << 62)}); err == nil {
		t.Error("from_unix beyond the representable range must be E0403")
	}
}
