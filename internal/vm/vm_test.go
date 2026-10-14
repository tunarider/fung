package vm

import (
	"bytes"
	"github.com/tunarider/fung/extras"
	"testing"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/frontend"
	"github.com/tunarider/fung/internal/prelude"
	"github.com/tunarider/fung/internal/value"
)

// compileWith runs the front end with the given prelude table.
func compileWith(t *testing.T, src string, prelude map[string]value.Value) *bytecode.Proto {
	t.Helper()
	return buildWith(t, src, prelude).Proto
}

// testRegistry is the full library the command assembles: the built-in
// extras, their bridges, and the VM's concurrency bridge.
func testRegistry() frontend.Registry {
	r := extras.Default()
	for k, v := range NativeModules() {
		r.Natives[k] = v
	}
	return r
}

// buildWith compiles with the built-in extras available; registerModules
// makes a Runtime able to import what the build loaded.
func buildWith(t *testing.T, src string, prelude map[string]value.Value) frontend.Build {
	t.Helper()
	r := testRegistry()
	l := &frontend.Loader{Prelude: prelude, Natives: r.Natives, Stdlib: r.Stdlib}
	b, errs := l.Compile("t.fung", src, frontend.StageAll)
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	return b
}

func registerModules(rt *Runtime, b frontend.Build) {
	for _, m := range b.Program.Modules {
		if m.Native != nil {
			rt.RegisterNative(m.Key, m.Native)
		} else {
			rt.Register(m.Key, m.Proto)
		}
	}
}

// bootTestPrelude boots the whole prelude on rt the way cmd/fung does:
// the Go core and concurrency natives, plus the public names of the
// embedded prelude.fung (06). Tests of fung-level names go through run.
func bootTestPrelude(t *testing.T, rt *Runtime) map[string]value.Value {
	t.Helper()
	p := concPrelude()
	src, ok := extras.Source("prelude")
	if !ok {
		t.Fatal("no embedded prelude.fung")
	}
	r := testRegistry()
	l := &frontend.Loader{Prelude: p, Natives: r.Natives, Stdlib: r.Stdlib, Internal: true}
	prog, errs := l.Load("extras/prelude.fung", src)
	if len(errs) > 0 {
		t.Fatal(errs)
	}
	for _, m := range prog.Modules {
		if m.Native != nil {
			rt.RegisterNative(m.Key, m.Native)
		} else {
			rt.Register(m.Key, m.Proto)
		}
	}
	rt.Register("prelude", prog.Entry)
	m, err := rt.ImportModule("prelude")
	if err != nil {
		t.Fatal(err)
	}
	for _, name := range m.Names() {
		v, _ := m.Get(name)
		p[name] = v
	}
	return p
}

// run executes a source with the full prelude (Go natives and
// prelude.fung) and returns the result value, what print wrote, and the
// error code ("" on success).
func run(t *testing.T, src string) (value.Value, string, string) {
	t.Helper()
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	b := buildWith(t, src, bootTestPrelude(t, rt))
	registerModules(rt, b)
	v, rerr := rt.Run(b.Proto)
	rt.Wait()
	code := ""
	if rerr != nil {
		code = rerr.Code
	}
	return v, out.String(), code
}

func TestEvaluatesExpressions(t *testing.T) {
	cases := map[string]value.Value{
		"1 + 2 * 3":      value.Int(7),
		"(1 + 2) * 3":    value.Int(9),
		"10 / 4":         value.Float(2.5),
		"7 // 2":         value.Int(3),
		"-7 % 3":         value.Int(2),
		"2 ** 10":        value.Int(1024),
		"-2 ** 2":        value.Int(-4),
		"1 == 1.0":       value.Bool(true),
		"1 != true":      value.Bool(true),
		"2 >= 3":         value.Bool(false),
		"not true":       value.Bool(false),
		"true and false": value.Bool(false),
		"false or true":  value.Bool(true),
		"1\n2":           value.Int(2),
		"":               value.Nil{},
	}
	for src, want := range cases {
		v, _, code := run(t, src)
		if code != "" || v != want {
			t.Errorf("%q = %v (%s), want %v", src, v, code, want)
		}
	}
}

func TestShortCircuitSkipsRightSide(t *testing.T) {
	// 1 / 0 would be E0502 if evaluated.
	if _, _, code := run(t, "false and (1 / 0 == 1)"); code != "" {
		t.Errorf("and did not short-circuit: %s", code)
	}
	if _, _, code := run(t, "true or (1 / 0 == 1)"); code != "" {
		t.Errorf("or did not short-circuit: %s", code)
	}
}

func TestPrintWritesToRuntimeStdout(t *testing.T) {
	_, out, code := run(t, "print(1 + 2 * 3)\nprint(1, 2.0, true)\nprint()")
	if code != "" || out != "7\n1 2.0 true\n\n" {
		t.Fatalf("out = %q, code = %s", out, code)
	}
}

