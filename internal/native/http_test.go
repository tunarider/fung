package native

import (
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/nat"
	"github.com/tunarider/fung/internal/value"
)

func httpFn(t *testing.T, name string) *value.Native {
	t.Helper()
	v, ok := Bridges()["_native.http"].Get(name)
	if !ok {
		t.Fatalf("no %s", name)
	}
	return v.(*value.Native)
}

func testServer(t *testing.T) *httptest.Server {
	t.Helper()
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/echo":
			body, _ := io.ReadAll(r.Body)
			w.Header().Set("X-Method", r.Method)
			w.Header().Add("X-Multi", "a")
			w.Header().Add("X-Multi", "b")
			w.Header().Set("Content-Type", "text/plain")
			w.Write([]byte(r.Header.Get("X-In") + "|" + string(body)))
		case "/missing":
			w.WriteHeader(404)
			w.Write([]byte("nope"))
		case "/slow":
			time.Sleep(300 * time.Millisecond)
			w.Write([]byte("late"))
		case "/moved":
			http.Redirect(w, r, "/echo", http.StatusFound)
		}
	}))
	t.Cleanup(srv.Close)
	return srv
}

func TestHTTPClient(t *testing.T) {
	srv := testServer(t)
	c := &nat.FakeContext{}
	headers := value.NewMapBuilder()
	headers.Set("X-In", value.Str("hi"))
	r, err := httpFn(t, "post").Fn(c, []value.Value{value.Str(srv.URL + "/echo"), value.Str("body"), headers.Map(), value.Dur(5 * time.Second)})
	if err != nil {
		t.Fatal(err)
	}
	resp := r.(*value.Tag)
	if resp.Name.Name() != "Ok" {
		t.Fatalf("post: %s", value.Format(r))
	}
	m := resp.Args[0].(*value.Map)
	status, _ := m.Get("status")
	body, _ := m.Get("body")
	hs, _ := m.Get("headers")
	xm, _ := hs.(*value.Map).Get("x-method")
	multi, _ := hs.(*value.Map).Get("x-multi")
	if status != value.Int(200) || body != value.Str("hi|body") || xm != value.Str("POST") || multi != value.Str("a, b") {
		t.Errorf("post response: %s", value.Format(r))
	}
	r, _ = httpFn(t, "get").Fn(c, []value.Value{value.Str(srv.URL + "/missing"), value.NewMap(), value.Dur(5 * time.Second)})
	if !strings.HasPrefix(value.Format(r), "Ok({status: 404, ") {
		t.Errorf("404 must still be Ok: %s", value.Format(r))
	}
	r, _ = httpFn(t, "request").Fn(c, []value.Value{value.Str("PUT"), value.Str(srv.URL + "/echo"), value.Nil{}, value.NewMap(), value.Dur(5 * time.Second)})
	if !strings.Contains(value.Format(r), `"x-method": "PUT"`) {
		t.Errorf("request PUT: %s", value.Format(r))
	}
	r, _ = httpFn(t, "get").Fn(c, []value.Value{value.Str(srv.URL + "/moved"), value.NewMap(), value.Dur(5 * time.Second)})
	if !strings.Contains(value.Format(r), `body: "|"`) {
		t.Errorf("redirect must be followed: %s", value.Format(r))
	}
}

func TestHTTPClientErrors(t *testing.T) {
	srv := testServer(t)
	c := &nat.FakeContext{}
	r, _ := httpFn(t, "get").Fn(c, []value.Value{value.Str(srv.URL + "/slow"), value.NewMap(), value.Dur(50 * time.Millisecond)})
	if !strings.HasPrefix(value.Format(r), "Err({kind: 'timeout, ") {
		t.Errorf("timeout: %s", value.Format(r))
	}
	r, _ = httpFn(t, "get").Fn(c, []value.Value{value.Str("http://127.0.0.1:1/"), value.NewMap(), value.Dur(time.Second)})
	if !strings.HasPrefix(value.Format(r), "Err({kind: 'network, ") {
		t.Errorf("refused: %s", value.Format(r))
	}
	r, _ = httpFn(t, "get").Fn(c, []value.Value{value.Str("::not a url"), value.NewMap(), value.Dur(time.Second)})
	if !strings.HasPrefix(value.Format(r), "Err({kind: 'network, ") {
		t.Errorf("bad url: %s", value.Format(r))
	}
	_, err := httpFn(t, "get").Fn(c, []value.Value{value.Str(srv.URL), value.Str("x"), value.Dur(time.Second)})
	if err == nil || err.(*diag.Error).Code != "E0403" {
		t.Errorf("headers must be a map: %v", err)
	}
	_, err = httpFn(t, "get").Fn(c, []value.Value{value.Str(srv.URL), value.NewMap(), value.Int(1)})
	if err == nil || err.(*diag.Error).Code != "E0403" {
		t.Errorf("timeout must be a dur: %v", err)
	}
}

// cancelContext is a FakeContext whose Done fires when the channel closes.
type cancelContext struct {
	nat.FakeContext
	done chan struct{}
}

func (c *cancelContext) Done() <-chan struct{} { return c.done }

func TestHTTPClientCancel(t *testing.T) {
	srv := testServer(t)
	c := &cancelContext{done: make(chan struct{})}
	go func() { time.Sleep(50 * time.Millisecond); close(c.done) }()
	_, err := httpFn(t, "get").Fn(c, []value.Value{value.Str(srv.URL + "/slow"), value.NewMap(), value.Dur(5 * time.Second)})
	if err == nil || err.(*diag.Error).Code != "E0701" {
		t.Errorf("a cancelled task must unwind with E0701, got %v", err)
	}
}
