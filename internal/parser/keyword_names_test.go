package parser

import "testing"

// Keywords are plain names where only a name can appear (01): after `.`,
// as a map literal key, and as a map pattern key. They stay keywords
// everywhere else.
func TestKeywordsAsFieldAndKeyNames(t *testing.T) {
	cases := map[string]string{
		"m.type":                     "(program (field type m))",
		"m.match.if":                 "(program (field if (field match m)))",
		"{type: 1, in: 2}":           "(program (map (entry type 1) (entry in 2)))",
		"{type: t, nil: n} = m":      "(program (assign (pmap (pentry type (pbind t)) (pentry nil (pbind n))) m))",
		"match m: {test: x} => x ;;": "(program (match m (arm (pmap (pentry test (pbind x))) (nil) (block x))))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%s\n got %s\nwant %s", src, got, want)
		}
	}
	// Still keywords as plain names.
	for _, src := range []string{"type = 1", "fn type() = 1", "{type} = m"} {
		if code := parseErrQuiet(t, src); code == "" {
			t.Errorf("%s must not parse", src)
		}
	}
}