func TestRuntimeErrorsCarryCodeAndPosition(t *testing.T) {
	cases := map[string]string{
		"1 / 0":                   "E0502",
		"9223372036854775807 + 1": "E0503",
		"1 + true":                "E0501",
		"1e308 * 10":              "E0504",
		"not 1":                   "E0505",
		"1 and true":              "E0505",
		"true and 1":              "E0505",
		"false or 1":              "E0505",
		"1(2)":                    "E0401",
		"-true":                   "E0506",
	}
	for src, want := range cases {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
	p := compileWith(t, "print(1 +\n 2 / 0)", prelude.Test())
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Pos.Line != 2 || err.Pos.Col != 4 {
		t.Fatalf("err = %v, want position of `/` (2:4)", err)
	}
}

func TestGoPanicBecomesInternalError(t *testing.T) {
	boom := &value.Native{Name: "boom", Fn: func(value.Context, []value.Value) (value.Value, error) { panic("x") }}
	p := compileWith(t, "boom()", map[string]value.Value{"boom": boom})
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Code != "E0801" {
		t.Fatalf("err = %v, want E0801", err)
	}
}

func TestVariablesBlocksAndLiterals(t *testing.T) {
	cases := map[string]string{
		"x = 1\ny = x + 1\nx = x * 10\nprint(x, y)":             "10 2\n",
		"if true: z = 5; print(z) ;;":                           "5\n",
		"w = if 7 > 5: \"big\" else: \"small\" ;;\nprint(w)":    "big\n",
		"print(if false: 1 ;;)":                                 "nil\n",
		"print(1 > 0 then \"a\" else \"b\")":                    "a\n",
		"x = 1\nprint(\"x is ${x}, next ${x + 1}\")":            "x is 1, next 2\n",
		"print(\"a\" + \"b\", \"a\" == \"a\", \"a\" < \"b\")":   "ab true true\n",
		"print('ok, 'ok == 'ok, 'ok == \"ok\")":                 "'ok true false\n",
		"print(1h30m, 500ms + 1.5s, 2 * 30s, 1m / 4, 30s < 1m)": "1h30m 2s 1m 15s true\n",
		"print(nil, nil == nil, nil == 0)":                      "nil true false\n",
		"print(-9223372036854775808)":                           "-9223372036854775808\n",
		"print(9007199254740993 == 9007199254740992.0)":         "false\n",
		"print(\"${\"q\"}\", \"${'s}\", \"${1.0}\")":            "q 's 1.0\n",
		"x = 1\nx = 2\nprint(x)":                                "2\n",
	}
	for src, want := range cases {
		_, out, code := run(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestBlockValueAndStatementValue(t *testing.T) {
	v, _, code := run(t, "x = 1")
	if code != "" || v != (value.Nil{}) {
		t.Fatalf("program ending in assignment = %v (%s), want nil", v, code)
	}
	v, _, _ = run(t, "if true: a = 1; a + 1 ;;")
	if v != value.Int(2) {
		t.Fatalf("block value = %v", v)
	}
}

func TestFailRaisesE0803WithPosition(t *testing.T) {
	_, out, code := run(t, "print(1)\nfail(\"boom\", 2)")
	if code != "E0803" || out != "1\n" {
		t.Fatalf("code=%s out=%q", code, out)
	}
	p := compileWith(t, "x = 1\nfail(\"boom\")", prelude.Test())
	_, err := (&Runtime{Stdout: &bytes.Buffer{}}).Run(p)
	if err == nil || err.Error() != "error[E0803]: boom" || err.Pos.Line != 2 || err.Pos.Col != 5 {
		t.Fatalf("err = %v at %v", err, err.Pos)
	}
}

func TestRuntimeErrorsForNewTypes(t *testing.T) {
	cases := map[string]string{
		"nil + 1":         "E0501",
		"\"a\" + 1":       "E0501",
		"'a < 'b":         "E0501",
		"1s * 1s":         "E0501",
		"1s / 0":          "E0502",
		"if 1: 2 ;;":      "E0505",
		"1 then 2 else 3": "E0505",
	}
	for src, want := range cases {
		if _, _, code := run(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}

func TestGlobalsPersistAcrossRunsOnOneRuntime(t *testing.T) {
	// The REPL (M6) runs one program per line on a single Runtime, and
	// tasks (M7) share the module's globals: they must live in Runtime.
	rt := &Runtime{Stdout: &bytes.Buffer{}}
	first := compileWith(t, "x = 41", prelude.Test())
	if _, err := rt.Run(first); err != nil {
		t.Fatal(err)
	}
	second := &bytecode.Proto{Name: "<main>", GlobalNames: []string{"x"}}
	pos := diag.Pos{File: "t.fung", Line: 1, Col: 1}
	second.Emit(bytecode.OpLoadGlobal, 0, pos)
	second.Emit(bytecode.OpConst, second.Const(value.Int(1)), pos)
	second.Emit(bytecode.OpAdd, 0, pos)
	second.Emit(bytecode.OpReturn, 0, pos)
	v, err := rt.Run(second)
	if err != nil || v != value.Int(42) {
		t.Fatalf("v = %v, err = %v; want 42", v, err)
	}
}
