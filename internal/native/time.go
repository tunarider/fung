package native

import (
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/nat"
	"github.com/tunarider/fung/internal/value"
)

// timeModule builds _native.time (06): instants and the token formats.
func timeModule() *value.Module {
	m := value.NewModule("time", "_native.time")
	add := func(n *value.Native) { m.Set(n.Name, n) }
	add(nat.Def("now", timeNowFn))
	add(nat.Def("parse", timeParseFn, nat.Param("s"), nat.Opt("format", value.Nil{})))
	add(nat.Def("format", timeFormatFn, nat.Param("t"), nat.Opt("format", value.Nil{})))
	add(nat.Def("utc", timeUTCFn, nat.Param("t")))
	add(nat.Def("local", timeLocalFn, nat.Param("t")))
	add(nat.Def("unix", timeUnixFn, nat.Param("t")))
	add(nat.Def("from_unix", timeFromUnixFn, nat.Param("n")))
	add(nat.Def("parts", timePartsFn, nat.Param("t")))
	return m
}

func needTime(name string, v value.Value) (time.Time, error) {
	t, ok := v.(value.Time)
	if !ok {
		return time.Time{}, diag.Errorf("E0403", diag.Pos{}, name, "time", v.TypeName())
	}
	return t.T, nil
}

func timeNowFn(_ value.Context, _ []value.Value) (value.Value, error) {
	return value.NewTime(time.Now()), nil
}

func timeParseFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.NeedStr("s", args[0])
	if err != nil {
		return nil, err
	}
	layout, has, err := nat.OptStr("format", args[1])
	if err != nil {
		return nil, err
	}
	var t time.Time
	var perr error
	if has {
		t, perr = parseTime(s, layout)
	} else {
		t, perr = time.Parse(time.RFC3339Nano, s)
	}
	if perr != nil {
		return nat.ErrValue("parse", "cannot parse `"+s+"` as a time: "+perr.Error()), nil
	}
	return nat.Ok(value.NewTime(t)), nil
}

func timeFormatFn(_ value.Context, args []value.Value) (value.Value, error) {
	t, err := needTime("t", args[0])
	if err != nil {
		return nil, err
	}
	layout, has, err := nat.OptStr("format", args[1])
	if err != nil {
		return nil, err
	}
	if !has {
		return value.Str(value.NewTime(t).ISO()), nil
	}
	return value.Str(formatTime(t, layout)), nil
}

func timeUTCFn(_ value.Context, args []value.Value) (value.Value, error) {
	t, err := needTime("t", args[0])
	if err != nil {
		return nil, err
	}
	return value.NewTime(t.UTC()), nil
}

func timeLocalFn(_ value.Context, args []value.Value) (value.Value, error) {
	t, err := needTime("t", args[0])
	if err != nil {
		return nil, err
	}
	return value.NewTime(t.Local()), nil
}

func timeUnixFn(_ value.Context, args []value.Value) (value.Value, error) {
	t, err := needTime("t", args[0])
	if err != nil {
		return nil, err
	}
	return value.Int(t.Unix()), nil
}

func timeFromUnixFn(_ value.Context, args []value.Value) (value.Value, error) {
	n, err := nat.NeedInt("n", args[0])
	if err != nil {
		return nil, err
	}
	if n >= 1<<62 || n <= -(1<<62) {
		// time.Unix wraps silently past the representable years.
		return nil, diag.Errorf("E0403", diag.Pos{}, "n", "a unix time within ±2^62 seconds", value.Format(args[0]))
	}
	return value.NewTime(time.Unix(int64(n), 0)), nil
}

var weekdays = [...]string{"sun", "mon", "tue", "wed", "thu", "fri", "sat"}

func timePartsFn(_ value.Context, args []value.Value) (value.Value, error) {
	t, err := needTime("t", args[0])
	if err != nil {
		return nil, err
	}
	m := value.NewMapBuilder()
	m.Set("year", value.Int(t.Year()))
	m.Set("month", value.Int(int(t.Month())))
	m.Set("day", value.Int(t.Day()))
	m.Set("hour", value.Int(t.Hour()))
	m.Set("minute", value.Int(t.Minute()))
	m.Set("second", value.Int(t.Second()))
	m.Set("weekday", value.Intern(weekdays[t.Weekday()]))
	return m.Map(), nil
}
