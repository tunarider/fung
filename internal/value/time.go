package value

import (
	"math"
	"time"

	"github.com/tunarider/fung/internal/diag"
)

// Time is an instant with its time zone (02). It has no literal form:
// time.now(), time.parse and time.from_unix make it.
type Time struct{ T time.Time }

func NewTime(t time.Time) Time { return Time{T: t} }

func (Time) TypeName() string { return "time" }

// ISO renders the value as RFC 3339 with fractional seconds only when
// they are not zero (06: 시간 형식).
func (t Time) ISO() string { return t.T.Format(time.RFC3339Nano) }

// addTimeDur is time + dur / time - dur (07).
func addTimeDur(t Time, d Dur) (Value, *diag.Error) {
	r := t.T.Add(time.Duration(d))
	if (d > 0 && !r.After(t.T)) || (d < 0 && !r.Before(t.T)) {
		return nil, overflow()
	}
	return Time{T: r}, nil
}

// subTimes is time - time → dur. Go's Sub saturates at the duration
// range; that is an overflow here (07: 범위를 넘으면 에러).
func subTimes(a, b Time) (Value, *diag.Error) {
	d := a.T.Sub(b.T)
	if d == time.Duration(math.MaxInt64) || d == time.Duration(math.MinInt64) {
		return nil, overflow()
	}
	return Dur(d), nil
}
