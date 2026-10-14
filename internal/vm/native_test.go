package vm

import (
	"bytes"
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/prelude"
	"github.com/tunarider/fung/internal/value"
)

// padNative is a native with a default and a rest parameter, to test the
// shared calling convention without depending on the real prelude.
func padNative() *value.Native {
	return &value.Native{
		Name: "pad",
		Params: []value.NativeParam{
			{Name: "s"}, {Name: "n", Default: value.Int(2)}, {Name: "tail", Rest: true},
		},
		Fn: func(_ value.Context, args []value.Value) (value.Value, error) {
			n := int(args[1].(value.Int))
			s := string(args[0].(value.Str))
			return value.Str(s + strings.Repeat("*", n) + value.Format(args[2])), nil
		},
	}
}

func runWith(t *testing.T, src string, extra map[string]value.Value) (string, string) {
	t.Helper()
	prelude := prelude.Test()
	for k, v := range extra {
		prelude[k] = v
	}
	p := compileWith(t, src, prelude)
	var out bytes.Buffer
	_, err := (&Runtime{Stdout: &out}).Run(p)
	code := ""
	if err != nil {
		code = err.Code
	}
	return out.String(), code
}

func TestNativesShareTheCallingConvention(t *testing.T) {
	extra := map[string]value.Value{"pad": padNative()}
	cases := map[string]string{
		`print(pad("a"))`:            "a**[]\n",
		`print(pad("a", 3))`:         "a***[]\n",
		`print(pad("a", n: 1))`:      "a*[]\n",
		`print(pad("a", 1, 'x, 'y))`: "a*['x, 'y]\n",
		`print(pad(n: 0, s: "b"))`:   "b[]\n",
		`print(pad("a", n: 2))`:      "a**[]\n", // same as the default
	}
	for src, want := range cases {
		out, code := runWith(t, src, extra)
		if code != "" || out != want {
			t.Errorf("%s: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	errs := map[string]string{
		`pad()`:                "E0407",
		`pad("a", n: 1, n: 2)`: "E0405",
		`pad("a", m: 1)`:       "E0406",
		`print(1, sep: "")`:    "E0406", // rest params cannot be named
		`len()`:                "E0407",
		`len(1, 2)`:            "E0402",
		`type_of()`:            "E0407",
	}
	for src, want := range errs {
		if _, code := runWith(t, src, extra); code != want {
			t.Errorf("%s: code = %s, want %s", src, code, want)
		}
	}
}

func TestExitUnwindsAsASignal(t *testing.T) {
	p := compileWith(t, "fn f(): print(1); exit(3); print(2) ;;\nf()\nprint(3)", prelude.Test())
	var out bytes.Buffer
	_, err := (&Runtime{Stdout: &out}).Run(p)
	if err == nil {
		t.Fatal("expected the exit signal")
	}
	code, ok := err.ExitCode()
	if !ok || code != 3 || out.String() != "1\n" {
		t.Fatalf("err=%v out=%q", err, out.String())
	}
	if _, _, code := run(t, "exit()"); code != "exit" {
		t.Fatalf("exit() code = %s", code)
	}
	if _, _, code := run(t, "exit(\"x\")"); code != "E0403" {
		t.Fatalf("exit(\"x\") code = %s", code)
	}
}

func TestStdinAndStderr(t *testing.T) {
	p := compileWith(t, "eprint(\"warn\", 1)\nprint(read_line(), read_line(), read_line())", prelude.Test())
	var out, errOut bytes.Buffer
	rt := &Runtime{Stdout: &out, Stderr: &errOut, Stdin: strings.NewReader("a\nb\n")}
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	if out.String() != "Ok(\"a\") Ok(\"b\") Ok(nil)\n" || errOut.String() != "warn 1\n" {
		t.Fatalf("out=%q err=%q", out.String(), errOut.String())
	}
	p = compileWith(t, "print(read_lines())", prelude.Test())
	out.Reset()
	rt = &Runtime{Stdout: &out, Stdin: strings.NewReader("x\ny")}
	rt.Run(p)
	if out.String() != "Ok([\"x\", \"y\"])\n" {
		t.Fatalf("read_lines: %q", out.String())
	}
}
