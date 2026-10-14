package prelude

import (
	"github.com/tunarider/fung/internal/nat"
	"strings"
	"unicode"

	"github.com/tunarider/fung/internal/diag"
	"github.com/tunarider/fung/internal/value"
)

// stringFns registers the prelude's string functions (06).
func stringFns(add func(*value.Native)) {
	add(nat.Def("split", splitFn, nat.Param("s"), nat.Param("sep")))
	add(nat.Def("join", joinFn, nat.Param("xs"), nat.Opt("sep", value.Str(""))))
	add(nat.Def("trim", trimFn, nat.Param("s"), nat.Opt("cut", value.Nil{})))
	add(nat.Def("upper", upperFn, nat.Param("s")))
	add(nat.Def("lower", lowerFn, nat.Param("s")))
	add(nat.Def("replace", replaceFn, nat.Param("s"), nat.Param("old"), nat.Param("new")))
	add(nat.Def("starts_with", startsWithFn, nat.Param("s"), nat.Param("prefix")))
	add(nat.Def("ends_with", endsWithFn, nat.Param("s"), nat.Param("suffix")))
	add(nat.Def("repeat", repeatFn, nat.Param("s"), nat.Param("n")))
}

func splitFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.StrArgs(args, "s", "sep")
	if err != nil {
		return nil, err
	}
	parts := strings.Split(s[0], s[1])
	out := make([]value.Value, len(parts))
	for i, p := range parts {
		out[i] = value.Str(p)
	}
	return nat.NewList(out), nil
}

// joinFn renders elements as print would (06): strings bare, the rest in
// literal form.
func joinFn(ctx value.Context, args []value.Value) (value.Value, error) {
	xs, err := nat.Items(ctx, args[0], "join")
	if err != nil {
		return nil, err
	}
	sep, err := nat.NeedStr("sep", args[1])
	if err != nil {
		return nil, err
	}
	parts := make([]string, len(xs))
	for i, x := range xs {
		parts[i] = value.FormatForPrint(x)
	}
	return value.Str(strings.Join(parts, sep)), nil
}

func trimFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.NeedStr("s", args[0])
	if err != nil {
		return nil, err
	}
	if _, isNil := args[1].(value.Nil); isNil {
		return value.Str(strings.TrimSpace(s)), nil
	}
	cut, err := nat.NeedStr("cut", args[1])
	if err != nil {
		return nil, err
	}
	return value.Str(strings.Trim(s, cut)), nil
}

func upperFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.NeedStr("s", args[0])
	if err != nil {
		return nil, err
	}
	return value.Str(strings.ToUpper(s)), nil
}

func lowerFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.NeedStr("s", args[0])
	if err != nil {
		return nil, err
	}
	return value.Str(strings.ToLower(s)), nil
}

func replaceFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.StrArgs(args, "s", "old", "new")
	if err != nil {
		return nil, err
	}
	return value.Str(strings.ReplaceAll(s[0], s[1], s[2])), nil
}

func startsWithFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.StrArgs(args, "s", "prefix")
	if err != nil {
		return nil, err
	}
	return value.Bool(strings.HasPrefix(s[0], s[1])), nil
}

func endsWithFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.StrArgs(args, "s", "suffix")
	if err != nil {
		return nil, err
	}
	return value.Bool(strings.HasSuffix(s[0], s[1])), nil
}

// maxChars bounds the strings repeat and pad_* may build (E0806).
const maxChars = 100_000_000

func repeatFn(_ value.Context, args []value.Value) (value.Value, error) {
	s, err := nat.NeedStr("s", args[0])
	if err != nil {
		return nil, err
	}
	n, err := nat.NeedInt("n", args[1])
	if err != nil {
		return nil, err
	}
	if n < 0 {
		n = 0
	}
	if n > 0 && len([]rune(s))*n > maxChars {
		return nil, diag.Errorf("E0806", diag.Pos{}, "characters", maxChars)
	}
	return value.Str(strings.Repeat(s, n)), nil
}

// isSpaceFree reports whether s has no leading or trailing whitespace
// (conversions are strict: `int(" 1")` is an error).
func isSpaceFree(s string) bool {
	return s != "" && !unicode.IsSpace([]rune(s)[0]) && !unicode.IsSpace([]rune(s)[len([]rune(s))-1])
}
