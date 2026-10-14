package vm

import (
	"bytes"
	"runtime"
	"strings"
	"testing"
	"time"
)

// startConc compiles src with the concurrency prelude and returns a
// runtime whose stdout and stderr are kept apart.
func startConc(t *testing.T, src string) (rt *Runtime, run func(), out, errOut *bytes.Buffer) {
	t.Helper()
	out, errOut = &bytes.Buffer{}, &bytes.Buffer{}
	rt = &Runtime{Stdout: out, Stderr: errOut, Sources: map[string]string{"t.fung": src}}
	b := buildWith(t, src, bootTestPrelude(t, rt))
	registerModules(rt, b)
	return rt, func() {
		if _, err := rt.Run(b.Proto); err != nil && err.Code != "E0701" {
			t.Errorf("run: %v", err)
		}
		rt.Wait()
	}, out, errOut
}

// 05: a failure is reported only once nobody can retrieve it, so how the
// task and the await happen to be scheduled never changes the outcome.
func TestAwaitAfterTheTaskDiedIsQuiet(t *testing.T) {
	rt, run, out, errOut := startConc(t, "t = spawn(() => fail(\"boom\"))\nsleep(20ms)\nprint(await(t) |> is_err)")
	run()
	rt.ReportUnretrieved()
	if out.String() != "true\n" || errOut.String() != "" || rt.Failed() {
		t.Fatalf("out=%q err=%q failed=%v", out, errOut, rt.Failed())
	}
}

// A handle that is still held can be awaited later, so its failure is
// reported only when the program ends without retrieving it, and once.
func TestHeldFailureIsReportedAtTheEnd(t *testing.T) {
	rt, run, out, errOut := startConc(t, "t = spawn(() => fail(\"boom\"))\nsleep(20ms)\nprint(\"after\")")
	run()
	if errOut.String() != "" || rt.Failed() {
		t.Fatalf("reported while the handle is still held: err=%q", errOut)
	}
	rt.ReportUnretrieved()
	rt.ReportUnretrieved()
	if out.String() != "after\n" || !strings.HasPrefix(errOut.String(), "task failed: error[E0803]: boom\n") || !rt.Failed() {
		t.Fatalf("out=%q err=%q failed=%v", out, errOut, rt.Failed())
	}
	if n := strings.Count(errOut.String(), "task failed"); n != 1 {
		t.Fatalf("reported %d times: %q", n, errOut)
	}
}

// A dropped handle can never be awaited: the GC finds it and the failure
// is reported while the program is still running. kick keeps the handle
// in a local slot deeper than the entry's later calls reach, so a popped
// stack slot that is not cleared would keep it alive.
func TestDroppedHandleIsReportedByTheGC(t *testing.T) {
	src := "fn kick():\n  a = 1\n  b = 2\n  t = spawn(() => fail(\"boom\"))\n  a\n;;\nkick()\nsleep(3s)"
	rt, run, _, errOut := startConc(t, src)
	finished := make(chan struct{})
	go func() { run(); close(finished) }()
	deadline := time.Now().Add(2 * time.Second)
	for !rt.Failed() && time.Now().Before(deadline) {
		time.Sleep(10 * time.Millisecond)
		runtime.GC()
	}
	reported := rt.Failed()
	rt.CancelAll()
	<-finished
	rt.ReportUnretrieved()
	if !reported {
		t.Fatalf("a dropped handle's failure was not reported while the program ran: err=%q", errOut)
	}
	if n := strings.Count(errOut.String(), "task failed"); n != 1 {
		t.Fatalf("reported %d times: %q", n, errOut)
	}
}

// A timed-out await does not retrieve the result (05).
func TestTimedOutAwaitDoesNotRetrieve(t *testing.T) {
	rt, run, out, errOut := startConc(t, "t = spawn(fn(): sleep(30ms); fail(\"late\") ;;)\nprint(await(t, timeout: 1ms))\nsleep(60ms)")
	run()
	rt.ReportUnretrieved()
	if !strings.Contains(out.String(), "timeout") || !strings.Contains(errOut.String(), "task failed: error[E0803]: late") || !rt.Failed() {
		t.Fatalf("out=%q err=%q failed=%v", out, errOut, rt.Failed())
	}
}

// Helpers that await every task they spawn never leave a failure behind,
// even when the task fails before the helper gets to await it.
func TestHelpersThatAwaitNeverReport(t *testing.T) {
	for range 20 {
		rt, run, out, errOut := startConc(t, "r = pmap([1, 2, 3], x => fail(\"boom\"))\nprint(r |> map(is_err))")
		run()
		rt.ReportUnretrieved()
		if out.String() != "[true, true, true]\n" || errOut.String() != "" || rt.Failed() {
			t.Fatalf("out=%q err=%q failed=%v", out, errOut, rt.Failed())
		}
	}
}
