package vm

import (
	"bytes"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/prelude"
	"github.com/tunarider/fung/internal/value"
)

// concPrelude is the real prelude plus the concurrency natives.
func concPrelude() map[string]value.Value {
	p := prelude.Test()
	for k, v := range Concurrency() {
		p[k] = v
	}
	return p
}

// runConc compiles with the concurrency prelude, runs the entry, waits
// for spawned tasks, and returns the output and error code.
func runConc(t *testing.T, src string) (value.Value, string, string) {
	t.Helper()
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out, Stderr: &out}
	b := buildWith(t, src, bootTestPrelude(t, rt))
	registerModules(rt, b)
	v, err := rt.Run(b.Proto)
	rt.Wait()
	code := ""
	if err != nil {
		code = err.Code
	}
	return v, out.String(), code
}

func TestSleepWaitsAndIsCancellable(t *testing.T) {
	p := compileWith(t, "sleep(20ms)\nprint(1)", concPrelude())
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	start := time.Now()
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	if time.Since(start) < 15*time.Millisecond || out.String() != "1\n" {
		t.Fatalf("slept %v, out %q", time.Since(start), out.String())
	}
	p = compileWith(t, "sleep(10s)\nprint(1)", concPrelude())
	rt = &Runtime{Stdout: &out}
	go func() { time.Sleep(10 * time.Millisecond); rt.CancelAll() }()
	_, err := rt.Run(p)
	if err == nil || err.Code != "E0701" {
		t.Fatalf("cancelled sleep: %v", err)
	}
	if _, _, code := runConc(t, "sleep(1)"); code != "E0403" {
		t.Fatalf("sleep(1): %s", code)
	}
	if _, _, code := runConc(t, "sleep(-1s)"); code != "" {
		t.Fatalf("negative sleep is a no-op: %s", code)
	}
}

func TestLockedWritersKeepLinesWhole(t *testing.T) {
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	task := &Task{rt: rt}
	var wg sync.WaitGroup
	for i := 0; i < 50; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			task.Stdout().Write([]byte(strings.Repeat("x", 100) + "\n"))
		}()
	}
	wg.Wait()
	for _, line := range strings.Split(strings.TrimSpace(out.String()), "\n") {
		if len(line) != 100 {
			t.Fatalf("torn line: %d", len(line))
		}
	}
}

func TestExitFromAnywhereRequestsExit(t *testing.T) {
	p := compileWith(t, "fn f() = exit(4)\nf()", concPrelude())
	rt := &Runtime{Stdout: &bytes.Buffer{}}
	_, err := rt.Run(p)
	if code, ok := rt.ExitCode(); !ok || code != 4 || err == nil {
		t.Fatalf("code=%d ok=%v err=%v", code, ok, err)
	}
	if rt.root().Err() == nil {
		t.Fatal("exit cancels the root context")
	}
}

