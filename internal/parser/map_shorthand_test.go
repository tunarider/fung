package parser

import "testing"

// {a, b} is short for {a: a, b: b} in literals and in patterns (02).
// Keywords get no shorthand: there is no variable they could name.
func TestMapShorthand(t *testing.T) {
	cases := map[string]string{
		"{a, b: 2, c}":                     "(program (map (entry a a) (entry b 2) (entry c c)))",
		"{DB_HOST}":                        "(program (map (entry DB_HOST DB_HOST)))",
		"{line, col} = e":                  "(program (assign (pmap (pentry line (pbind line)) (pentry col (pbind col))) e))",
		"match e: {kind, n: 1} => kind ;;": "(program (match e (arm (pmap (pentry kind (pbind kind)) (pentry n (plit 1))) (nil) (block kind))))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%s\n got %s\nwant %s", src, got, want)
		}
	}
	for _, src := range []string{"{type}", "{\"a\"}", "{type} = m", "{a, a}"} {
		if code := parseErrQuiet(t, src); code == "" {
			t.Errorf("%s must not parse", src)
		}
	}
}
