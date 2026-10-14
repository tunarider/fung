package native

import (
	"context"
	"errors"
	"io"
	"net"
	"net/http"
	"sort"
	"strings"
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/nat"
	"github.com/tunarider/fung/internal/value"
)

// httpModule builds _native.http (06): a client over net/http and, in
// serve, a server whose handlers run as fung tasks.
func httpModule() *value.Module {
	m := value.NewModule("http", "_native.http")
	add := func(n *value.Native) { m.Set(n.Name, n) }
	defTimeout := value.Dur(30 * time.Second)
	add(nat.Def("get", httpGetFn, nat.Param("url"), nat.Opt("headers", value.NewMap()), nat.Opt("timeout", defTimeout)))
	add(nat.Def("post", httpPostFn, nat.Param("url"), nat.Param("body"), nat.Opt("headers", value.NewMap()), nat.Opt("timeout", defTimeout)))
	add(nat.Def("request", httpRequestFn, nat.Param("method"), nat.Param("url"), nat.Opt("body", value.Nil{}), nat.Opt("headers", value.NewMap()), nat.Opt("timeout", defTimeout)))
	add(nat.Def("serve", httpServeFn, nat.Param("addr"), nat.Param("handler"), nat.Opt("ready", value.Nil{})))
	return m
}

// taskContext derives a Go context from the task's cancellation signal
// and an optional timeout (0: none).
func taskContext(done <-chan struct{}, timeout time.Duration) (context.Context, context.CancelFunc) {
	var ctx context.Context
	var cancel context.CancelFunc
	if timeout > 0 {
		ctx, cancel = context.WithTimeout(context.Background(), timeout)
	} else {
		ctx, cancel = context.WithCancel(context.Background())
	}
	if done != nil {
		// Ends when the task is cancelled or when cancel() is called, so
		// the goroutine never outlives the request.
		go func() {
			select {
			case <-done:
				cancel()
			case <-ctx.Done():
			}
		}()
	}
	return ctx, cancel
}

