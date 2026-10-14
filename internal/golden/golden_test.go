package golden

import (
	"strings"
	"testing"

	"github.com/tunarider/fung/internal/diag"
)

func TestParseCollectsExpectedOutputLinesInOrder(t *testing.T) {
	src := "print(1)\n# expect: 1\nprint(\"a b\")\n# expect: a b\n"
	c, err := Parse("t.fung", []byte(src))
	if err != nil {
		t.Fatal(err)
	}
	if c.Stdout != "1\na b\n" {
		t.Fatalf("Stdout = %q, want %q", c.Stdout, "1\na b\n")
	}
	if c.ErrorCode != "" {
		t.Fatalf("ErrorCode = %q, want empty", c.ErrorCode)
	}
}

func TestParseKeepsExpectedOutputEmptyWhenNothingExpected(t *testing.T) {
	c, err := Parse("t.fung", []byte("x = 1\n"))
	if err != nil {
		t.Fatal(err)
	}
	if c.Stdout != "" {
		t.Fatalf("Stdout = %q, want empty", c.Stdout)
	}
}

func TestParseBareExpectMeansEmptyLine(t *testing.T) {
	c, err := Parse("t.fung", []byte("print(\"\")\n# expect:\n"))
	if err != nil {
		t.Fatal(err)
	}
	if c.Stdout != "\n" {
		t.Fatalf("Stdout = %q, want %q", c.Stdout, "\n")
	}
}

func TestParseReadsErrorCode(t *testing.T) {
	c, err := Parse("t.fung", []byte("print(fiels)\n# expect-error: E0204\n"))
	if err != nil {
		t.Fatal(err)
	}
	if c.ErrorCode != "E0204" {
		t.Fatalf("ErrorCode = %q, want E0204", c.ErrorCode)
	}
}

func TestParseRejectsTwoErrorCodes(t *testing.T) {
	_, err := Parse("t.fung", []byte("# expect-error: E0204\n# expect-error: E0205\n"))
	if err == nil {
		t.Fatal("want error for two expect-error lines")
	}
}

func TestParseRejectsMalformedErrorCode(t *testing.T) {
	_, err := Parse("t.fung", []byte("# expect-error: oops\n"))
	if err == nil {
		t.Fatal("want error for malformed error code")
	}
}

func TestParseIgnoresOrdinaryComments(t *testing.T) {
	c, err := Parse("t.fung", []byte("# expectations are nice\n# expected: 1\n"))
	if err != nil {
		t.Fatal(err)
	}
	if c.Stdout != "" || c.ErrorCode != "" {
		t.Fatalf("got %+v, want nothing expected", c)
	}
}

func TestCheckPassesOnExactMatch(t *testing.T) {
	c := Case{Stdout: "1\n"}
	if err := Check(c, Result{Stdout: "1\n", ExitCode: diag.ExitOK}); err != nil {
		t.Fatal(err)
	}
}

func TestCheckFailsOnDifferentStdout(t *testing.T) {
	c := Case{Stdout: "1\n"}
	err := Check(c, Result{Stdout: "2\n", ExitCode: diag.ExitOK})
	if err == nil || !strings.Contains(err.Error(), "stdout") {
		t.Fatalf("err = %v, want stdout mismatch", err)
	}
}

func TestCheckFailsOnUnexpectedNonZeroExit(t *testing.T) {
	c := Case{Stdout: ""}
	err := Check(c, Result{Stderr: "error[E0001]: boom\n", ExitCode: diag.ExitRuntimeError})
	if err == nil {
		t.Fatal("want error for unexpected failure")
	}
}

func TestCheckFailsOnUnexpectedStderr(t *testing.T) {
	c := Case{Stdout: ""}
	err := Check(c, Result{Stderr: "warning: unused x\n", ExitCode: diag.ExitOK})
	if err == nil || !strings.Contains(err.Error(), "stderr") {
		t.Fatalf("err = %v, want stderr complaint", err)
	}
}

func TestCheckPassesWhenExpectedErrorIsReported(t *testing.T) {
	c := Case{ErrorCode: "E0204"}
	for _, code := range []int{diag.ExitRuntimeError, diag.ExitCompileError} {
		r := Result{Stderr: "error[E0204]: undefined name `fiels`\n  --> t.fung:1:7\n", ExitCode: code}
		if err := Check(c, r); err != nil {
			t.Errorf("exit %d: %v", code, err)
		}
	}
}

func TestCheckFailsWhenWrongErrorIsReported(t *testing.T) {
	c := Case{ErrorCode: "E0204"}
	r := Result{Stderr: "error[E0510]: cannot add\n", ExitCode: diag.ExitRuntimeError}
	err := Check(c, r)
	if err == nil || !strings.Contains(err.Error(), "E0204") {
		t.Fatalf("err = %v, want mention of expected code", err)
	}
}

func TestCheckFailsWhenExpectedErrorDidNotHappen(t *testing.T) {
	c := Case{ErrorCode: "E0204"}
	if err := Check(c, Result{ExitCode: diag.ExitOK}); err == nil {
		t.Fatal("want error when program succeeded but an error was expected")
	}
}

func TestCheckRejectsUsageExitEvenWhenErrorExpected(t *testing.T) {
	// Exit 3 means the fung command itself could not start; that is a
	// test-harness problem, never a legitimate expected error.
	c := Case{ErrorCode: "E0204"}
	r := Result{Stderr: "error[E0204]: x\n", ExitCode: diag.ExitUsage}
	if err := Check(c, r); err == nil {
		t.Fatal("want error for usage exit code")
	}
}

func TestCheckComparesStdoutProducedBeforeTheError(t *testing.T) {
	c := Case{Stdout: "before\n", ErrorCode: "E0001"}
	r := Result{Stdout: "other\n", Stderr: "error[E0001]: x\n", ExitCode: diag.ExitRuntimeError}
	if err := Check(c, r); err == nil {
		t.Fatal("want stdout mismatch even when error matches")
	}
}

func TestParseDirectives(t *testing.T) {
	c, err := Parse("t.fung", []byte("# args: a b\n# stdin: one\n# stdin: two\nprint(1)\n# expect: 1\n# expect-exit: 3\n"))
	if err != nil {
		t.Fatal(err)
	}
	if len(c.Args) != 2 || c.Args[0] != "a" || c.Args[1] != "b" || c.Stdin != "one\ntwo\n" || c.ExitCode != 3 {
		t.Fatalf("case = %+v", c)
	}
	if err := Check(c, Result{Stdout: "1\n", ExitCode: 3}); err != nil {
		t.Fatal(err)
	}
	if err := Check(c, Result{Stdout: "1\n", ExitCode: 0}); err == nil {
		t.Fatal("wrong exit code must fail")
	}
	if _, err := Parse("t.fung", []byte("# expect-exit: x\n")); err == nil {
		t.Fatal("malformed exit code")
	}
	if _, err := Parse("t.fung", []byte("# expect-exit: 1\n# expect-error: E0204\n")); err == nil {
		t.Fatal("expect-exit and expect-error together")
	}
}
