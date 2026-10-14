package native

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strconv"
	"strings"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/nat"
	"github.com/tunarider/fung/internal/value"
)

// jsonModule builds _native.json (06): a token-level decoder that keeps
// object key order, and an encoder over fung values. It lives in Go
// because parsing walks every character (06: 성능이 이유면 native).
func jsonModule() *value.Module {
	m := value.NewModule("json", "_native.json")
	add := func(n *value.Native) { m.Set(n.Name, n) }
	add(nat.Def("parse", jsonParseFn, nat.Param("s")))
	add(nat.Def("stringify", jsonStringifyFn, nat.Param("v"), nat.Opt("indent", value.Nil{})))
	return m
}

// jsonError is a parse failure with a 1-based position.
type jsonError struct {
	line, col int
	msg       string
}

func (e *jsonError) Error() string { return e.msg }

// lineCol converts a byte offset into a 1-based line and column.
func lineCol(s string, off int64) (int, int) {
	if off > int64(len(s)) {
		off = int64(len(s))
	}
	before := s[:off]
	line := strings.Count(before, "\n") + 1
	col := len([]rune(before[strings.LastIndex(before, "\n")+1:])) + 1
	return line, col
}

func jsonParseFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.NeedStr("s", args[0])
	if err != nil {
		return nil, err
	}
	v, perr := decodeJSON(s)
	if perr != nil {
		je := perr.(*jsonError)
		return nat.ErrValue("parse", fmt.Sprintf("invalid JSON at line %d, column %d: %s", je.line, je.col, je.msg),
			value.Str("line"), value.Int(je.line), value.Str("col"), value.Int(je.col)), nil
	}
	return nat.Ok(v), nil
}

// decodeJSON reads one JSON value and insists nothing follows it.
func decodeJSON(s string) (value.Value, error) {
	dec := json.NewDecoder(strings.NewReader(s))
	dec.UseNumber()
	v, err := readValue(dec, s)
	if err != nil {
		return nil, err
	}
	rest := skipSpace(s, dec.InputOffset())
	if _, err := dec.Token(); err != io.EOF {
		return nil, jsonErr(s, rest, err, "unexpected text after the value")
	}
	return v, nil
}

// skipSpace moves off past JSON whitespace so a position names the
// first character of the value, not the blank before it.
func skipSpace(s string, off int64) int64 {
	for off < int64(len(s)) && (s[off] == ' ' || s[off] == '\t' || s[off] == '\n' || s[off] == '\r') {
		off++
	}
	return off
}

// jsonErr positions a decoder error: a syntax error carries the offset
// just past the offending character, anything else uses the offset
// handed in (where the value started).
func jsonErr(s string, off int64, err error, fallback string) error {
	msg := fallback
	var se *json.SyntaxError
	if errors.As(err, &se) {
		off, msg = se.Offset, se.Error()
		if off > 0 && !strings.HasPrefix(msg, "unexpected end") {
			off-- // name the character itself, not the one after it
		}
	} else if err != nil && err != io.EOF {
		msg = err.Error()
	}
	line, col := lineCol(s, off)
	return &jsonError{line: line, col: col, msg: msg}
}

func readValue(dec *json.Decoder, s string) (value.Value, error) {
	start := skipSpace(s, dec.InputOffset())
	tok, err := dec.Token()
	if err != nil {
		if err == io.EOF {
			return nil, jsonErr(s, start, nil, "unexpected end of input")
		}
		return nil, jsonErr(s, start, err, "invalid token")
	}
	switch x := tok.(type) {
	case json.Delim:
		switch x {
		case '{':
			m := value.NewMapBuilder()
			for dec.More() {
				kt, err := dec.Token()
				if err != nil {
					return nil, jsonErr(s, dec.InputOffset(), err, "invalid object key")
				}
				k, ok := kt.(string)
				if !ok {
					return nil, jsonErr(s, dec.InputOffset(), nil, "object keys must be strings")
				}
				v, err := readValue(dec, s)
				if err != nil {
					return nil, err
				}
				m.Set(k, v)
			}
			if _, err := dec.Token(); err != nil { // the closing }
				return nil, jsonErr(s, dec.InputOffset(), err, "unterminated object")
			}
			return m.Map(), nil
		case '[':
			items := []value.Value{}
			for dec.More() {
				v, err := readValue(dec, s)
				if err != nil {
					return nil, err
				}
				items = append(items, v)
			}
			if _, err := dec.Token(); err != nil { // the closing ]
				return nil, jsonErr(s, dec.InputOffset(), err, "unterminated array")
			}
			return value.NewList(items), nil
		}
		return nil, jsonErr(s, start, nil, "unexpected "+string(x))
	case string:
		return value.Str(x), nil
	case bool:
		return value.Bool(x), nil
	case nil:
		return value.Nil{}, nil
	case json.Number:
		return numberValue(string(x), s, start)
	}
	return nil, jsonErr(s, start, nil, "unexpected token")
}

