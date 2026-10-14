package vm

import (
	"github.com/tunarider/fung/internal/bytecode"
	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// popN removes the top n values and returns them in push order.
func (t *Task) popN(n int) []value.Value {
	items := append([]value.Value(nil), t.stack[len(t.stack)-n:]...)
	t.truncate(len(t.stack) - n)
	return items
}

// errMessage renders an Err for E0603 (02): a single payload that is a
// map with a string `message` shows that; a payload of another shape
// shows its literal form; an Err with zero or several payloads shows the
// whole tag.
func errMessage(err value.Value) string {
	tag := err.(*value.Tag)
	if len(tag.Args) != 1 {
		return value.Format(err)
	}
	v := tag.Args[0]
	if m, ok := v.(*value.Map); ok {
		if msg, found := m.Get("message"); found {
			if s, ok := msg.(value.Str); ok {
				return string(s)
			}
		}
	}
	return value.Format(v)
}

// resultKind classifies a value for `!`, `?` and `??` (02): Ok payload,
// Err, nil, or something else. `!` rejects nil (E0604), `?` lets it
// through, `??` replaces it; anything else is E0605 for `!` and `?`.
func resultKind(v value.Value) (payload value.Value, isOk, isErr, isNil bool) {
	switch x := v.(type) {
	case *value.Tag:
		switch x.Name.Name() {
		case "Ok":
			if len(x.Args) == 1 {
				return x.Args[0], true, false, false
			}
			return &value.Tuple{Items: x.Args}, true, false, false
		case "Err":
			return nil, false, true, false
		}
	case value.Nil:
		return nil, false, false, true
	}
	return nil, false, false, false
}

// execM4 runs the instructions added in M4 (collections, patterns,
// results). It reports handled=false for opcodes it does not know.
func (t *Task) execM4(f *frame, op bytecode.Op, in bytecode.Instr, pos diag.Pos) (bool, *diag.Error) {
	switch op {
	case bytecode.OpList:
		t.push(value.NewList(t.popN(in.Arg())))
	case bytecode.OpTuple:
		t.push(&value.Tuple{Items: t.popN(in.Arg())})
	case bytecode.OpMap:
		pairs := t.popN(2 * in.Arg())
		m := value.NewMapBuilder()
		for i := 0; i < len(pairs); i += 2 {
			m.Set(string(pairs[i].(value.Str)), pairs[i+1])
		}
		t.push(m.Map())
	case bytecode.OpTag:
		args := t.popN(in.Arg())
		name := t.pop().(value.Sym)
		t.push(&value.Tag{Name: name, Args: args})
	case bytecode.OpRange:
		end, start := t.pop(), t.pop()
		s, ok1 := start.(value.Int)
		e, ok2 := end.(value.Int)
		if !ok1 || !ok2 {
			return true, diag.Errorf("E0501", pos, "make a range from", start.TypeName(), end.TypeName())
		}
		t.push(value.Range{Start: int64(s), End: int64(e), Inclusive: in.Arg() == 1})
	case bytecode.OpIndex:
		i, v := t.pop(), t.pop()
		r, err := value.Index(v, i)
		if err != nil {
			return true, err.WithPos(pos)
		}
		t.push(r)
	case bytecode.OpSlice:
		var start, end value.Value = value.Nil{}, value.Nil{}
		if in.Arg()&2 != 0 {
			end = t.pop()
		}
		if in.Arg()&1 != 0 {
			start = t.pop()
		}
		r, err := value.Slice(t.pop(), start, end, in.Arg()&4 != 0)
		if err != nil {
			return true, err.WithPos(pos)
		}
		t.push(r)
	case bytecode.OpField:
		r, err := value.Index(t.pop(), f.proto.Consts[in.Arg()])
		if err != nil {
			return true, err.WithPos(pos)
		}
		t.push(r)
	case bytecode.OpIn:
		container, x := t.pop(), t.pop()
		found, err := value.Contains(container, x)
		if err != nil {
			return true, err.WithPos(pos)
		}
		t.push(value.Bool(found))
	case bytecode.OpUnwrapAbort:
		v := t.pop()
		payload, isOk, isErr, isNil := resultKind(v)
		switch {
		case isOk:
			t.push(payload)
		case isErr:
			return true, diag.Errorf("E0603", pos, errMessage(v))
		case isNil:
			return true, diag.Errorf("E0604", pos, "!")
		default:
			return true, diag.Errorf("E0605", pos, "!", v.TypeName())
		}
	case bytecode.OpUnwrapTest:
		v := t.stack[len(t.stack)-1]
		payload, isOk, isErr, isNil := resultKind(v)
		switch {
		case isOk:
			t.stack[len(t.stack)-1] = payload
		case isErr:
			f.ip = in.Arg() // the Err stays on the stack for RETURN
		case isNil:
			// nil is a value, not a failure: `?` only looks at Err (02).
		default:
			return true, diag.Errorf("E0605", pos, "?", v.TypeName())
		}
	case bytecode.OpCoalesce:
		payload, isOk, isErr, isNil := resultKind(t.stack[len(t.stack)-1])
		switch {
		case isOk:
			t.stack[len(t.stack)-1] = payload
		case isErr || isNil:
			t.pop()
			f.ip = in.Arg()
		}
	case bytecode.OpMatch:
		v := t.pop()
		t.push(value.Bool(t.matchPattern(f, v, f.proto.Patterns[in.Arg()])))
	case bytecode.OpMatchOrFail:
		v := t.pop()
		if !t.matchPattern(f, v, f.proto.Patterns[in.Arg()]) {
			return true, diag.Errorf("E0602", pos, value.Format(v))
		}
	case bytecode.OpNoMatch:
		return true, diag.Errorf("E0601", pos, value.Format(t.pop()))
	default:
		return false, nil
	}
	return true, nil
}