// headerMap lowers header names and joins repeated values (06).
func headerMap(h http.Header) *value.Map {
	keys := make([]string, 0, len(h))
	for k := range h {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	m := value.NewMapBuilder()
	for _, k := range keys {
		m.Set(strings.ToLower(k), value.Str(strings.Join(h[k], ", ")))
	}
	return m.Map()
}

// requestArgs reads the shared (body, headers, timeout) arguments.
func requestArgs(body, headers, timeout value.Value) (string, bool, *value.Map, time.Duration, error) {
	b, hasBody, err := nat.OptStr("body", body)
	if err != nil {
		return "", false, nil, 0, err
	}
	h, ok := headers.(*value.Map)
	if !ok {
		return "", false, nil, 0, diag.Errorf("E0403", diag.Pos{}, "headers", "map", headers.TypeName())
	}
	d, ok := timeout.(value.Dur)
	if !ok {
		return "", false, nil, 0, diag.Errorf("E0403", diag.Pos{}, "timeout", "dur", timeout.TypeName())
	}
	return b, hasBody, h, time.Duration(d), nil
}

func httpGetFn(ctx value.Context, args []value.Value) (value.Value, error) {
	return doRequest(ctx, "GET", args[0], value.Nil{}, args[1], args[2])
}

func httpPostFn(ctx value.Context, args []value.Value) (value.Value, error) {
	return doRequest(ctx, "POST", args[0], args[1], args[2], args[3])
}

func httpRequestFn(ctx value.Context, args []value.Value) (value.Value, error) {
	method, err := nat.NeedStr("method", args[0])
	if err != nil {
		return nil, err
	}
	return doRequest(ctx, strings.ToUpper(method), args[1], args[2], args[3], args[4])
}

// doRequest performs one request. Any response is Ok (06: 404도 Ok);
// transport failures are Err 'network, deadlines Err 'timeout, and a
// cancelled task unwinds with E0701 like every blocking primitive.
func doRequest(ctx value.Context, method string, urlV, bodyV, headersV, timeoutV value.Value) (value.Value, error) {
	url, err := nat.NeedStr("url", urlV)
	if err != nil {
		return nil, err
	}
	body, hasBody, headers, timeout, err := requestArgs(bodyV, headersV, timeoutV)
	if err != nil {
		return nil, err
	}
	var reader io.Reader
	if hasBody {
		reader = strings.NewReader(body)
	}
	gctx, cancel := taskContext(ctx.Done(), timeout)
	defer cancel()
	req, rerr := http.NewRequestWithContext(gctx, method, url, reader)
	if rerr != nil {
		return nat.ErrValue("network", "cannot request `"+url+"`: "+rerr.Error()), nil
	}
	for _, k := range headers.Keys() {
		v, _ := headers.Get(k)
		s, ok := v.(value.Str)
		if !ok {
			return nil, diag.Errorf("E0403", diag.Pos{}, "headers", "a map of strings", "a map holding "+v.TypeName())
		}
		req.Header.Set(k, string(s))
	}
	resp, derr := http.DefaultClient.Do(req)
	if derr != nil {
		return requestError(ctx, url, gctx, derr)
	}
	defer resp.Body.Close()
	data, rerr := io.ReadAll(resp.Body)
	if rerr != nil {
		return requestError(ctx, url, gctx, rerr)
	}
	m := value.NewMapBuilder()
	m.Set("status", value.Int(resp.StatusCode))
	m.Set("headers", headerMap(resp.Header))
	m.Set("body", value.Str(string(data)))
	return nat.Ok(m.Map()), nil
}

// requestError sorts a failed request: the task was cancelled, the
// deadline passed, or the transport failed.
func requestError(ctx value.Context, url string, gctx context.Context, err error) (value.Value, error) {
	select {
	case <-ctx.Done():
		return nil, diag.Errorf("E0701", diag.Pos{})
	default:
	}
	if errors.Is(gctx.Err(), context.DeadlineExceeded) || errors.Is(err, context.DeadlineExceeded) {
		return nat.ErrValue("timeout", "request to `"+url+"` timed out"), nil
	}
	return nat.ErrValue("network", "cannot request `"+url+"`: "+nat.Reason(err)), nil
}

// httpServeFn binds addr and serves until the task is cancelled (06).
// Each request runs the handler on a fresh fung task via RunTask.
func httpServeFn(ctx value.Context, args []value.Value) (value.Value, error) {
	addr, err := nat.NeedStr("addr", args[0])
	if err != nil {
		return nil, err
	}
	if err := nat.NeedFn("handler", args[1]); err != nil {
		return nil, err
	}
	handler := args[1]
	if _, isNil := args[2].(value.Nil); !isNil {
		if err := nat.NeedFn("ready", args[2]); err != nil {
			return nil, err
		}
	}
	ln, lerr := net.Listen("tcp", addr)
	if lerr != nil {
		return nat.ErrValue("network", "cannot listen on `"+addr+"`: "+nat.Reason(lerr)), nil
	}
	if ready, ok := args[2].(value.Fn); ok {
		if _, cerr := ctx.Call(ready, []value.Value{value.Str(ln.Addr().String())}); cerr != nil {
			ln.Close()
			return nil, cerr
		}
	}
	srv := &http.Server{
		Handler: http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			serveOne(ctx, handler, w, r)
		}),
		ReadHeaderTimeout: readHeaderTimeout,
	}
	go srv.Serve(ln)
	<-ctx.Done()
	shutdownCtx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	srv.Shutdown(shutdownCtx)
	return nil, diag.Errorf("E0701", diag.Pos{})
}

// Server limits (06): a client may not hold a connection open without
// sending its headers, nor push an unbounded body into memory.
const (
	readHeaderTimeout = 10 * time.Second
	maxRequestBody    = 16 << 20
)

