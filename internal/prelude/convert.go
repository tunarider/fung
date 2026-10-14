package prelude

import (
	"github.com/tunarider/fung/internal/nat"
	"math"
	"strconv"
	"strings"
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/lexer"
	"github.com/tunarider/fung/internal/value"
)

// convertFns registers conversions and result predicates (06). int, float,
// str and sym assert ("this must be a number": a runtime error when it is
// not); parse_int and parse_float check and return Ok/Err.
func convertFns(add func(*value.Native)) {
	add(nat.Def("int", intFn, nat.Param("x")))
	add(nat.Def("float", floatFn, nat.Param("x")))
	add(nat.Def("str", strFn, nat.Param("x")))
	add(nat.Def("sym", symFn, nat.Param("x")))
	add(nat.Def("parse_int", parseIntFn, nat.Param("s")))
	add(nat.Def("parse_float", parseFloatFn, nat.Param("s")))
	add(nat.Def("parse_dur", parseDurFn, nat.Param("s")))
}

// parseDurFn reads a duration in the literal syntax (01: 1h30m, 500ms,
// -5s), the way parse_int reads an int: Ok/Err for untrusted text.
func parseDurFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.NeedStr("s", args[0])
	if err != nil {
		return nil, err
	}
	if d, ok := parseDur(strings.TrimSpace(s)); ok {
		return ok_(value.Dur(d)), nil
	}
	return nat.ErrValue("parse", "not a duration: "+s), nil
}

// parseDur accepts exactly one duration literal, optionally negated.
func parseDur(s string) (time.Duration, bool) {
	neg := strings.HasPrefix(s, "-")
	if neg {
		s = s[1:]
	}
	toks, _, err := lexer.Lex("<parse_dur>", s)
	if err != nil || len(toks) == 0 || toks[0].Kind != lexer.Duration {
		return 0, false
	}
	for _, t := range toks[1:] {
		if t.Kind != lexer.EOF && t.Kind != lexer.Newline {
			return 0, false
		}
	}
	d := toks[0].Dur
	if neg {
		d = -d
	}
	return d, true
}

func cannotConvert(v value.Value, to string) error {
	return diag.Errorf("E0513", diag.Pos{}, value.Format(v), to)
}

// wellPlacedUnderscores reports whether every `_` sits between two
// digits (01: 숫자 사이에만).
func wellPlacedUnderscores(s string) bool {
	rs := []rune(s)
	for i, r := range rs {
		if r != '_' {
			continue
		}
		if i == 0 || i == len(rs)-1 || !isDigitRune(rs[i-1]) || !isDigitRune(rs[i+1]) {
			return false
		}
	}
	return true
}

func isDigitRune(r rune) bool { return r >= '0' && r <= '9' }

// parseInt accepts an optional sign, digits and `_` separators (01).
func parseInt(s string) (int64, bool) {
	if !isSpaceFree(s) || !wellPlacedUnderscores(s) {
		return 0, false
	}
	n, err := strconv.ParseInt(strings.ReplaceAll(s, "_", ""), 10, 64)
	return n, err == nil
}

func parseFloat(s string) (float64, bool) {
	if !isSpaceFree(s) || !wellPlacedUnderscores(s) {
		return 0, false
	}
	f, err := strconv.ParseFloat(strings.ReplaceAll(s, "_", ""), 64)
	return f, err == nil && !math.IsInf(f, 0) && !math.IsNaN(f)
}

func intFn(_ value.Context, args []value.Value) (value.Value, error) {
	switch x := args[0].(type) {
	case value.Int:
		return x, nil
	case value.Float:
		f := float64(x)
		if math.IsInf(f, 0) || math.IsNaN(f) || f >= math.MaxInt64 || f < math.MinInt64 {
			return nil, cannotConvert(x, "int")
		}
		return value.Int(int64(f)), nil
	case value.Str:
		if n, ok := parseInt(string(x)); ok {
			return value.Int(n), nil
		}
	}
	return nil, cannotConvert(args[0], "int")
}

func floatFn(_ value.Context, args []value.Value) (value.Value, error) {
	switch x := args[0].(type) {
	case value.Int:
		return value.Float(float64(x)), nil
	case value.Float:
		return x, nil
	case value.Str:
		if f, ok := parseFloat(string(x)); ok {
			return value.Float(f), nil
		}
	}
	return nil, cannotConvert(args[0], "float")
}

func strFn(_ value.Context, args []value.Value) (value.Value, error) {
	if s, ok := args[0].(value.Str); ok {
		return s, nil
	}
	return value.Str(value.Format(args[0])), nil
}

func symFn(_ value.Context, args []value.Value) (value.Value, error) {
	switch x := args[0].(type) {
	case value.Str:
		if value.IsName(string(x)) {
			return value.Intern(string(x)), nil
		}
	case value.Sym:
		return x, nil
	}
	return nil, cannotConvert(args[0], "sym")
}

func parseIntFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.NeedStr("s", args[0])
	if err != nil {
		return nil, err
	}
	if n, ok := parseInt(s); ok {
		return ok_(value.Int(n)), nil
	}
	return nat.ErrValue("parse", "not an integer: "+s), nil
}

func parseFloatFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.NeedStr("s", args[0])
	if err != nil {
		return nil, err
	}
	if f, ok := parseFloat(s); ok {
		return ok_(value.Float(f)), nil
	}
	return nat.ErrValue("parse", "not a number: "+s), nil
}

// ok_ is ok under a name that does not shadow the bool in parse*Fn.
func ok_(v value.Value) value.Value { return nat.Ok(v) }
