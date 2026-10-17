package main

import (
	"bytes"
	"github.com/tunarider/fung/extras"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// invoke runs the CLI entry point in-process and returns its streams and exit code.
func invoke(t *testing.T, args ...string) (stdout, stderr string, code int) {
	t.Helper()
	var out, errOut bytes.Buffer
	code = run(args, strings.NewReader(""), &out, &errOut)
	return out.String(), errOut.String(), code
}

func TestVersionPrintsNameAndVersion(t *testing.T) {
	stdout, stderr, code := invoke(t, "version")
	if code != exitOK {
		t.Fatalf("exit code = %d, want %d (stderr: %q)", code, exitOK, stderr)
	}
	if !strings.HasPrefix(stdout, "fung ") {
		t.Fatalf("stdout = %q, want prefix \"fung \"", stdout)
	}
	if stderr != "" {
		t.Fatalf("stderr = %q, want empty", stderr)
	}
}

func TestNoArgsStartsTheRepl(t *testing.T) {
	var out, errOut bytes.Buffer
	code := run(nil, strings.NewReader("1 + 1\nuntil(1, x => x * 2, x => x > 5)\n"), &out, &errOut)
	if code != 0 || errOut.String() != "" || out.String() != "> 2\n> 8\n> \n" {
		t.Fatalf("code=%d out=%q err=%q", code, out.String(), errOut.String())
	}
}

func TestUnknownSubcommandIsUsageError(t *testing.T) {
	_, stderr, code := invoke(t, "frobnicate")
	if code != exitUsage {
		t.Fatalf("exit code = %d, want %d", code, exitUsage)
	}
	if !strings.Contains(stderr, "frobnicate") {
		t.Fatalf("stderr = %q, want it to name the unknown command", stderr)
	}
	if !strings.Contains(stderr, "usage") {
		t.Fatalf("stderr = %q, want usage text", stderr)
	}
}

func TestHelpPrintsUsageAndSucceeds(t *testing.T) {
	for _, arg := range []string{"help", "-h", "--help"} {
		stdout, _, code := invoke(t, arg)
		if code != exitOK {
			t.Errorf("%s: exit code = %d, want %d", arg, code, exitOK)
		}
		if !strings.Contains(stdout, "usage") {
			t.Errorf("%s: stdout = %q, want usage text", arg, stdout)
		}
	}
}

func TestDumpRejectsUnknownStage(t *testing.T) {
	_, stderr, code := invoke(t, "dump", "tokens", "x.fung")
	if code != exitUsage {
		t.Fatalf("exit code = %d, want %d", code, exitUsage)
	}
	if !strings.Contains(stderr, "tokens") {
		t.Fatalf("stderr = %q, want it to name the bad stage", stderr)
	}
}

func TestExitCodesAreDistinct(t *testing.T) {
	codes := map[int]string{}
	for name, c := range map[string]int{
		"ok": exitOK, "runtime": exitRuntimeError, "compile": exitCompileError, "usage": exitUsage,
	} {
		if prev, dup := codes[c]; dup {
			t.Fatalf("%s and %s share exit code %d", prev, name, c)
		}
		codes[c] = name
	}
	if exitOK != 0 {
		t.Fatalf("exitOK = %d, want 0", exitOK)
	}
}

func TestScriptPathDetection(t *testing.T) {
	cases := map[string]bool{
		"x.fung":        true,  // extension
		"./tool":        true,  // relative path (shebang without extension)
		"/usr/bin/tool": true,  // absolute path
		"frobnicate":    false, // bare word: a subcommand name, possibly misspelled
		"-x":            false, // flag
		"":              false,
	}
	for arg, want := range cases {
		if got := isScriptPath(arg); got != want {
			t.Errorf("isScriptPath(%q) = %v, want %v", arg, got, want)
		}
	}
}

func TestEvalPrintsResultOfPrint(t *testing.T) {
	stdout, stderr, code := invoke(t, "-e", "print(1 + 2 * 3)")
	if code != exitOK || stdout != "7\n" || stderr != "" {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout, stderr)
	}
}