// requestValue builds the handler's req map (06).
func requestValue(w http.ResponseWriter, r *http.Request) (value.Value, error) {
	body, err := io.ReadAll(http.MaxBytesReader(w, r.Body, maxRequestBody))
	if err != nil {
		return nil, err
	}
	query := value.NewMapBuilder()
	q := r.URL.Query()
	keys := make([]string, 0, len(q))
	for k := range q {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		query.Set(k, value.Str(q.Get(k)))
	}
	m := value.NewMapBuilder()
	m.Set("method", value.Str(r.Method))
	m.Set("path", value.Str(r.URL.Path))
	m.Set("query", query.Map())
	m.Set("headers", headerMap(r.Header))
	m.Set("body", value.Str(string(body)))
	return m.Map(), nil
}

// serveOne runs the handler for one request and writes its answer. A
// handler error or an unusable result is a 500 reported on stderr; the
// server goes on (06).
func serveOne(ctx value.Context, handler value.Value, w http.ResponseWriter, r *http.Request) {
	req, err := requestValue(w, r)
	if err != nil {
		var tooBig *http.MaxBytesError
		if errors.As(err, &tooBig) {
			http.Error(w, "request body too large", http.StatusRequestEntityTooLarge)
			return
		}
		http.Error(w, "cannot read the request body", http.StatusBadRequest)
		return
	}
	res, cerr := ctx.RunTask(handler, []value.Value{req})
	if cerr != nil {
		if isShutdown(cerr) {
			// The program is ending (exit, cancellation, deadlock): not
			// this handler's fault, so no report.
			http.Error(w, "server shutting down", http.StatusServiceUnavailable)
			return
		}
		io.WriteString(ctx.Stderr(), "http: handler failed: "+cerr.Error()+"\n")
		http.Error(w, "handler failed", http.StatusInternalServerError)
		return
	}
	status, headers, body, why := responseParts(res)
	if why != "" {
		io.WriteString(ctx.Stderr(), "http: handler response: "+why+"\n")
		http.Error(w, "handler failed", http.StatusInternalServerError)
		return
	}
	for k, v := range headers {
		w.Header().Set(k, v)
	}
	if w.Header().Get("Content-Type") == "" && body != "" {
		w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	}
	w.WriteHeader(status)
	io.WriteString(w, body)
}

// isShutdown tells the errors that mean "the program is ending" from a
// handler bug: an exit signal, a cancelled task, or the deadlock report.
func isShutdown(err error) bool {
	de, ok := err.(*diag.Error)
	if !ok {
		return false
	}
	if _, isExit := de.ExitCode(); isExit {
		return true
	}
	return de.Code == "E0701" || de.Code == "E0702"
}

// responseParts reads a handler result: a str is a 200 text answer, a
// map carries {status = 200, headers = {}, body = ""}. why names what
// is wrong with an unusable result ("" when it is fine).
func responseParts(res value.Value) (status int, headers map[string]string, body string, why string) {
	switch x := res.(type) {
	case value.Str:
		return 200, nil, string(x), ""
	case *value.Map:
		status = 200
		if v, ok := x.Get("status"); ok {
			n, isInt := v.(value.Int)
			if !isInt {
				return 0, nil, "", "`status` must be int, got " + v.TypeName()
			}
			if n < 100 || n > 599 {
				return 0, nil, "", "`status` must be between 100 and 599, got " + value.Format(n)
			}
			status = int(n)
		}
		if v, ok := x.Get("body"); ok {
			s, isStr := v.(value.Str)
			if !isStr {
				return 0, nil, "", "`body` must be str, got " + v.TypeName()
			}
			body = string(s)
		}
		headers = map[string]string{}
		if v, ok := x.Get("headers"); ok {
			hm, isMap := v.(*value.Map)
			if !isMap {
				return 0, nil, "", "`headers` must be map, got " + v.TypeName()
			}
			for _, k := range hm.Keys() {
				hv, _ := hm.Get(k)
				s, isStr := hv.(value.Str)
				if !isStr {
					return 0, nil, "", "header `" + k + "` must be str, got " + hv.TypeName()
				}
				headers[k] = string(s)
			}
		}
		return status, headers, body, ""
	}
	return 0, nil, "", "handler returned " + res.TypeName() + ", expected a str or a map"
}
