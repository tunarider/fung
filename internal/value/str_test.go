package value

import "testing"

func TestStrFormatQuotesAndEscapes(t *testing.T) {
	cases := map[Str]string{
		"a b":        `"a b"`,
		"tab\there":  `"tab\there"`,
		"nl\n":       `"nl\n"`,
		`q"q`:        `"q\"q"`,
		`back\slash`: `"back\\slash"`,
		"cost $5":    `"cost $5"`,
		"tmpl ${x}":  `"tmpl \${x}"`,
		"참치":         `"참치"`,
		"bell\x07":   `"bell\u{7}"`,
	}
	for s, want := range cases {
		if got := Format(s); got != want {
			t.Errorf("Format(%q) = %s, want %s", string(s), got, want)
		}
	}
	if got := FormatForPrint(Str("a \"b\"")); got != `a "b"` {
		t.Errorf("FormatForPrint = %q", got)
	}
	if Str("x").TypeName() != "str" {
		t.Error("TypeName")
	}
}

func TestStrConcatAndErrors(t *testing.T) {
	ok(t, Add, Str("a"), Str("b"), Str("ab"))
	fails(t, Add, Str("a"), Int(1), "E0501")
	fails(t, Sub, Str("a"), Str("b"), "E0501")
	fails(t, Mul, Str("a"), Int(3), "E0501")
}

func TestStrEqualAndCompare(t *testing.T) {
	if !Equal(Str("a"), Str("a")) || Equal(Str("a"), Str("b")) || Equal(Str("1"), Int(1)) {
		t.Error("Equal")
	}
	if c, err := Compare(Str("abc"), Str("abd")); err != nil || c != -1 {
		t.Errorf("Compare abc abd = %d %v", c, err)
	}
	if c, _ := Compare(Str("가"), Str("나")); c != -1 {
		t.Error("Hangul order")
	}
	if _, err := Compare(Str("a"), Int(1)); err == nil || err.Code != "E0501" {
		t.Error("str vs int")
	}
}
