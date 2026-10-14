package value

import (
	"math"
	"testing"

	"github.com/tunarider/fung/internal/diag"
)

type binop func(a, b Value) (Value, *diag.Error)

func ok(t *testing.T, f binop, a, b Value, want Value) {
	t.Helper()
	got, err := f(a, b)
	if err != nil {
		t.Fatalf("%v op %v: %v", a, b, err)
	}
	if got != want {
		t.Fatalf("%v op %v = %#v, want %#v", a, b, got, want)
	}
}

func fails(t *testing.T, f binop, a, b Value, code string) {
	t.Helper()
	_, err := f(a, b)
	if err == nil || err.Code != code {
		t.Fatalf("%v op %v: err = %v, want %s", a, b, err, code)
	}
}

func TestAddSubMul(t *testing.T) {
	ok(t, Add, Int(1), Int(2), Int(3))
	ok(t, Add, Int(1), Float(2.5), Float(3.5))
	ok(t, Sub, Float(1), Int(3), Float(-2))
	ok(t, Mul, Int(6), Int(7), Int(42))
	fails(t, Add, Int(1), Bool(true), "E0501")
	fails(t, Mul, Nil{}, Int(1), "E0501")
}

func TestIntegerOverflow(t *testing.T) {
	max, min := Int(math.MaxInt64), Int(math.MinInt64)
	fails(t, Add, max, Int(1), "E0503")
	fails(t, Sub, min, Int(1), "E0503")
	fails(t, Mul, max, Int(2), "E0503")
	fails(t, Mul, min, Int(-1), "E0503")
	fails(t, FloorDiv, min, Int(-1), "E0503")
	fails(t, Pow, Int(2), Int(63), "E0503")
	ok(t, Pow, Int(2), Int(62), Int(1<<62))
	ok(t, Add, max, Int(-1), Int(math.MaxInt64-1))
}

func TestDivision(t *testing.T) {
	ok(t, Div, Int(10), Int(4), Float(2.5))
	ok(t, Div, Int(10), Int(2), Float(5))
	ok(t, FloorDiv, Int(7), Int(2), Int(3))
	ok(t, FloorDiv, Int(-7), Int(2), Int(-4))
	ok(t, FloorDiv, Float(7.5), Int(2), Float(3))
	ok(t, Mod, Int(7), Int(3), Int(1))
	ok(t, Mod, Int(-7), Int(3), Int(2))
	ok(t, Mod, Float(-7.5), Int(2), Float(0.5))
	for _, f := range []binop{Div, FloorDiv, Mod} {
		fails(t, f, Int(1), Int(0), "E0502")
		fails(t, f, Float(1), Float(0), "E0502")
		fails(t, f, Int(1), Float(0), "E0502")
	}
}

func TestPow(t *testing.T) {
	ok(t, Pow, Int(2), Int(10), Int(1024))
	ok(t, Pow, Int(2), Int(0), Int(1))
	ok(t, Pow, Int(-2), Int(3), Int(-8))
	ok(t, Pow, Int(2), Int(-1), Float(0.5))
	ok(t, Pow, Float(2), Int(2), Float(4))
	ok(t, Pow, Int(4), Float(0.5), Float(2))
	fails(t, Pow, Int(2), Bool(true), "E0501")
}

func TestNonFiniteResultsAreErrors(t *testing.T) {
	fails(t, Mul, Float(1e308), Float(10), "E0504")
	fails(t, Pow, Float(10), Int(400), "E0504")
	fails(t, Sub, Float(math.Inf(1)), Float(math.Inf(1)), "E0504") // defensive: Inf never exists, but if it did
}

func TestUnary(t *testing.T) {
	if v, _ := Neg(Int(3)); v != Int(-3) {
		t.Fatalf("Neg(3) = %v", v)
	}
	if v, _ := Neg(Float(2.5)); v != Float(-2.5) {
		t.Fatalf("Neg(2.5) = %v", v)
	}
	if _, err := Neg(Int(math.MinInt64)); err == nil || err.Code != "E0503" {
		t.Fatalf("Neg(min) err = %v", err)
	}
	if _, err := Neg(Bool(true)); err == nil || err.Code != "E0506" {
		t.Fatalf("Neg(true) err = %v", err)
	}
	if v, _ := Not(Bool(true)); v != Bool(false) {
		t.Fatalf("Not(true) = %v", v)
	}
	if _, err := Not(Int(1)); err == nil || err.Code != "E0505" {
		t.Fatalf("Not(1) err = %v", err)
	}
}

func TestErrorMessagesNameTheOperation(t *testing.T) {
	_, err := Add(Int(1), Bool(true))
	if got := err.Error(); got != "error[E0501]: cannot add int and bool" {
		t.Fatalf("got %q", got)
	}
	_, err = Neg(Bool(true))
	if got := err.Error(); got != "error[E0506]: cannot negate bool" {
		t.Fatalf("got %q", got)
	}
}
