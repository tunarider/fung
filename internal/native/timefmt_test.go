package native

import (
	"testing"
	"time"
)

var kst = time.FixedZone("KST", 9*3600)

func TestFormatTimeTokens(t *testing.T) {
	at := time.Date(2026, 10, 4, 9, 5, 7, 123_000_000, kst)
	cases := map[string]string{
		"YYYY-MM-DD HH:mm:ss":  "2026-10-04 09:05:07",
		"YYYY-MM-DDTHH:mm:ssZ": "2026-10-04T09:05:07+09:00",
		"YY/MM/DD":             "26/10/04",
		"HH:mm:ss.SSS ZZ":      "09:05:07.123 +0900",
		"DD.MM.YYYY":           "04.10.2026",
		"literal text":         "literal text", // only the tokens change
		"YYYYMMDD":             "20261004",
		"ddd, DD MMM YYYY":     "Sun, 04 Oct 2026",
		"MMM MM":               "Oct 10",
	}
	for layout, want := range cases {
		if got := formatTime(at, layout); got != want {
			t.Errorf("formatTime(%q) = %q, want %q", layout, got, want)
		}
	}
}

func TestParseTimeTokens(t *testing.T) {
	ok := map[[2]string]string{
		{"2026-10-04 09:05:07", "YYYY-MM-DD HH:mm:ss"}:            "2026-10-04T09:05:07", // local zone: compared without zone
		{"2026-10-04T09:05:07+09:00", "YYYY-MM-DDTHH:mm:ssZ"}:     "2026-10-04T09:05:07+09:00",
		{"2026-10-04T09:05:07-0130", "YYYY-MM-DDTHH:mm:ssZZ"}:     "2026-10-04T09:05:07-01:30",
		{"26/10/04 09:05:07.250", "YY/MM/DD HH:mm:ss.SSS"}:        "2026-10-04T09:05:07.25",
		{"20261004", "YYYYMMDD"}:                                  "2026-10-04T00:00:00",
		{"Sun, 04 oct 2026", "ddd, DD MMM YYYY"}:                  "2026-10-04T00:00:00",       // names match any case
		{"04/Dec/2026:23:59:59 +0900", "DD/MMM/YYYY:HH:mm:ss ZZ"}: "2026-12-04T23:59:59+09:00", // access-log style
	}
	for in, want := range ok {
		got, err := parseTime(in[0], in[1])
		if err != nil {
			t.Errorf("parseTime(%q, %q): %v", in[0], in[1], err)
			continue
		}
		layout := "2006-01-02T15:04:05.999999999"
		if len(want) > 19 && (want[len(want)-6] == '+' || want[len(want)-6] == '-') {
			layout += "-07:00"
		}
		if s := got.Format(layout); s != want {
			t.Errorf("parseTime(%q, %q) = %s, want %s", in[0], in[1], s, want)
		}
	}
	bad := [][2]string{
		{"2026-13-01", "YYYY-MM-DD"},       // month 13: not normalized away
		{"2026-02-30", "YYYY-MM-DD"},       // Feb 30
		{"2026-1-5", "YYYY-MM-DD"},         // digits missing
		{"2026-10-04x", "YYYY-MM-DD"},      // trailing garbage
		{"2026-10-04", "YYYY-MM-DD HH:mm"}, // input too short
		{"09:60", "HH:mm"},                 // minute 60
		{"2026-10-04T09:05:07+9:00", "YYYY-MM-DDTHH:mm:ssZ"},
		{"Foo 04", "MMM DD"},   // unknown month name
		{"Xyz, 04", "ddd, DD"}, // unknown weekday name (a known one is accepted but not checked against the date)
	}
	for _, in := range bad {
		if _, err := parseTime(in[0], in[1]); err == nil {
			t.Errorf("parseTime(%q, %q) must fail", in[0], in[1])
		}
	}
}
