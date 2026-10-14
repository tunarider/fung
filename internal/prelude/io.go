package prelude

import (
	"github.com/tunarider/fung/internal/nat"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// exitFn raises the exit signal (06): the VM unwinds, the command runs
// at_exit handlers, then the process ends with the code.
func exitFn(_ value.Context, args []value.Value) (value.Value, error) {
	code, ok := args[0].(value.Int)
	if !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "code", "int", args[0].TypeName())
	}
	return nil, diag.ExitSignal(int(code))
}

// dbgFn prints a value to standard error with its source text and place
// (the compiler appends both for a direct dbg(e) call, 08) and returns it.
func dbgFn(ctx value.Context, args []value.Value) (value.Value, error) {
	line := "dbg = " + value.Format(args[0])
	if text, ok := args[1].(value.Str); ok {
		where := ""
		if w, ok := args[2].(value.Str); ok {
			where = "[" + string(w) + "] "
		}
		line = where + string(text) + " = " + value.Format(args[0])
	}
	_, err := ctx.Stderr().Write([]byte(line + "\n"))
	return args[0], err
}

// eprintFn is print to standard error.
func eprintFn(ctx value.Context, args []value.Value) (value.Value, error) {
	_, err := ctx.Stderr().Write([]byte(nat.JoinForPrint(args[0].(*value.List).ToSlice()) + "\n"))
	return value.Nil{}, err
}

// nextLine reads one line from stdin (nil) or a file handle. The bool is
// false at the end; the Err value (closed/invalid handle) is returned as
// the first result when non-nil.
func nextLine(ctx value.Context, f value.Value) (line string, more bool, errv value.Value, err error) {
	switch x := f.(type) {
	case value.Nil:
		l, e := ctx.ReadLine()
		if e != nil {
			return "", false, nil, nil
		}
		return trimLine(l), true, nil, nil
	case *value.File:
		x.Mu.Lock()
		defer x.Mu.Unlock()
		if x.Closed {
			return "", false, nat.ErrValue("closed", "file `"+x.Path+"` is closed"), nil
		}
		if x.Reader == nil {
			return "", false, nat.ErrValue("invalid", "file `"+x.Path+"` is not open for reading"), nil
		}
		l, e := x.Reader.ReadString('\n')
		if e != nil && l == "" {
			return "", false, nil, nil
		}
		return trimLine(l), true, nil, nil
	}
	return "", false, nil, diag.Errorf("E0403", diag.Pos{}, "f", "file | nil", f.TypeName())
}

func trimLine(line string) string {
	return strings.TrimSuffix(strings.TrimSuffix(line, "\n"), "\r")
}

// readLineFn reads one line: Ok(line), Ok(nil) at the end.
func readLineFn(ctx value.Context, args []value.Value) (value.Value, error) {
	line, more, errv, err := nextLine(ctx, args[0])
	if err != nil {
		return nil, err
	}
	if errv != nil {
		return errv, nil
	}
	if !more {
		return nat.Ok(value.Nil{}), nil
	}
	return nat.Ok(value.Str(line)), nil
}

// readLinesFn reads every remaining line into a list.
func readLinesFn(ctx value.Context, args []value.Value) (value.Value, error) {
	var lines []value.Value
	for {
		line, more, errv, err := nextLine(ctx, args[0])
		if err != nil {
			return nil, err
		}
		if errv != nil {
			return errv, nil
		}
		if !more {
			break
		}
		lines = append(lines, value.Str(line))
	}
	return nat.Ok(value.NewList(lines)), nil
}