func TestSpawnAwaitAndResults(t *testing.T) {
	cases := map[string]string{
		"t = spawn(() => 1 + 1)\nprint(await(t), await(t))":                                                                            "2 2\n",
		"t = spawn(() => Err(\"e\"))\nprint(await(t))":                                                                                 "Err(\"e\")\n",
		"t = spawn(() => 1 / 0)\nprint(await(t))":                                                                                      "Err({kind: 'error, message: \"error[E0502]: division by zero\"})\n",
		"t = spawn(() => sleep(10s))\ncancel(t)\nprint(await(t))":                                                                      "Err({kind: 'cancelled})\n",
		"t = spawn(() => sleep(10s))\nprint(await(t, timeout: 10ms))\ncancel(t)":                                                       "Err({kind: 'timeout})\n",
		"ts = for i in 0..5: spawn(() => i * i) ;;\nprint(ts |> map(await))":                                                           "[0, 1, 4, 9, 16]\n",
		"print(type_of(spawn(() => 1)), spawn(() => 1) == spawn(() => 1))":                                                             "'task false\n",
		"outer = spawn(fn():\n  inner = spawn(() => sleep(10s))\n  await(inner)\n;;)\nsleep(10ms)\ncancel(outer)\nprint(await(outer))": "Err({kind: 'cancelled})\n",
		"t = spawn(() => 1)\nprint(await(t), cancel(t), await(t))":                                                                     "1 nil 1\n", // cancelling a finished task changes nothing
	}
	for src, want := range cases {
		_, out, code := runConc(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	for src, want := range map[string]string{"spawn(1)": "E0403", "await(1)": "E0403", "cancel(1)": "E0403", "await(spawn(() => 1), timeout: 1)": "E0403"} {
		if _, _, code := runConc(t, src); code != want {
			t.Errorf("%q: code = %s, want %s", src, code, want)
		}
	}
}

func TestUnawaitedFailureIsReportedOnce(t *testing.T) {
	src := "spawn(() => fail(\"boom\"))\nsleep(20ms)\nprint(\"after\")"
	p := compileWith(t, src, concPrelude())
	var out, errOut bytes.Buffer
	rt := &Runtime{Stdout: &out, Stderr: &errOut, Sources: map[string]string{"t.fung": src}}
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	rt.Wait()
	if out.String() != "after\n" || !strings.HasPrefix(errOut.String(), "task failed: error[E0803]: boom\n  --> t.fung:1:") || !rt.Failed() {
		t.Fatalf("out=%q err=%q failed=%v", out.String(), errOut.String(), rt.Failed())
	}
	if strings.Count(errOut.String(), "task failed") != 1 {
		t.Fatalf("reported more than once: %q", errOut.String())
	}
	out.Reset()
	errOut.Reset()
	rt = &Runtime{Stdout: &out, Stderr: &errOut}
	p = compileWith(t, "t = spawn(() => fail(\"boom\"))\nprint(await(t) |> is_err)", bootTestPrelude(t, rt))
	rt.Run(p)
	rt.Wait()
	if out.String() != "true\n" || errOut.String() != "" || rt.Failed() {
		t.Fatalf("awaited: out=%q err=%q failed=%v", out.String(), errOut.String(), rt.Failed())
	}
	// A cancelled task is not a failure.
	p = compileWith(t, "t = spawn(() => sleep(10s))\ncancel(t)\nsleep(10ms)", concPrelude())
	errOut.Reset()
	rt = &Runtime{Stdout: &out, Stderr: &errOut}
	rt.Run(p)
	rt.Wait()
	if errOut.String() != "" || rt.Failed() {
		t.Fatalf("cancelled: err=%q failed=%v", errOut.String(), rt.Failed())
	}
}

func TestAwaitOnSelfIsADeadlock(t *testing.T) {
	// The entry waits for a task that waits for the entry's own handle
	// chain: with one live task blocked and nothing to wake it, E0702.
	var errOut bytes.Buffer
	p := compileWith(t, "t = spawn(fn(): sleep(10s) ;;)\nu = spawn(() => await(t))\nawait(u)\n", concPrelude())
	rt := &Runtime{Stdout: &bytes.Buffer{}, Stderr: &errOut}
	go func() { time.Sleep(20 * time.Millisecond); rt.CancelAll() }()
	if _, err := rt.Run(p); err != nil && err.Code != "E0701" {
		t.Fatalf("a sleeper keeps it alive (no deadlock) until cancelled: %v", err)
	}
	rt.Wait()
	if strings.Contains(errOut.String(), "deadlock") {
		t.Fatalf("false deadlock: %q", errOut.String())
	}
}

func TestChannels(t *testing.T) {
	cases := map[string]string{
		"ch = channel()\nspawn(() => send(ch, 1))\nprint(recv(ch))":                                                                           "Ok(1)\n",
		"ch = channel(2)\nsend(ch, 1)\nsend(ch, 2)\nclose(ch)\nprint(recv(ch), recv(ch), recv(ch), send(ch, 3))":                              "Ok(1) Ok(2) Err({kind: 'closed}) Err({kind: 'closed})\n",
		"ch = channel()\nprint(recv(ch, timeout: 10ms))":                                                                                      "Err({kind: 'timeout})\n",
		"ch = channel(3)\neach(1..=3, x => send(ch, x))\nclose(ch)\nprint(for x in ch: x * 10 ;;)":                                            "[10, 20, 30]\n",
		"ch = channel(3)\neach([3, 1, 2], x => send(ch, x))\nclose(ch)\nprint(sort(ch), close(ch))":                                           "[1, 2, 3] Ok(nil)\n",
		"ch = channel(1)\nt = spawn(fn():\n  each(0..100, i => send(ch, i))\n  close(ch)\n;;)\nprint(fold(ch, 0, (a, x) => a + x), await(t))": "4950 Ok(nil)\n",
		"print(type_of(channel()), channel() == channel())":                                                                                   "'channel false\n",
		"ch = channel()\nt = spawn(() => recv(ch))\nsend(ch, 'hi)\nprint(await(t))":                                                           "Ok('hi)\n",
		"ch = channel()\nt = spawn(() => send(ch, 1))\ncancel(t)\nprint(await(t) |> is_err)":                                                  "true\n",
	}
	for src, want := range cases {
		_, out, code := runConc(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	for src, want := range map[string]string{"channel(-1)": "E0403", "channel(\"a\")": "E0403", "send(1, 2)": "E0403", "recv(1)": "E0403", "close(1)": "E0403", "recv(channel(), timeout: 1)": "E0403"} {
		if _, _, code := runConc(t, src); code != want {
			t.Errorf("%q: %s, want %s", src, code, want)
		}
	}
}

func TestDeadlockIsDetected(t *testing.T) {
	var errOut bytes.Buffer
	p := compileWith(t, "ch = channel()\nrecv(ch)", concPrelude())
	rt := &Runtime{Stdout: &bytes.Buffer{}, Stderr: &errOut}
	_, err := rt.Run(p)
	if err == nil || err.Code != "E0702" || !strings.Contains(errOut.String(), "error[E0702]: deadlock") {
		t.Fatalf("err=%v stderr=%q", err, errOut.String())
	}
	if code, ok := rt.ExitCode(); !ok || code != 1 {
		t.Fatalf("exit code after deadlock = %d %v", code, ok)
	}
	p = compileWith(t, "ch = channel()\nt = spawn(() => recv(ch))\nawait(t)", concPrelude())
	rt = &Runtime{Stdout: &bytes.Buffer{}, Stderr: &errOut}
	if _, err := rt.Run(p); err == nil || (err.Code != "E0702" && err.Code != "E0701") {
		t.Fatalf("mutual wait: %v", err)
	}
	rt.Wait()
	// A sleeper keeps the program alive: a recv with a timeout returns
	// instead of a false deadlock, and an unbuffered send meets its receiver.
	p = compileWith(t, "t = spawn(() => sleep(30ms))\nch = channel()\nprint(recv(ch, timeout: 10ms))\nawait(t)", concPrelude())
	var out bytes.Buffer
	rt = &Runtime{Stdout: &out, Stderr: &bytes.Buffer{}}
	if _, err := rt.Run(p); err != nil || out.String() != "Err({kind: 'timeout})\n" {
		t.Fatalf("false deadlock: %v %q", err, out.String())
	}
	p = compileWith(t, "ch = channel()\nspawn(fn(): sleep(10ms); send(ch, 1) ;;)\nprint(recv(ch))", concPrelude())
	out.Reset()
	rt = &Runtime{Stdout: &out, Stderr: &bytes.Buffer{}}
	if _, err := rt.Run(p); err != nil || out.String() != "Ok(1)\n" {
		t.Fatalf("receiver waiting for a sleeping sender: %v %q", err, out.String())
	}
}

func TestAtoms(t *testing.T) {
	cases := map[string]string{
		"a = atom(0)\nprint(swap(a, x => x + 1), deref(a), a)":                                                                       "1 1 <atom 1>\n",
		"a = atom(0)\nts = for _ in 0..10: spawn(() => each(0..100, _ => swap(a, x => x + 1))) ;;\neach(ts, await)\nprint(deref(a))": "1000\n",
		"a = atom([])\nswap(a, xs => xs + [1])\nprint(deref(a), type_of(a), a == a, atom(1) == atom(1))":                             "[1] 'atom true false\n",
		"a = atom(0)\nb = atom(0)\nprint(swap(a, x => swap(b, y => y + 1) + x))":                                                     "1\n", // a different atom inside swap is fine
	}
	for src, want := range cases {
		_, out, code := runConc(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
	if _, _, code := runConc(t, "a = atom(0)\nswap(a, x => swap(a, y => y))"); code != "E0703" {
		t.Errorf("re-entrant swap: %s", code)
	}
	for src, want := range map[string]string{"swap(1, x => x)": "E0403", "a = atom(0)\nswap(a, 1)": "E0403", "deref(1)": "E0403"} {
		if _, _, code := runConc(t, src); code != want {
			t.Errorf("%q: %s, want %s", src, code, want)
		}
	}
	if _, _, code := runConc(t, "a = atom(0)\nswap(a, x => 1 / 0)\nprint(deref(a))"); code != "E0502" {
		t.Errorf("a failing f leaves the atom unlocked and unchanged: %s", code)
	}
	if _, out, _ := runConc(t, "a = atom(5)\nt = spawn(() => swap(a, x => 1 / 0))\nawait(t)\nprint(swap(a, x => x + 1))"); out != "6\n" {
		t.Errorf("after a failed swap in another task the atom is usable: %q", out)
	}
}

func TestDeadlockAfterTheLastRunnerFinishes(t *testing.T) {
	// The sender finishes without sending: the receiver is the only task
	// left and it can never wake. Go's own detector must not get there.
	var errOut bytes.Buffer
	p := compileWith(t, "ch = channel()\nt = spawn(() => 1)\nprint(recv(ch))", concPrelude())
	rt := &Runtime{Stdout: &bytes.Buffer{}, Stderr: &errOut}
	if _, err := rt.Run(p); err == nil || err.Code != "E0702" {
		t.Fatalf("err=%v stderr=%q", err, errOut.String())
	}
	rt.Wait()
	// An orphan blocked after the entry is done: Wait returns because the
	// deadlock report cancels it.
	errOut.Reset()
	p = compileWith(t, "spawn(() => recv(channel()))\nprint(\"entry done\")", concPrelude())
	rt = &Runtime{Stdout: &bytes.Buffer{}, Stderr: &errOut}
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	done := make(chan struct{})
	go func() { rt.Wait(); close(done) }()
	select {
	case <-done:
	case <-time.After(3 * time.Second):
		t.Fatal("orphan was never judged")
	}
	if !strings.Contains(errOut.String(), "E0702") {
		t.Fatalf("stderr=%q", errOut.String())
	}
	// A test body and an exit handler count as live too.
	p = compileWith(t, "fn spin(n) = n == 0 then 0 else spin(n - 1)\nt = spawn(() => spin(300000))\nprint(await(t) == 0)", concPrelude())
	var out bytes.Buffer
	rt = &Runtime{Stdout: &out, Stderr: &errOut}
	b := buildWith(t, "test \"waits for a worker\":\n  t = spawn(() => 1)\n  assert await(t) == 1\n;;", concPrelude())
	_ = b
	if err := rt.CallTest(&bytecode.Closure{Proto: p, Globals: make([]value.Value, len(p.GlobalNames))}); err != nil {
		t.Fatalf("CallTest body awaiting a worker: %v", err)
	}
	if out.String() != "true\n" {
		t.Fatalf("out=%q", out.String())
	}
}

func TestDerefAndPrintInsideSwap(t *testing.T) {
	cases := map[string]string{
		"a = atom(1)\nprint(swap(a, x => deref(a) + x))":         "2\n",
		"a = atom(1)\nprint(swap(a, fn(x): print(a); x + 1 ;;))": "<atom 1>\n2\n",
	}
	for src, want := range cases {
		_, out, code := runConc(t, src)
		if code != "" || out != want {
			t.Errorf("%q: out=%q code=%s, want %q", src, out, code, want)
		}
	}
}

func TestAwaitTimeoutDoesNotHideALaterFailure(t *testing.T) {
	p := compileWith(t, "t = spawn(fn(): sleep(20ms); fail(\"later\") ;;)\nprint(await(t, timeout: 1ms))\nsleep(80ms)", concPrelude())
	var out, errOut bytes.Buffer
	rt := &Runtime{Stdout: &out, Stderr: &errOut}
	rt.Run(p)
	rt.Wait()
	if !strings.Contains(out.String(), "timeout") || !strings.Contains(errOut.String(), "task failed: error[E0803]: later") || !rt.Failed() {
		t.Fatalf("out=%q err=%q failed=%v", out.String(), errOut.String(), rt.Failed())
	}
}

func TestAtExitAndTestsAreThreadSafe(t *testing.T) {
	p := compileWith(t, "ts = for i in 0..200: spawn(() => at_exit(() => print(\"h\"))) ;;\neach(ts, await)", concPrelude())
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out}
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	rt.Wait()
	rt.RunExitHandlers()
	if n := strings.Count(out.String(), "h\n"); n != 200 {
		t.Fatalf("handlers ran %d times", n)
	}
}

func TestStdinIsSharedSafely(t *testing.T) {
	p := compileWith(t, "t = spawn(() => read_line())\nprint(read_line(), await(t))", concPrelude())
	var out bytes.Buffer
	rt := &Runtime{Stdout: &out, Stdin: strings.NewReader("a\nb\n")}
	if _, err := rt.Run(p); err != nil {
		t.Fatal(err)
	}
	rt.Wait()
	if !strings.Contains(out.String(), "Ok(\"a\")") || !strings.Contains(out.String(), "Ok(\"b\")") {
		t.Fatalf("out=%q", out.String())
	}
}
