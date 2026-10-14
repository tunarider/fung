package parser

import "testing"

func TestImportStatement(t *testing.T) {
	cases := map[string]string{
		"import fs":                                "(program (import fs))",
		"import \"./lib/rename\"":                  `(program (import "./lib/rename"))`,
		"import \"./lib/rename\" as rn":            `(program (import "./lib/rename" as rn))`,
		"import \"./lib/rename\" (slugify, kebab)": `(program (import "./lib/rename" (slugify, kebab)))`,
		"import \"./x\" as y (a)":                  `(program (import "./x" as y (a)))`,
		"import fs\nprint(fs)":                     "(program (import fs) (call print fs))",
		"import _native.fs as _n":                  "(program (import _native.fs as _n))",
	}
	for src, want := range cases {
		if got := parse(t, src); got != want {
			t.Errorf("%q:\n got %s\nwant %s", src, got, want)
		}
	}
	errs := map[string]string{
		"import":             "E0112",
		"import 1":           "E0112",
		"import fs as":       "E0112",
		"import fs ()":       "E0112",
		"import fs (a,)":     "E0112",
		"import \"x\" as Fs": "E0112", // alias must be a name, not a tag
		"import \"${x}\"":    "E0112", // no interpolation in a module path
	}
	for src, want := range errs {
		if got := parseErr(t, src); got != want {
			t.Errorf("%q: %s, want %s", src, got, want)
		}
	}
}

func TestMapLiteralRejectsDuplicateKeys(t *testing.T) {
	if got := parseErr(t, "{a: 1, a: 2}"); got != "E0113" {
		t.Errorf("dup key: %s", got)
	}
	if got := parseErr(t, "{a: 1, \"a\": 2}"); got != "E0113" {
		t.Errorf("dup quoted key: %s", got)
	}
	if got := parse(t, "{a: 1, b: 1}"); got != "(program (map (entry a 1) (entry b 1)))" {
		t.Errorf("distinct keys: %s", got)
	}
}

func TestUnderscoreAssignmentDiscards(t *testing.T) {
	if got := parse(t, "_ = f()"); got != "(program (assign (pwild) (call f)))" {
		t.Errorf("got %s", got)
	}
}
