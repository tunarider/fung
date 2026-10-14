package vm

import (
	"sync"
	"time"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// Channel is a queue between tasks (05). The Go channel is never closed:
// closing is a separate signal, so a send after close is Err({kind:
// 'closed}) rather than a Go panic, and values buffered before the close
// can still be received.
type Channel struct {
	ch   chan value.Value
	done chan struct{}
	once sync.Once
}

func (*Channel) TypeName() string { return "channel" }
func (*Channel) Display() string  { return "<channel>" }

func (c *Channel) closed() bool {
	select {
	case <-c.done:
		return true
	default:
		return false
	}
}

func okTag(v value.Value) value.Value {
	return &value.Tag{Name: value.Intern("Ok"), Args: []value.Value{v}}
}

// send delivers v, waiting for buffer room or a receiver (05).
func (c *Channel) send(t *Task, v value.Value) (value.Value, *diag.Error) {
	if c.closed() {
		return errKind("closed"), nil
	}
	select {
	case c.ch <- v:
		return okTag(value.Nil{}), nil
	default:
	}
	sent, closed := false, false
	if err := t.block(false, func() {
		select {
		case c.ch <- v:
			sent = true
		case <-c.done:
			closed = true
		case <-t.done():
		}
	}); err != nil {
		return nil, err
	}
	switch {
	case sent:
		return okTag(value.Nil{}), nil
	case closed:
		return errKind("closed"), nil
	case t.isCancelled():
		return nil, diag.Errorf("E0701", diag.Pos{})
	}
	return errKind("closed"), nil
}

// recv takes one value: buffered values come out even after close; an
// empty closed channel is Err closed; a timeout is Err timeout.
func (c *Channel) recv(t *Task, timed bool, d time.Duration) (value.Value, *diag.Error) {
	select {
	case v := <-c.ch:
		return okTag(v), nil
	default:
	}
	if c.closed() {
		return errKind("closed"), nil
	}
	var got value.Value
	gotOK, closed, timedOut := false, false, false
	if err := t.block(timed, func() {
		var timer <-chan time.Time
		if timed {
			tm := time.NewTimer(d)
			defer tm.Stop()
			timer = tm.C
		}
		select {
		case v := <-c.ch:
			got, gotOK = v, true
		case <-c.done:
			closed = true
		case <-timer:
			timedOut = true
		case <-t.done():
		}
	}); err != nil {
		return nil, err
	}
	switch {
	case gotOK:
		return okTag(got), nil
	case closed:
		select {
		case v := <-c.ch:
			return okTag(v), nil
		default:
			return errKind("closed"), nil
		}
	case t.isCancelled():
		return nil, diag.Errorf("E0701", diag.Pos{})
	case timedOut:
		return errKind("timeout"), nil
	}
	return errKind("closed"), nil
}

// Drain implements value.Iterable: receive until the channel closes.
func (c *Channel) Drain(ctx value.Context) ([]value.Value, error) {
	t := ctx.(*Task)
	out := []value.Value{}
	for {
		r, err := c.recv(t, false, 0)
		if err != nil {
			return nil, err
		}
		tag := r.(*value.Tag)
		if tag.Name.Name() == "Err" {
			return out, nil
		}
		out = append(out, tag.Args[0])
	}
}

func registerChannels(add func(*value.Native)) {
	add(&value.Native{Name: "channel", Params: []value.NativeParam{{Name: "n", Default: value.Int(0)}}, Fn: channelFn})
	add(&value.Native{Name: "send", Params: []value.NativeParam{{Name: "ch"}, {Name: "v"}}, Fn: sendFn})
	add(&value.Native{Name: "recv", Params: []value.NativeParam{{Name: "ch"}, {Name: "timeout", Default: value.Nil{}}}, Fn: recvFn})
	add(&value.Native{Name: "close", Params: []value.NativeParam{{Name: "ch"}}, Fn: closeFn})
}

func needChannel(v value.Value) (*Channel, error) {
	c, ok := v.(*Channel)
	if !ok {
		return nil, diag.Errorf("E0403", diag.Pos{}, "ch", "channel", v.TypeName())
	}
	return c, nil
}

func channelFn(_ value.Context, args []value.Value) (value.Value, error) {
	n, ok := args[0].(value.Int)
	if !ok || n < 0 {
		return nil, diag.Errorf("E0403", diag.Pos{}, "n", "a non-negative int", value.Format(args[0]))
	}
	return &Channel{ch: make(chan value.Value, int(n)), done: make(chan struct{})}, nil
}

func sendFn(ctx value.Context, args []value.Value) (value.Value, error) {
	c, err := needChannel(args[0])
	if err != nil {
		return nil, err
	}
	v, derr := c.send(ctx.(*Task), args[1])
	if derr != nil {
		return nil, derr
	}
	return v, nil
}

func recvFn(ctx value.Context, args []value.Value) (value.Value, error) {
	c, err := needChannel(args[0])
	if err != nil {
		return nil, err
	}
	d, timed, err := timeoutArg(args[1])
	if err != nil {
		return nil, err
	}
	v, derr := c.recv(ctx.(*Task), timed, d)
	if derr != nil {
		return nil, derr
	}
	return v, nil
}

func closeFn(_ value.Context, args []value.Value) (value.Value, error) {
	c, err := needChannel(args[0])
	if err != nil {
		return nil, err
	}
	c.once.Do(func() { close(c.done) })
	return okTag(value.Nil{}), nil
}
