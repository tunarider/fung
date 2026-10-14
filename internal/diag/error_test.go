package diag

import (
	"strings"
	"testing"
)

func TestErrorfUsesCatalogMessage(t *testing.T) {
	e := Errorf("E0204", Pos{File: "a.fung", Line: 3, Col: 7}, "fiels")
	want := "error[E0204]: undefined name `fiels`"
	if e.Error() != want {
		t.Fatalf("Error() = %q, want %q", e.Error(), want)
	}
}

func TestFormatShowsPositionAndHint(t *testing.T) {
	e := Errorf("E0111", Pos{File: "a.fung", Line: 1, Col: 9})
	got := Format(e, "")
	want := "error[E0111]: comparison operators cannot be chained\n  --> a.fung:1:9\n help: split it with `and`: a < b and b < c\n"
	if got != want {
		t.Fatalf("Format() = %q, want %q", got, want)
	}
}

func TestFormatWithoutHintOrPosition(t *testing.T) {
	e := Errorf("E0502", Pos{})
	got := Format(e, "")
	want := "error[E0502]: division by zero\n"
	if got != want {
		t.Fatalf("Format() = %q, want %q", got, want)
	}
}

func TestUnknownCodeStillFormats(t *testing.T) {
	e := Errorf("E9999", Pos{}, "x")
	if got := e.Error(); got != "error[E9999]: unknown error E9999 [x]" {
		t.Fatalf("Error() = %q", got)
	}
}

func TestWithPosFillsOnlyEmptyPosition(t *testing.T) {
	e := Errorf("E0502", Pos{})
	p := Pos{File: "a.fung", Line: 2, Col: 1}
	if got := e.WithPos(p).Pos; got != p {
		t.Fatalf("WithPos on empty = %v, want %v", got, p)
	}
	q := Pos{File: "b.fung", Line: 9, Col: 9}
	if got := e.WithPos(q).Pos; got != p {
		t.Fatalf("WithPos on filled = %v, want it kept %v", got, p)
	}
}

func TestIsCompileError(t *testing.T) {
	cases := map[string]bool{"E0101": true, "E0204": true, "E0301": true, "E0401": false, "E0502": false, "E0801": false}
	for code, want := range cases {
		if got := IsCompileError(code); got != want {
			t.Errorf("IsCompileError(%s) = %v, want %v", code, got, want)
		}
	}
}

func TestPosString(t *testing.T) {
	if got := (Pos{File: "a.fung", Line: 3, Col: 7}).String(); got != "a.fung:3:7" {
		t.Fatalf("String() = %q", got)
	}
}

func TestFormatWithSourceExcerpt(t *testing.T) {
	src := "x = 1\n  for f in fiels:\n"
	e := Errorf("E0204", Pos{File: "rename.fung", Line: 2, Col: 12}, "fiels").Span(5).WithHelp("a similar name exists: `files`")
	want := "error[E0204]: undefined name `fiels`\n" +
		"  --> rename.fung:2:12\n" +
		"  |\n" +
		"2 |   for f in fiels:\n" +
		"  |            ^^^^^ not found in this scope\n" +
		"  |\n" +
		" help: a similar name exists: `files`\n"
	if got := Format(e, src); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}

func TestFormatExcerptExpandsTabsAndWidensGutter(t *testing.T) {
	src := strings.Repeat("\n", 9) + "\tprint(1 / 0)\n"
	e := Errorf("E0502", Pos{File: "a.fung", Line: 10, Col: 10})
	want := "error[E0502]: division by zero\n" +
		"  --> a.fung:10:10\n" +
		"   |\n" +
		"10 |     print(1 / 0)\n" +
		"   |             ^\n" +
		"   |\n"
	if got := Format(e, src); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}

func TestFormatWithoutSourceOrOutOfRangeLineHasNoExcerpt(t *testing.T) {
	e := Errorf("E0502", Pos{File: "a.fung", Line: 3, Col: 1})
	if got := Format(e, ""); got != "error[E0502]: division by zero\n  --> a.fung:3:1\n" {
		t.Fatalf("got %q", got)
	}
	if got := Format(e, "one line\n"); got != "error[E0502]: division by zero\n  --> a.fung:3:1\n" {
		t.Fatalf("got %q", got)
	}
}

func TestHelpOverridesCatalogHint(t *testing.T) {
	e := Errorf("E0111", Pos{}).WithHelp("custom")
	if got := Format(e, ""); got != "error[E0111]: comparison operators cannot be chained\n help: custom\n" {
		t.Fatalf("got %q", got)
	}
}

func TestRetiredCodeStaysOutOfTheCatalog(t *testing.T) {
	// E0207 was retired with scope model C (2026-10-13); its number is never reused.
	if !strings.HasPrefix(Message("E0207"), "unknown error") {
		t.Fatal("E0207 must stay retired")
	}
}

