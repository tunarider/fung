package vm

import (
	"bytes"
	"strings"
	"testing"
)

// TestHTTPServeHandlerErrors: a failing handler answers 500 and reports
// to stderr, the server keeps serving, and cancelling the server task
// ends it without a failure (05: 취소는 실패가 아니다).
func TestHTTPServeHandlerErrors(t *testing.T) {
	src := `
import http
port = channel(1)
s = spawn(() => http.serve("127.0.0.1:0", req => req.path == "/ok" then "fine" else fail("bug"), ready: a => send(port, a)))
base = "http://" + recv(port)!
print(http.get("${base}/boom")!.status)
print(http.get("${base}/ok")!.body)
print(http.get("${base}/boom")!.status)
cancel(s)
print(await(s))
`
	_, out, code := run(t, src)
	if code != "" {
		t.Fatalf("error %s", code)
	}
	if out != "500\nfine\n500\nErr({kind: 'cancelled})\n" {
		t.Errorf("output:\n%s", out)
	}
}

func TestHTTPServeBindFailure(t *testing.T) {
	_, out, code := run(t, `
import http
print(http.serve("127.0.0.1:1", r => "x"))
`)
	if code != "" || !strings.HasPrefix(out, "Err({kind: 'network, message: \"cannot listen on `127.0.0.1:1`") {
		t.Errorf("code %s, output %s", code, out)
	}
}

// TestHTTPServeExitInHandler: exit(n) inside a handler ends the program
// with that code and is not reported as a handler failure.
func TestHTTPServeExitInHandler(t *testing.T) {
	var out, errOut bytes.Buffer
	rt := &Runtime{Stdout: &out, Stderr: &errOut}
	b := buildWith(t, `
import http
port = channel(1)
s = spawn(() => http.serve("127.0.0.1:0", req => exit(3), ready: a => send(port, a)))
http.get("http://" + recv(port)! + "/")
`, bootTestPrelude(t, rt))
	registerModules(rt, b)
	rt.Run(b.Proto)
	rt.Wait()
	if code, ok := rt.ExitCode(); !ok || code != 3 {
		t.Fatalf("exit code = %d, %v", code, ok)
	}
	if strings.Contains(errOut.String(), "handler failed") {
		t.Errorf("exit must not be reported as a handler failure:\n%s", errOut.String())
	}
}

// TestHTTPServeLimits: a request body over the limit is refused with 413
// and the handler is not called; a wrong-typed response key names the key.
func TestHTTPServeLimits(t *testing.T) {
	var out, errOut bytes.Buffer
	rt := &Runtime{Stdout: &out, Stderr: &errOut}
	b := buildWith(t, `
import http
port = channel(1)
calls = atom(0)
s = spawn(() => http.serve("127.0.0.1:0", fn(req):
  swap(calls, n => n + 1)
  req.path == "/bad" then {status: "200", body: "x"} else "ok"
;;, ready: a => send(port, a)))
base = "http://" + recv(port)!
print(http.post("${base}/", repeat("x", 17 * 1024 * 1024))!.status)
print(http.get("${base}/bad")!.status)
print(deref(calls))
cancel(s)
`, bootTestPrelude(t, rt))
	registerModules(rt, b)
	if _, err := rt.Run(b.Proto); err != nil {
		t.Fatal(err)
	}
	rt.Wait()
	if out.String() != "413\n500\n1\n" {
		t.Errorf("output:\n%s", out.String())
	}
	if !strings.Contains(errOut.String(), "`status` must be int, got str") {
		t.Errorf("stderr must name the bad key:\n%s", errOut.String())
	}
}
