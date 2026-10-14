package value

import (
	"math"
	"strconv"
	"strings"
	"time"

	"github.com/tunarider/fung/internal/diag"
)

// Dur is a duration with nanosecond precision (02).
type Dur time.Duration

func (Dur) TypeName() string { return "dur" }

// formatDur prints the literal form: descending h, m, s, ms components,
// the last one possibly fractional (1h30m, 1s500ms, 0.5ms, 0s).
func formatDur(d Dur) string {
	if d == 0 {
		return "0s"
	}
	var b strings.Builder
	ns := int64(d)
	if ns < 0 {
		b.WriteByte('-')
		ns = -ns
	}
	h, ns := ns/int64(time.Hour), ns%int64(time.Hour)
	m, ns := ns/int64(time.Minute), ns%int64(time.Minute)
	s, ns := ns/int64(time.Second), ns%int64(time.Second)
	if h > 0 {
		b.WriteString(strconv.FormatInt(h, 10) + "h")
	}
	if m > 0 {
		b.WriteString(strconv.FormatInt(m, 10) + "m")
	}
	if s > 0 {
		b.WriteString(strconv.FormatInt(s, 10) + "s")
	}
	if ns > 0 {
		ms := float64(ns) / float64(time.Millisecond)
		b.WriteString(strconv.FormatFloat(ms, 'f', -1, 64) + "ms")
	}
	return b.String()
}

func addDur(a, b Dur) (Value, *diag.Error) {
	c := a + b
	if (b > 0 && c < a) || (b < 0 && c > a) {
		return nil, overflow()
	}
	return c, nil
}

// scaleDur multiplies a duration by a number, rejecting overflow.
func scaleDur(d Dur, n Value) (Value, *diag.Error) {
	var f float64
	switch x := n.(type) {
	case Int:
		c, ok := mulInt(int64(d), int64(x))
		if !ok {
			return nil, overflow()
		}
		return Dur(c), nil
	case Float:
		f = float64(x)
	default:
		return nil, diag.Errorf("E0501", diag.Pos{}, "multiply", d.TypeName(), n.TypeName())
	}
	r := math.Round(float64(d) * f)
	if math.IsNaN(r) || math.Abs(r) >= float64(math.MaxInt64) {
		return nil, overflow()
	}
	return Dur(r), nil
}

// divDur divides a duration by a number.
func divDur(d Dur, n Value) (Value, *diag.Error) {
	var f float64
	switch x := n.(type) {
	case Int:
		f = float64(x)
	case Float:
		f = float64(x)
	default:
		return nil, diag.Errorf("E0501", diag.Pos{}, "divide", d.TypeName(), n.TypeName())
	}
	if f == 0 {
		return nil, divZero()
	}
	r := math.Round(float64(d) / f)
	if math.IsNaN(r) || math.Abs(r) >= float64(math.MaxInt64) {
		return nil, overflow()
	}
	return Dur(r), nil
}
