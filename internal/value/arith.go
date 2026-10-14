package value

import (
	"math"

	"github.com/tunarider/fung/internal/diag"
)

// numeric coerces int/float pairs: both ints stay ints, otherwise both
// become floats (07: 정수와 실수를 섞으면 실수 결과).
func numeric(verb string, a, b Value) (ai, bi int64, af, bf float64, isInt bool, err *diag.Error) {
	switch x := a.(type) {
	case Int:
		switch y := b.(type) {
		case Int:
			return int64(x), int64(y), 0, 0, true, nil
		case Float:
			return 0, 0, float64(x), float64(y), false, nil
		}
	case Float:
		switch y := b.(type) {
		case Int:
			return 0, 0, float64(x), float64(y), false, nil
		case Float:
			return 0, 0, float64(x), float64(y), false, nil
		}
	}
	return 0, 0, 0, 0, false, diag.Errorf("E0501", diag.Pos{}, verb, a.TypeName(), b.TypeName())
}

// finite rejects Inf and NaN (07: fung 값에는 Inf·NaN이 없다).
func finite(f float64) (Value, *diag.Error) {
	if math.IsInf(f, 0) || math.IsNaN(f) {
		return nil, diag.Errorf("E0504", diag.Pos{})
	}
	return Float(f), nil
}

func overflow() *diag.Error { return diag.Errorf("E0503", diag.Pos{}) }
func divZero() *diag.Error  { return diag.Errorf("E0502", diag.Pos{}) }

