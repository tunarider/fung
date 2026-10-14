package value

import (
	"fmt"
	"strconv"
	"strings"
	"unicode"
)

// Format renders a value in fung literal form (08: 출력을 그대로 코드에
// 붙여도 동작한다). Floats always show a decimal point or exponent so
// they read back as floats.
func Format(v Value) string {
	switch x := v.(type) {
	case Int:
		return strconv.FormatInt(int64(x), 10)
	case Float:
		return formatFloat(float64(x))
	case Bool:
		if x {
			return "true"
		}
		return "false"
	case Nil:
		return "nil"
	case Str:
		return quoteStr(string(x))
	case Sym:
		return "'" + x.Name()
	case Dur:
		return formatDur(x)
	case Time:
		return x.ISO()
	case *List:
		return "[" + formatItems(x.ToSlice()) + "]"
	case *Tuple:
		if len(x.Items) == 1 {
			return "(" + Format(x.Items[0]) + ",)"
		}
		return "(" + formatItems(x.Items) + ")"
	case *Map:
		parts := make([]string, 0, x.Len())
		for k, v := range x.All() {
			key := k
			if !isPlainKey(k) {
				key = quoteStr(k)
			}
			parts = append(parts, key+": "+Format(v))
		}
		return "{" + strings.Join(parts, ", ") + "}"
	case *Tag:
		return x.Name.Name() + "(" + formatItems(x.Args) + ")"
	case Range:
		op := ".."
		if x.Inclusive {
			op = "..="
		}
		return strconv.FormatInt(x.Start, 10) + op + strconv.FormatInt(x.End, 10)
	case Fn:
		if name := x.FnName(); name != "" {
			return "<fn " + name + ">"
		}
		return "<fn>"
	case Opaque:
		return x.Display()
	case *Module:
		return "<module " + x.Name + ">"
	case *File:
		return "<file " + strconv.Quote(x.Path) + ">"
	}
	return "<unknown>"
}

// FormatForPrint is what print() and ${} use: strings bare, the rest in
// literal form.
func FormatForPrint(v Value) string {
	if s, ok := v.(Str); ok {
		return string(s)
	}
	return Format(v)
}

// quoteStr writes a string as a fung literal using only the escapes 01
// allows. `$` is escaped only where it would start an interpolation.
func quoteStr(s string) string {
	var b strings.Builder
	b.WriteByte('"')
	rs := []rune(s)
	for i, r := range rs {
		switch {
		case r == '\n':
			b.WriteString(`\n`)
		case r == '\t':
			b.WriteString(`\t`)
		case r == '\r':
			b.WriteString(`\r`)
		case r == '\\':
			b.WriteString(`\\`)
		case r == '"':
			b.WriteString(`\"`)
		case r == '$' && i+1 < len(rs) && rs[i+1] == '{':
			b.WriteString(`\$`)
		case !unicode.IsPrint(r) && r != ' ':
			fmt.Fprintf(&b, `\u{%X}`, r)
		default:
			b.WriteRune(r)
		}
	}
	b.WriteByte('"')
	return b.String()
}

// formatFloat uses plain notation for exponents in [-4, 21) and exponent
// notation outside, like JavaScript, so 1000000.0 prints as itself and
// 1e21 as 1e21. Go's 'g' format would switch to exponents at 1e6.
func formatFloat(f float64) string {
	e := strconv.FormatFloat(f, 'e', -1, 64) // d.ddde±XX
	exp, _ := strconv.Atoi(e[strings.IndexByte(e, 'e')+1:])
	if exp < -4 || exp >= 21 {
		return strings.Replace(e, "e+", "e", 1)
	}
	s := strconv.FormatFloat(f, 'f', -1, 64)
	if !strings.Contains(s, ".") {
		s += ".0"
	}
	return s
}