func TestEvalCompileErrorExitsTwo(t *testing.T) {
	_, stderr, code := invoke(t, "-e", "print(fiels)")
	if code != exitCompileError || !strings.Contains(stderr, "error[E0204]: undefined name `fiels`") || !strings.Contains(stderr, "--> -e:1:7") {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestEvalReportsAllResolveErrors(t *testing.T) {
	_, stderr, code := invoke(t, "-e", "print(a + b)")
	if code != exitCompileError || strings.Count(stderr, "error[E0204]") != 2 {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestEvalRuntimeErrorExitsOne(t *testing.T) {
	stdout, stderr, code := invoke(t, "-e", "print(1)\nprint(1 / 0)")
	if code != exitRuntimeError || stdout != "1\n" || !strings.Contains(stderr, "error[E0502]: division by zero") {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout, stderr)
	}
}

func TestRunMissingFileIsUsageError(t *testing.T) {
	_, stderr, code := invoke(t, "run", "/nonexistent/x.fung")
	if code != exitUsage || !strings.Contains(stderr, "x.fung") {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestDumpStages(t *testing.T) {
	stdout, _, code := invoke(t, "dump", "ast", "-e", "1 + 2 * 3")
	if code != exitOK || stdout != "(program (binary + 1 (binary * 2 3)))\n" {
		t.Fatalf("ast: code=%d stdout=%q", code, stdout)
	}
	stdout, _, code = invoke(t, "dump", "core", "-e", "print(1 and 2)")
	if code != exitOK || stdout != "(do (call (prelude print) (if 1 (op bool 2) false)))\n" {
		t.Fatalf("core: code=%d stdout=%q", code, stdout)
	}
	stdout, _, code = invoke(t, "dump", "bytecode", "-e", "1 + 2")
	if code != exitOK || !strings.HasPrefix(stdout, "== <main> ==\n0000  1:1   CONST        0    ; 1\n") {
		t.Fatalf("bytecode: code=%d stdout=%q", code, stdout)
	}
}

func TestDumpNeedsSource(t *testing.T) {
	_, _, code := invoke(t, "dump", "ast")
	if code != exitUsage {
		t.Fatalf("code=%d", code)
	}
}

func TestEvalErrorShowsSourceExcerpt(t *testing.T) {
	_, stderr, code := invoke(t, "-e", "x = 1\nprint(fiels)")
	want := "error[E0204]: undefined name `fiels`\n" +
		"  --> -e:2:7\n" +
		"  |\n" +
		"2 | print(fiels)\n" +
		"  |       ^^^^^ not found in this scope\n" +
		"  |\n"
	if code != exitCompileError || stderr != want {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestRuntimeErrorShowsExcerptFromFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "a.fung")
	os.WriteFile(path, []byte("x = 2\nprint(x / 0)\n"), 0o644)
	stdout, stderr, code := invoke(t, path)
	if code != exitRuntimeError || stdout != "" || !strings.Contains(stderr, "2 | print(x / 0)\n  |         ^\n") {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout, stderr)
	}
}

func TestDumpAstWithEButNoCodeIsUsageError(t *testing.T) {
	_, stderr, code := invoke(t, "dump", "ast", "-e")
	if code != exitUsage || !strings.Contains(stderr, "-e needs code") {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestRuntimeErrorFromVMAlwaysExitsOne(t *testing.T) {
	_, stderr, code := invoke(t, "-e", "fail(\"x\")")
	if code != exitRuntimeError || !strings.Contains(stderr, "error[E0803]") {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

func TestCallPathIsPrinted(t *testing.T) {
	_, stderr, code := invoke(t, "-e", "fn f(x) = x / 0\nfn g() = f(1) + 0\ng()")
	want := " call path (innermost first):\n   -e:1       f\n   -e:2       g\n   -e:3       <entry>\n"
	if code != exitRuntimeError || !strings.Contains(stderr, want) {
		t.Fatalf("code=%d stderr=%q", code, stderr)
	}
}

// An error raised inside an embedded extras file points at the user's
// call; the extras frames stay in the call path (06).
func TestErrorsInsideExtrasPointAtTheCaller(t *testing.T) {
	for _, src := range []string{"first(\"abc\")", "pad_left(\"a\", 3, \"\")", "fn f(x) = take(x, 1)\nf(1)"} {
		_, stderr, code := invoke(t, "-e", src)
		if code != exitRuntimeError || !strings.Contains(stderr, "  --> -e:") || !strings.Contains(stderr, "extras/prelude.fung") {
			t.Errorf("%q: code=%d stderr=%q", src, code, stderr)
		}
	}
}

func TestAtExitRunsAndFailuresSetExitCode(t *testing.T) {
	stdout, stderr, code := invoke(t, "-e", "at_exit(() => print(\"bye\"))\nprint(\"hi\")")
	if code != exitOK || stdout != "hi\nbye\n" || stderr != "" {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout, stderr)
	}
	stdout, stderr, code = invoke(t, "-e", "at_exit(() => print(\"cleanup\"))\nat_exit(() => fail(\"boom\"))\nprint(1 / 0)")
	if code != exitRuntimeError || stdout != "cleanup\n" || strings.Count(stderr, "error[") != 2 {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout, stderr)
	}
}

func TestScriptArgsAndExit(t *testing.T) {
	out, errOut, code := invoke(t, "-e", "print(args)\nat_exit(() => print(\"bye\"))\nexit(3)\nprint(\"no\")", "x", "y")
	if code != 3 || out != "[\"x\", \"y\"]\nbye\n" || errOut != "" {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
	_, _, code = invoke(t, "-e", "exit()")
	if code != 0 {
		t.Fatalf("exit() code = %d", code)
	}
	_, errOut, code = invoke(t, "-e", "at_exit(() => fail(\"boom\"))\nexit(0)")
	if code != 1 || !strings.Contains(errOut, "E0803") {
		t.Fatalf("failing handler after exit(0): code=%d err=%q", code, errOut)
	}
	_, _, code = invoke(t, "-e", "at_exit(() => exit(5))\nprint(1)")
	if code != 5 {
		t.Fatalf("exit inside a handler: code=%d", code)
	}
}

func TestDumpKnowsTheFungPrelude(t *testing.T) {
	out, errOut, code := invoke(t, "dump", "core", "-e", "print(until(1, x => x, x => true))")
	if code != 0 || !strings.Contains(out, "(prelude until)") {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
	_, errOut, code = invoke(t, "-e", "import prelude")
	if code != 2 || !strings.Contains(errOut, "E0301") {
		t.Fatalf("import prelude: code=%d err=%q", code, errOut)
	}
}

func TestStdinFlowsToReadLine(t *testing.T) {
	var out, errOut bytes.Buffer
	code := run([]string{"-e", "print(read_line())"}, strings.NewReader("hi\n"), &out, &errOut)
	if code != 0 || out.String() != "Ok(\"hi\")\n" {
		t.Fatalf("code=%d out=%q", code, out.String())
	}
}

func TestErrorsInImportedFilesShowTheirSource(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "lib.fung"), []byte("fn f() = 1 +)\n"), 0o644)
	os.WriteFile(filepath.Join(dir, "main.fung"), []byte("import \"./lib\"\n"), 0o644)
	_, errOut, code := invoke(t, filepath.Join(dir, "main.fung"))
	if code != 2 || !strings.Contains(errOut, "lib.fung:1") || !strings.Contains(errOut, "fn f() = 1 +)") {
		t.Fatalf("code=%d err=%q", code, errOut)
	}
	os.WriteFile(filepath.Join(dir, "lib.fung"), []byte("x = 1 / 0\n"), 0o644)
	os.WriteFile(filepath.Join(dir, "main.fung"), []byte("import \"./lib\"\n"), 0o644)
	_, errOut, code = invoke(t, filepath.Join(dir, "main.fung"))
	if code != 1 || !strings.Contains(errOut, "E0502") || !strings.Contains(errOut, "<module lib>") {
		t.Fatalf("runtime error in a module: code=%d err=%q", code, errOut)
	}
}

func TestCheckCommand(t *testing.T) {
	out, errOut, code := invoke(t, "check", "-e", "print(1)")
	if code != 0 || out != "" || errOut != "" {
		t.Fatalf("clean: code=%d out=%q err=%q", code, out, errOut)
	}
	_, errOut, code = invoke(t, "check", "-e", "fn f():\n  x = 1\n  2\n;;")
	if code != 0 || !strings.Contains(errOut, "warning[W0102]: unused variable `x`") {
		t.Fatalf("warning: code=%d err=%q", code, errOut)
	}
	_, errOut, code = invoke(t, "check", "-e", "print(y)")
	if code != 2 || !strings.Contains(errOut, "E0204") {
		t.Fatalf("error: code=%d err=%q", code, errOut)
	}
	out, errOut, code = invoke(t, "-e", "fn f():\n  x = 1\n  2\n;;\nprint(f())")
	if code != 0 || errOut != "" || out != "2\n" {
		t.Fatalf("run never warns: code=%d out=%q err=%q", code, out, errOut)
	}
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "a.fung"), []byte("import \"./b\"\n"), 0o644)
	os.WriteFile(filepath.Join(dir, "b.fung"), []byte("print(1)\n"), 0o644)
	_, errOut, code = invoke(t, "check", filepath.Join(dir, "a.fung"))
	if code != 2 || !strings.Contains(errOut, "E0306") {
		t.Fatalf("check follows imports: code=%d err=%q", code, errOut)
	}
	_, _, code = invoke(t, "check")
	if code != 3 {
		t.Fatalf("check without a target is a usage error: %d", code)
	}
}

func TestTestCommand(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "math.fung"), []byte("fn double(x) = x * 2\n"), 0o644)
	os.WriteFile(filepath.Join(dir, "math_test.fung"), []byte(
		"import \"./math\"\ntest \"doubles\": assert math.double(2) == 4 ;;\ntest \"fails\": assert math.double(2) == 5 ;;\ntest \"raises\": fail(\"boom\") ;;\n"), 0o644)
	os.WriteFile(filepath.Join(dir, "notests.fung"), []byte("fail(\"a script without tests must not run\")\n"), 0o644)
	os.MkdirAll(filepath.Join(dir, "_skip"), 0o755)
	os.WriteFile(filepath.Join(dir, "_skip", "s.fung"), []byte("test \"never\": assert false ;;\n"), 0o644)
	out, errOut, code := invoke(t, "test", dir)
	if code != 1 {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
	rel := func(name string) string { return filepath.Join(dir, name) }
	for _, want := range []string{
		"ok   " + rel("math_test.fung") + ": doubles\n",
		"FAIL " + rel("math_test.fung") + ": fails\n",
		"assertion failed: math.double(2) == 5",
		"math.double(2) = 4",
		"FAIL " + rel("math_test.fung") + ": raises\n",
		"E0803",
		"1 passed, 2 failed\n",
	} {
		if !strings.Contains(out, want) {
			t.Errorf("missing %q in %q", want, out)
		}
	}
	if strings.Contains(out, "notests") || strings.Contains(out, "never") {
		t.Errorf("files without tests and _dirs are skipped: %q", out)
	}
	os.WriteFile(filepath.Join(dir, "ok_test.fung"), []byte("test \"a\": assert 1 == 1 ;;\n"), 0o644)
	out, _, code = invoke(t, "test", filepath.Join(dir, "ok_test.fung"))
	if code != 0 || !strings.Contains(out, "1 passed, 0 failed") {
		t.Fatalf("single passing file: code=%d out=%q", code, out)
	}
	os.WriteFile(filepath.Join(dir, "bad_test.fung"), []byte("test \"a\": assert ( ;;\n"), 0o644)
	_, errOut, code = invoke(t, "test", filepath.Join(dir, "bad_test.fung"))
	if code != 2 || !strings.Contains(errOut, "E0110") {
		t.Fatalf("compile error: code=%d err=%q", code, errOut)
	}
	os.WriteFile(filepath.Join(dir, "exit_test.fung"), []byte("test \"a\": exit(0) ;;\ntest \"b\": assert true ;;\n"), 0o644)
	out, _, code = invoke(t, "test", filepath.Join(dir, "exit_test.fung"))
	if code != 1 || strings.Contains(out, ": b") || !strings.Contains(out, "FAIL "+rel("exit_test.fung")+": a\n") || !strings.Contains(out, "exit(0) called inside a test") || !strings.Contains(out, "0 passed, 1 failed") {
		t.Fatalf("exit inside a test is a reported failure that stops the file: code=%d out=%q", code, out)
	}
	os.WriteFile(filepath.Join(dir, "exit_top_test.fung"), []byte("exit(3)\ntest \"b\": assert true ;;\n"), 0o644)
	out, _, code = invoke(t, "test", filepath.Join(dir, "exit_top_test.fung"))
	if code != 1 || !strings.Contains(out, "FAIL "+rel("exit_top_test.fung")+": <top level>") || !strings.Contains(out, "exit(3) called") {
		t.Fatalf("exit at the top level of a test file: code=%d out=%q", code, out)
	}
	os.WriteFile(filepath.Join(dir, "warn_test.fung"), []byte("fn f():\n  u = 1\n  2\n;;\ntest \"a\": assert f() == 2 ;;\n"), 0o644)
	_, errOut, code = invoke(t, "test", filepath.Join(dir, "warn_test.fung"))
	if code != 0 || !strings.Contains(errOut, "W0102") {
		t.Fatalf("test shows warnings: code=%d err=%q", code, errOut)
	}
}

func TestExamplesPassTheirTests(t *testing.T) {
	out, errOut, code := invoke(t, "test", filepath.Join("..", "..", "examples"))
	if code != 0 || !strings.HasSuffix(out, "12 passed, 0 failed\n") || errOut != "" {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
}

func TestStdlibCliTests(t *testing.T) {
	src, ok := extras.Source("cli")
	if !ok {
		t.Fatal("no cli.fung")
	}
	path := filepath.Join(t.TempDir(), "cli.fung")
	os.WriteFile(path, []byte(src), 0o644)
	out, errOut, code := invoke(t, "test", path)
	if code != 0 || !strings.Contains(out, "1 passed, 0 failed") || errOut != "" {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
}

func TestConcurrencyExitRules(t *testing.T) {
	out, errOut, code := invoke(t, "-e", "t = spawn(() => 1)\nprint(await(t))")
	if code != 0 || out != "1\n" || errOut != "" {
		t.Fatalf("basic: code=%d out=%q err=%q", code, out, errOut)
	}
	out, errOut, code = invoke(t, "-e", "spawn(() => fail(\"x\"))\nsleep(20ms)\nprint(\"done\")")
	if code != 1 || out != "done\n" || !strings.Contains(errOut, "task failed: error[E0803]: x") {
		t.Fatalf("unawaited failure: code=%d out=%q err=%q", code, out, errOut)
	}
	_, _, code = invoke(t, "-e", "spawn(() => fail(\"x\"))\nsleep(20ms)\nexit(0)")
	if code != 0 {
		t.Fatalf("exit(0) wins: %d", code)
	}
	out, _, code = invoke(t, "-e", "at_exit(() => print(\"bye\"))\nspawn(fn(): sleep(10ms); exit(3) ;;)\nsleep(10s)\nprint(\"never\")")
	if code != 3 || out != "bye\n" {
		t.Fatalf("exit from a task: code=%d out=%q", code, out)
	}
	out, _, code = invoke(t, "-e", "spawn(fn(): sleep(20ms); print(\"late\") ;;)\nprint(\"entry done\")")
	if code != 0 || out != "entry done\nlate\n" {
		t.Fatalf("waits for tasks: code=%d out=%q", code, out)
	}
	_, errOut, code = invoke(t, "-e", "spawn(() => sleep(10s))\n1 / 0")
	if code != 1 || !strings.Contains(errOut, "E0502") {
		t.Fatalf("entry error cancels tasks and exits 1: code=%d err=%q", code, errOut)
	}
	_, errOut, code = invoke(t, "-e", "ch = channel()\nrecv(ch)")
	if code != 1 || !strings.Contains(errOut, "deadlock") {
		t.Fatalf("deadlock: code=%d err=%q", code, errOut)
	}
	out, _, code = invoke(t, "-e", "a = atom(0)\neach(for _ in 0..4: spawn(() => each(0..10, _ => swap(a, n => n + 1))) ;;, await)\nprint(deref(a))")
	if code != 0 || out != "40\n" {
		t.Fatalf("atoms through the command: code=%d out=%q", code, out)
	}
}

func TestTestCommandCountsUnawaitedFailures(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "bg_test.fung"), []byte("test \"spawns a failing task\": spawn(() => fail(\"bg\")); sleep(20ms) ;;\n"), 0o644)
	out, errOut, code := invoke(t, "test", filepath.Join(dir, "bg_test.fung"))
	if code != 1 || !strings.Contains(out, "FAIL "+filepath.Join(dir, "bg_test.fung")+": <tasks>") || !strings.Contains(errOut, "task failed") {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
}

func TestExitHandlersAfterDeadlockAndExitInsideHandlers(t *testing.T) {
	out, errOut, code := invoke(t, "-e", "at_exit(fn(): sleep(1ms); print(\"cleanup ran\") ;;)\nrecv(channel())")
	if code != 1 || out != "cleanup ran\n" || strings.Count(errOut, "E0702") != 1 {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
	out, _, code = invoke(t, "-e", "at_exit(fn(): t = spawn(() => exit(7)); await(t); print(\"after\") ;;)\nprint(\"main\")")
	if code != 7 || strings.Contains(out, "after") {
		t.Fatalf("exit from a task inside a handler: code=%d out=%q", code, out)
	}
	out, errOut, code = invoke(t, "-e", "r = pmap([1, 2, 3], x => fail(\"boom\"))\nprint(r |> map(is_err))")
	if code != 0 || out != "[true, true, true]\n" || errOut != "" {
		t.Fatalf("pmap handles its tasks' failures itself: code=%d out=%q err=%q", code, out, errOut)
	}
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "worker_test.fung"), []byte("fn spin(n) = n == 0 then 0 else spin(n - 1)\ntest \"waits for a worker\":\n  t = spawn(() => spin(400000))\n  assert await(t) == 0\n;;\n"), 0o644)
	out, errOut, code = invoke(t, "test", filepath.Join(dir, "worker_test.fung"))
	if code != 0 || !strings.Contains(out, "1 passed, 0 failed") {
		t.Fatalf("test body awaiting a computing worker: code=%d out=%q err=%q", code, out, errOut)
	}
}

// TestTestCommandReportsBrokenLibraryOnce: a library that fails to
// compile is reported once, and every file that could not load counts
// as a failure in the summary (open-questions, 2026-10-14).
func TestTestCommandReportsBrokenLibraryOnce(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "lib.fung"), []byte("fn broken() = 1 +\n"), 0o644)
	os.WriteFile(filepath.Join(dir, "a_test.fung"), []byte("import \"./lib\"\ntest \"a\": assert true ;;\n"), 0o644)
	os.WriteFile(filepath.Join(dir, "b_test.fung"), []byte("import \"./lib\"\ntest \"b\": assert true ;;\n"), 0o644)
	out, errOut, code := invoke(t, "test", dir)
	if code != exitCompileError {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
	if n := strings.Count(errOut, "error[E0110]"); n != 1 {
		t.Errorf("the library error must be printed once, got %d:\n%s", n, errOut)
	}
	for _, f := range []string{"a_test.fung", "b_test.fung", "lib.fung"} {
		if !strings.Contains(out, "FAIL "+filepath.Join(dir, f)+": <load>\n") {
			t.Errorf("missing FAIL line for %s:\n%s", f, out)
		}
	}
	if !strings.HasSuffix(out, "0 passed, 3 failed\n") {
		t.Errorf("summary must count the files that could not load:\n%s", out)
	}
}

func TestFmtCommand(t *testing.T) {
	dir := t.TempDir()
	messy := filepath.Join(dir, "messy.fung")
	clean := filepath.Join(dir, "clean.fung")
	broken := filepath.Join(dir, "broken.fung")
	os.WriteFile(messy, []byte("fn f(x):\nx+1\n;;\n"), 0o644)
	os.WriteFile(clean, []byte("x = 1\n"), 0o644)
	os.WriteFile(broken, []byte("f(\n"), 0o644)
	os.MkdirAll(filepath.Join(dir, "_fixtures"), 0o755)
	os.WriteFile(filepath.Join(dir, "_fixtures", "skip.fung"), []byte("y=2\n"), 0o644)

	out, errOut, code := invoke(t, "fmt", "--check", dir)
	if code != exitCompileError || out != messy+"\n" || !strings.Contains(errOut, "error[E0110]") {
		t.Fatalf("check: code=%d out=%q err=%q", code, out, errOut)
	}
	out, _, code = invoke(t, "fmt", "--check", clean)
	if code != exitOK || out != "" {
		t.Fatalf("check clean: code=%d out=%q", code, out)
	}
	out, _, code = invoke(t, "fmt", messy, clean)
	if code != exitOK || out != messy+"\n" {
		t.Fatalf("fmt: code=%d out=%q", code, out)
	}
	if b, _ := os.ReadFile(messy); string(b) != "fn f(x):\n  x + 1\n;;\n" {
		t.Errorf("messy not formatted: %q", b)
	}
	if b, _ := os.ReadFile(filepath.Join(dir, "_fixtures", "skip.fung")); string(b) != "y=2\n" {
		t.Error("_ directories must be skipped")
	}
	if b, _ := os.ReadFile(broken); string(b) != "f(\n" {
		t.Error("a broken file must be left alone")
	}
	var outBuf, errBuf bytes.Buffer
	code = run([]string{"fmt", "-"}, strings.NewReader("a=1\n"), &outBuf, &errBuf)
	if code != exitOK || outBuf.String() != "a = 1\n" {
		t.Errorf("stdin: code=%d out=%q err=%q", code, outBuf.String(), errBuf.String())
	}
	// --check on standard input names <stdin> and writes no code.
	for _, args := range [][]string{{"fmt", "--check", "-"}, {"fmt", "-", "--check"}} {
		outBuf.Reset()
		errBuf.Reset()
		code = run(args, strings.NewReader("a=1\n"), &outBuf, &errBuf)
		if code != exitRuntimeError || outBuf.String() != "<stdin>\n" || errBuf.String() != "" {
			t.Errorf("%v messy: code=%d out=%q err=%q", args, code, outBuf.String(), errBuf.String())
		}
		outBuf.Reset()
		errBuf.Reset()
		code = run(args, strings.NewReader("a = 1\n"), &outBuf, &errBuf)
		if code != exitOK || outBuf.String() != "" || errBuf.String() != "" {
			t.Errorf("%v clean: code=%d out=%q err=%q", args, code, outBuf.String(), errBuf.String())
		}
		outBuf.Reset()
		errBuf.Reset()
		code = run(args, strings.NewReader("f(\n"), &outBuf, &errBuf)
		if code != exitCompileError || outBuf.String() != "" || !strings.Contains(errBuf.String(), "error[E0110]") {
			t.Errorf("%v broken: code=%d out=%q err=%q", args, code, outBuf.String(), errBuf.String())
		}
	}
	_, errOut, code = invoke(t, "fmt")
	if code != exitUsage || !strings.Contains(errOut, "fmt needs") {
		t.Errorf("no args: code=%d err=%q", code, errOut)
	}
}

// A symlink to a directory is walked like the directory itself.
func TestFmtFollowsDirectorySymlink(t *testing.T) {
	dir := t.TempDir()
	real := filepath.Join(dir, "d")
	os.MkdirAll(real, 0o755)
	os.WriteFile(filepath.Join(real, "a.fung"), []byte("x=1\n"), 0o644)
	link := filepath.Join(dir, "dlink")
	if err := os.Symlink(real, link); err != nil {
		t.Skip("no symlinks here")
	}
	out, _, code := invoke(t, "fmt", "--check", link)
	if code != exitRuntimeError || !strings.Contains(out, "a.fung") {
		t.Errorf("symlinked root: code=%d out=%q", code, out)
	}
}