func TestNewCodesExist(t *testing.T) {
	for _, code := range []string{"E0112", "E0205", "E0206", "E0803"} {
		if strings.HasPrefix(Message(code, "a", "b"), "unknown error") {
			t.Errorf("%s missing from catalog", code)
		}
	}
	if Hint("E0205") == "" || Hint("E0206") == "" || Label("E0204") == "" {
		t.Error("expected hints for E0205/E0206 and a label for E0204")
	}
}

func TestFormatCallPath(t *testing.T) {
	e := Errorf("E0501", Pos{File: "r.fung", Line: 7, Col: 11}, "add", "int", "str").WithCallPath([]CallSite{
		{File: "r.fung", Line: 7, Fn: "format_size"},
		{File: "r.fung", Line: 15, Fn: "report", Elided: 3},
		{File: "r.fung", Line: 22, Fn: "<main>"},
	})
	want := "error[E0501]: cannot add int and str\n" +
		"  --> r.fung:7:11\n" +
		" call path (innermost first):\n" +
		"   r.fung:7   format_size\n" +
		"   r.fung:15  report (3 tail calls elided)\n" +
		"   r.fung:22  <entry>\n"
	if got := Format(e, ""); got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}

func TestFormatCallPathIsTruncated(t *testing.T) {
	var path []CallSite
	for i := 0; i < 30; i++ {
		path = append(path, CallSite{File: "a.fung", Line: i + 1, Fn: "f"})
	}
	got := Format(Errorf("E0804", Pos{File: "a.fung", Line: 1, Col: 1}, 100000).WithCallPath(path), "")
	if !strings.Contains(got, "   … 20 more\n") || strings.Count(got, "\n   a.fung") != 10 {
		t.Fatalf("got:\n%s", got)
	}
}

func TestWithCallPathKeepsExisting(t *testing.T) {
	e := Errorf("E0502", Pos{}).WithCallPath([]CallSite{{Fn: "a"}})
	e.WithCallPath([]CallSite{{Fn: "b"}})
	if len(e.CallPath) != 1 || e.CallPath[0].Fn != "a" {
		t.Fatal("call path was overwritten")
	}
}

func TestM3CodesExist(t *testing.T) {
	for _, code := range []string{"E0208", "E0209", "E0210", "E0211", "E0212", "E0402", "E0403", "E0404", "E0405", "E0406", "E0407", "E0804", "E0805"} {
		if strings.HasPrefix(Message(code, "a", "b", "c"), "unknown error") {
			t.Errorf("%s missing", code)
		}
	}
}

func TestExitSignal(t *testing.T) {
	e := ExitSignal(3)
	if code, ok := e.ExitCode(); !ok || code != 3 {
		t.Fatalf("ExitCode = %d %v", code, ok)
	}
	if e.Error() != "exit(3)" {
		t.Fatalf("Error = %q", e.Error())
	}
	if _, ok := Errorf("E0801", Pos{}, "x").ExitCode(); ok {
		t.Fatal("ordinary errors are not exits")
	}
	for _, code := range []string{"E0113", "E0221", "E0301", "E0302", "E0303", "E0304", "E0305", "E0306", "E0307", "E0308", "E0309", "E0310", "E0311", "E0513", "E0514"} {
		if strings.HasPrefix(Message(code, "a", "b"), "unknown error") {
			t.Errorf("%s missing from the catalog", code)
		}
	}
}

func TestWarningsAndNotes(t *testing.T) {
	w := Errorf("W0101", Pos{File: "t.fung", Line: 1, Col: 1}, "fs")
	if w.Error() != "warning[W0101]: unused import `fs`" {
		t.Fatalf("Error = %q", w.Error())
	}
	e := Errorf("E0807", Pos{File: "t.fung", Line: 2, Col: 8}, "f(x) == 3")
	e.Notes = []string{"f(x) = 4"}
	out := Format(e, "x = 1\nassert f(x) == 3\n")
	if !strings.Contains(out, "error[E0807]: assertion failed: f(x) == 3") || !strings.Contains(out, "\n  f(x) = 4\n") {
		t.Fatalf("Format = %q", out)
	}
	for _, code := range []string{"E0114", "E0807", "W0101", "W0102", "W0103"} {
		if strings.HasPrefix(Message(code, "a"), "unknown error") {
			t.Errorf("%s missing", code)
		}
	}
}

func TestConcurrencyCodes(t *testing.T) {
	for _, code := range []string{"E0701", "E0702", "E0703"} {
		if strings.HasPrefix(Message(code), "unknown error") {
			t.Errorf("%s missing", code)
		}
	}
}
