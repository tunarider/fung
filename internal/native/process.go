package native

import (
	"bytes"
	"errors"
	"fmt"
	"github.com/tunarider/fung/internal/nat"
	"os"
	"os/exec"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// processModule builds _native.process: exec, sh and env (06: 명령 실행).
// prelude.fung re-exports them under their plain names.
func processModule() *value.Module {
	m := value.NewModule("process", "_native.process")
	add := func(n *value.Native) { m.Set(n.Name, n) }
	add(nat.Def("exec", execFn, nat.Param("cmd"), nat.Opt("cwd", value.Nil{}), nat.Opt("env", value.Nil{}), nat.Opt("stdin", value.Nil{})))
	add(nat.Def("sh", shFn, nat.Param("cmd"), nat.Opt("cwd", value.Nil{}), nat.Opt("env", value.Nil{}), nat.Opt("stdin", value.Nil{})))
	add(nat.Def("env", envFn, nat.Opt("name", value.Nil{})))
	return m
}

func runCommand(argv []string, args []value.Value) (value.Value, error) {
	cmd := exec.Command(argv[0], argv[1:]...)
	if cwd, ok, err := nat.OptStr("cwd", args[1]); err != nil {
		return nil, err
	} else if ok {
		cmd.Dir = cwd
	}
	if _, isNil := args[2].(value.Nil); !isNil {
		m, ok := args[2].(*value.Map)
		if !ok {
			return nil, diag.Errorf("E0403", diag.Pos{}, "env", "map | nil", args[2].TypeName())
		}
		cmd.Env = os.Environ()
		for _, k := range m.Keys() {
			v, _ := m.Get(k)
			s, ok := v.(value.Str)
			if !ok {
				return nil, diag.Errorf("E0403", diag.Pos{}, "env", "a map of strings", "a map holding "+v.TypeName())
			}
			cmd.Env = append(cmd.Env, k+"="+string(s))
		}
	}
	if in, ok, err := nat.OptStr("stdin", args[3]); err != nil {
		return nil, err
	} else if ok {
		cmd.Stdin = strings.NewReader(in)
	}
	var out, errOut bytes.Buffer
	cmd.Stdout, cmd.Stderr = &out, &errOut
	runErr := cmd.Run()
	var exitErr *exec.ExitError
	switch {
	case runErr == nil:
		m := value.NewMapBuilder()
		m.Set("out", value.Str(out.String()))
		m.Set("err", value.Str(errOut.String()))
		m.Set("code", value.Int(0))
		return nat.Ok(m.Map()), nil
	case errors.As(runErr, &exitErr):
		code := exitErr.ExitCode()
		return nat.ErrValue("exit", fmt.Sprintf("`%s` exited with code %d", argv[0], code),
			value.Str("code"), value.Int(code), value.Str("out"), value.Str(out.String()), value.Str("err"), value.Str(errOut.String())), nil
	default:
		return nat.ErrValue("not_found", fmt.Sprintf("cannot run `%s`: %s", argv[0], nat.Reason(runErr))), nil
	}
}

func execFn(_ value.Context, args []value.Value) (value.Value, error) {
	argv, err := nat.StrList("cmd", args[0])
	if err != nil {
		return nil, err
	}
	if len(argv) == 0 {
		return nil, diag.Errorf("E0403", diag.Pos{}, "cmd", "a non-empty list of strings", "an empty list")
	}
	return runCommand(argv, args)
}

func shFn(_ value.Context, args []value.Value) (value.Value, error) {
	cmd, err := nat.NeedStr("cmd", args[0])
	if err != nil {
		return nil, err
	}
	return runCommand([]string{"/bin/sh", "-c", cmd}, args)
}

func envFn(_ value.Context, args []value.Value) (value.Value, error) {
	name, ok, err := nat.OptStr("name", args[0])
	if err != nil {
		return nil, err
	}
	if ok {
		if v, found := os.LookupEnv(name); found {
			return value.Str(v), nil
		}
		return value.Nil{}, nil
	}
	m := value.NewMapBuilder()
	for _, kv := range os.Environ() {
		k, v, _ := strings.Cut(kv, "=")
		m.Set(k, value.Str(v))
	}
	return m.Map(), nil
}
