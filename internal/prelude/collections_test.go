package prelude

import (
	"testing"

	"github.com/tunarider/fung/internal/value"
)

func list(items ...value.Value) *value.List { return value.NewList(items) }

func callNative(t *testing.T, name string, args ...value.Value) (value.Value, error) {
	t.Helper()
	n := Test()[name].(*value.Native)
	return n.Fn(&ctx{}, args)
}

func TestMapFunctionsCopy(t *testing.T) {
	mB := value.NewMapBuilder()
	mB.Set("a", value.Int(1))
	m := mB.Map()
	put, _ := callNative(t, "put", m, value.Str("b"), value.Int(2))
	removed, _ := callNative(t, "remove", m, value.Str("a"))
	merged, _ := callNative(t, "merge", m, put)
	if value.Format(put) != "{a: 1, b: 2}" || value.Format(removed) != "{}" || value.Format(merged) != "{a: 1, b: 2}" {
		t.Fatalf("put=%s removed=%s merged=%s", value.Format(put), value.Format(removed), value.Format(merged))
	}
	if value.Format(m) != "{a: 1}" {
		t.Fatal("the input map changed")
	}
}

// flatten stays in Go (06): a fold with `acc + x` copies the accumulator
// per element, so the fung version was quadratic.
func TestFlattenSplicesOneLevel(t *testing.T) {
	f, err := callNative(t, "flatten", list(list(value.Int(1)), value.Int(2), list(), list(list(value.Int(3)))))
	if err != nil || value.Format(f) != "[1, 2, [3]]" {
		t.Fatalf("flatten = %s, %v", value.Format(f), err)
	}
}