// numberValue keeps integers exact: a literal without `.`, `e` or `E`
// is an int and must fit int64 (06: 조용히 float로 바꾸지 않는다).
func numberValue(lit, s string, off int64) (value.Value, error) {
	if !strings.ContainsAny(lit, ".eE") {
		n, err := strconv.ParseInt(lit, 10, 64)
		if err != nil {
			return nil, jsonErr(s, off, nil, "integer "+lit+" is out of range")
		}
		return value.Int(n), nil
	}
	f, err := strconv.ParseFloat(lit, 64)
	if err != nil {
		return nil, jsonErr(s, off, nil, "number "+lit+" is out of range")
	}
	return value.Float(f), nil
}

func jsonStringifyFn(_ value.Context, args []value.Value) (value.Value, error) {
	indent := 0
	switch x := args[1].(type) {
	case value.Nil:
	case value.Int:
		if x < 0 {
			return nil, diag.Errorf("E0403", diag.Pos{}, "indent", "a non-negative int", value.Format(x))
		}
		indent = int(x)
	default:
		return nil, diag.Errorf("E0403", diag.Pos{}, "indent", "int | nil", args[1].TypeName())
	}
	var b bytes.Buffer
	if err := encodeJSON(&b, args[0], indent, 0); err != nil {
		return nil, err
	}
	return value.Str(b.String()), nil
}

// quoteJSON escapes a string the JSON way without HTML escaping.
func quoteJSON(s string) string {
	var b bytes.Buffer
	enc := json.NewEncoder(&b)
	enc.SetEscapeHTML(false)
	enc.Encode(s) // cannot fail for a string
	return strings.TrimSuffix(b.String(), "\n")
}

// encodeJSON writes v (06: stringify 대응표). Maps keep their order.
// indent 0 is one line; otherwise nested values go on their own lines.
func encodeJSON(b *bytes.Buffer, v value.Value, indent, depth int) error {
	newline := func(d int) {
		if indent > 0 {
			b.WriteByte('\n')
			b.WriteString(strings.Repeat(" ", indent*d))
		}
	}
	colon := ":"
	if indent > 0 {
		colon = ": "
	}
	switch x := v.(type) {
	case value.Nil:
		b.WriteString("null")
	case value.Bool, value.Int, value.Float:
		b.WriteString(value.Format(x))
	case value.Str:
		b.WriteString(quoteJSON(string(x)))
	case value.Sym:
		b.WriteString(quoteJSON(x.Name()))
	case value.Time:
		b.WriteString(quoteJSON(x.ISO()))
	case *value.List:
		return encodeItems(b, x.ToSlice(), indent, depth, newline)
	case *value.Tuple:
		return encodeItems(b, x.Items, indent, depth, newline)
	case *value.Map:
		if x.Len() == 0 {
			b.WriteString("{}")
			return nil
		}
		b.WriteByte('{')
		for i, k := range x.Keys() {
			if i > 0 {
				b.WriteByte(',')
			}
			newline(depth + 1)
			b.WriteString(quoteJSON(k))
			b.WriteString(colon)
			val, _ := x.Get(k)
			if err := encodeJSON(b, val, indent, depth+1); err != nil {
				return err
			}
		}
		newline(depth)
		b.WriteByte('}')
	default:
		return diag.Errorf("E0513", diag.Pos{}, v.TypeName(), "JSON")
	}
	return nil
}

func encodeItems(b *bytes.Buffer, items []value.Value, indent, depth int, newline func(int)) error {
	if len(items) == 0 {
		b.WriteString("[]")
		return nil
	}
	b.WriteByte('[')
	for i, it := range items {
		if i > 0 {
			b.WriteByte(',')
		}
		newline(depth + 1)
		if err := encodeJSON(b, it, indent, depth+1); err != nil {
			return err
		}
	}
	newline(depth)
	b.WriteByte(']')
	return nil
}