func Add(a, b Value) (Value, *diag.Error) {
	switch x := a.(type) {
	case Str:
		if y, ok := b.(Str); ok {
			return x + y, nil
		}
		return nil, diag.Errorf("E0501", diag.Pos{}, "add", a.TypeName(), b.TypeName())
	case Dur:
		switch y := b.(type) {
		case Dur:
			return addDur(x, y)
		case Time:
			return addTimeDur(y, x)
		}
		return nil, diag.Errorf("E0501", diag.Pos{}, "add", a.TypeName(), b.TypeName())
	case Time:
		if y, ok := b.(Dur); ok {
			return addTimeDur(x, y)
		}
		return nil, diag.Errorf("E0501", diag.Pos{}, "add", a.TypeName(), b.TypeName())
	case *List:
		if y, ok := b.(*List); ok {
			return x.Concat(y), nil
		}
		return nil, diag.Errorf("E0501", diag.Pos{}, "add", a.TypeName(), b.TypeName())
	}
	ai, bi, af, bf, isInt, err := numeric("add", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt {
		return finite(af + bf)
	}
	c := ai + bi
	if (bi > 0 && c < ai) || (bi < 0 && c > ai) {
		return nil, overflow()
	}
	return Int(c), nil
}

func Sub(a, b Value) (Value, *diag.Error) {
	if x, ok := a.(Time); ok {
		switch y := b.(type) {
		case Dur:
			if y == Dur(math.MinInt64) {
				return nil, overflow()
			}
			return addTimeDur(x, -y)
		case Time:
			return subTimes(x, y)
		}
		return nil, diag.Errorf("E0501", diag.Pos{}, "subtract", a.TypeName(), b.TypeName())
	}
	if x, ok := a.(Dur); ok {
		y, ok := b.(Dur)
		if !ok {
			return nil, diag.Errorf("E0501", diag.Pos{}, "subtract", a.TypeName(), b.TypeName())
		}
		if y == Dur(math.MinInt64) {
			return nil, overflow()
		}
		return addDur(x, -y)
	}
	ai, bi, af, bf, isInt, err := numeric("subtract", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt {
		return finite(af - bf)
	}
	c := ai - bi
	if (bi > 0 && c > ai) || (bi < 0 && c < ai) {
		return nil, overflow()
	}
	return Int(c), nil
}

// mulInt multiplies with overflow detection.
func mulInt(a, b int64) (int64, bool) {
	if a == 0 || b == 0 {
		return 0, true
	}
	c := a * b
	if c/b != a || (a == -1 && b == math.MinInt64) || (b == -1 && a == math.MinInt64) {
		return 0, false
	}
	return c, true
}

func Mul(a, b Value) (Value, *diag.Error) {
	if d, ok := a.(Dur); ok {
		return scaleDur(d, b)
	}
	if d, ok := b.(Dur); ok {
		return scaleDur(d, a)
	}
	ai, bi, af, bf, isInt, err := numeric("multiply", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt {
		return finite(af * bf)
	}
	c, ok := mulInt(ai, bi)
	if !ok {
		return nil, overflow()
	}
	return Int(c), nil
}

// Div is `/`: always a float result for numbers; a duration divided by a
// number stays a duration.
func Div(a, b Value) (Value, *diag.Error) {
	if d, ok := a.(Dur); ok {
		return divDur(d, b)
	}
	ai, bi, af, bf, isInt, err := numeric("divide", a, b)
	if err != nil {
		return nil, err
	}
	if isInt {
		af, bf = float64(ai), float64(bi)
	}
	if bf == 0 {
		return nil, divZero()
	}
	return finite(af / bf)
}

// FloorDiv is `//`: floor division, ints stay ints.
func FloorDiv(a, b Value) (Value, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("divide", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt {
		if bf == 0 {
			return nil, divZero()
		}
		return finite(math.Floor(af / bf))
	}
	if bi == 0 {
		return nil, divZero()
	}
	if ai == math.MinInt64 && bi == -1 {
		return nil, overflow()
	}
	q := ai / bi
	if ai%bi != 0 && (ai < 0) != (bi < 0) {
		q--
	}
	return Int(q), nil
}

// Mod is `%`, paired with FloorDiv: the result has the sign of the divisor.
func Mod(a, b Value) (Value, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("take the remainder of", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt {
		if bf == 0 {
			return nil, divZero()
		}
		return finite(af - bf*math.Floor(af/bf))
	}
	if bi == 0 {
		return nil, divZero()
	}
	if ai == math.MinInt64 && bi == -1 {
		return Int(0), nil
	}
	r := ai % bi
	if r != 0 && (r < 0) != (bi < 0) {
		r += bi
	}
	return Int(r), nil
}

// Pow is `**`: int ** non-negative int stays int (checked), otherwise float.
func Pow(a, b Value) (Value, *diag.Error) {
	ai, bi, af, bf, isInt, err := numeric("raise", a, b)
	if err != nil {
		return nil, err
	}
	if !isInt || bi < 0 {
		if isInt {
			af, bf = float64(ai), float64(bi)
		}
		return finite(math.Pow(af, bf))
	}
	result, base, exp := int64(1), ai, bi
	for exp > 0 {
		if exp&1 == 1 {
			r, ok := mulInt(result, base)
			if !ok {
				return nil, overflow()
			}
			result = r
		}
		exp >>= 1
		if exp > 0 {
			sq, ok := mulInt(base, base)
			if !ok {
				return nil, overflow()
			}
			base = sq
		}
	}
	return Int(result), nil
}

func Neg(a Value) (Value, *diag.Error) {
	switch x := a.(type) {
	case Int:
		if x == math.MinInt64 {
			return nil, overflow()
		}
		return -x, nil
	case Float:
		return -x, nil
	case Dur:
		if x == Dur(math.MinInt64) {
			return nil, overflow()
		}
		return -x, nil
	}
	return nil, diag.Errorf("E0506", diag.Pos{}, "negate", a.TypeName())
}

// Not requires a bool (07: truthiness 없음).
func Not(a Value) (Value, *diag.Error) {
	if b, ok := a.(Bool); ok {
		return !b, nil
	}
	return nil, diag.Errorf("E0505", diag.Pos{}, a.TypeName())
}
