package native

import (
	"fmt"
	"strconv"
	"strings"
	"time"
)

// tokens are the time format tokens (06: 시간 형식), longest first so
// YYYY wins over YY and ZZ over Z. Characters outside the table are
// literal: they print as they are and must match on parse.
var tokens = []string{"YYYY", "MMM", "SSS", "ddd", "YY", "MM", "DD", "HH", "mm", "ss", "ZZ", "Z"}

var (
	monthNames   = [...]string{"Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"}
	weekdayNames = [...]string{"Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"}
)

// name reads a three-letter English name from s, matching any case, and
// returns its index in names.
func name(s string, names []string, what string) (int, string, error) {
	if len(s) < 3 {
		return 0, "", fmt.Errorf("expected a %s name", what)
	}
	for i, n := range names {
		if strings.EqualFold(s[:3], n) {
			return i, s[3:], nil
		}
	}
	return 0, "", fmt.Errorf("unknown %s name %q", what, s[:3])
}

// nextToken returns the token at the start of layout, or "" for a
// literal character.
func nextToken(layout string) string {
	for _, tk := range tokens {
		if strings.HasPrefix(layout, tk) {
			return tk
		}
	}
	return ""
}

func pad(n, width int) string {
	s := strconv.Itoa(n)
	for len(s) < width {
		s = "0" + s
	}
	return s
}

func zone(t time.Time, colon bool) string {
	_, off := t.Zone()
	sign := "+"
	if off < 0 {
		sign, off = "-", -off
	}
	h, m := off/3600, (off%3600)/60
	if colon {
		return sign + pad(h, 2) + ":" + pad(m, 2)
	}
	return sign + pad(h, 2) + pad(m, 2)
}

// formatTime renders t with the token layout.
func formatTime(t time.Time, layout string) string {
	var b strings.Builder
	for layout != "" {
		tk := nextToken(layout)
		switch tk {
		case "YYYY":
			b.WriteString(pad(t.Year(), 4))
		case "YY":
			b.WriteString(pad(t.Year()%100, 2))
		case "MM":
			b.WriteString(pad(int(t.Month()), 2))
		case "MMM":
			b.WriteString(monthNames[t.Month()-1])
		case "ddd":
			b.WriteString(weekdayNames[t.Weekday()])
		case "DD":
			b.WriteString(pad(t.Day(), 2))
		case "HH":
			b.WriteString(pad(t.Hour(), 2))
		case "mm":
			b.WriteString(pad(t.Minute(), 2))
		case "ss":
			b.WriteString(pad(t.Second(), 2))
		case "SSS":
			b.WriteString(pad(t.Nanosecond()/1_000_000, 3))
		case "Z":
			b.WriteString(zone(t, true))
		case "ZZ":
			b.WriteString(zone(t, false))
		default:
			r := []rune(layout)[0]
			b.WriteRune(r)
			layout = layout[len(string(r)):]
			continue
		}
		layout = layout[len(tk):]
	}
	return b.String()
}

// digits reads exactly n ASCII digits from the front of s.
func digits(s string, n int, what string) (int, string, error) {
	if len(s) < n {
		return 0, "", fmt.Errorf("expected %d digits for %s", n, what)
	}
	for i := 0; i < n; i++ {
		if s[i] < '0' || s[i] > '9' {
			return 0, "", fmt.Errorf("expected %d digits for %s", n, what)
		}
	}
	v, _ := strconv.Atoi(s[:n])
	return v, s[n:], nil
}

// parseZone reads +HH:MM (colon) or +HHMM and returns the offset in seconds.
func parseZone(s string, colon bool) (int, string, error) {
	if s == "" || (s[0] != '+' && s[0] != '-') {
		return 0, "", fmt.Errorf("expected a time zone offset")
	}
	sign := 1
	if s[0] == '-' {
		sign = -1
	}
	h, rest, err := digits(s[1:], 2, "the zone hours")
	if err != nil {
		return 0, "", err
	}
	if colon {
		if !strings.HasPrefix(rest, ":") {
			return 0, "", fmt.Errorf("expected `:` in the time zone offset")
		}
		rest = rest[1:]
	}
	m, rest, err := digits(rest, 2, "the zone minutes")
	if err != nil {
		return 0, "", err
	}
	return sign * (h*3600 + m*60), rest, nil
}

// parseTime reads s with the token layout. Fields the layout does not
// mention are zero; without a zone token the time is local. Out-of-range
// fields (month 13, Feb 30) are errors: time.Date would normalize them
// silently.
func parseTime(s, layout string) (time.Time, error) {
	year, month, day, hour, min, sec, ms := 0, 1, 1, 0, 0, 0, 0
	loc := time.Local
	var err error
	for layout != "" {
		tk := nextToken(layout)
		switch tk {
		case "YYYY":
			year, s, err = digits(s, 4, "the year")
		case "YY":
			year, s, err = digits(s, 2, "the year")
			year += 2000
		case "MM":
			month, s, err = digits(s, 2, "the month")
		case "MMM":
			var i int
			i, s, err = name(s, monthNames[:], "month")
			month = i + 1
		case "ddd":
			// The weekday is read and checked as a name only: the date
			// decides the real weekday.
			_, s, err = name(s, weekdayNames[:], "weekday")
		case "DD":
			day, s, err = digits(s, 2, "the day")
		case "HH":
			hour, s, err = digits(s, 2, "the hour")
		case "mm":
			min, s, err = digits(s, 2, "the minute")
		case "ss":
			sec, s, err = digits(s, 2, "the second")
		case "SSS":
			ms, s, err = digits(s, 3, "the milliseconds")
		case "Z", "ZZ":
			var off int
			off, s, err = parseZone(s, tk == "Z")
			loc = time.FixedZone("", off)
		default:
			r := []rune(layout)[0]
			if !strings.HasPrefix(s, string(r)) {
				return time.Time{}, fmt.Errorf("expected %q", string(r))
			}
			s = s[len(string(r)):]
			layout = layout[len(string(r)):]
			continue
		}
		if err != nil {
			return time.Time{}, err
		}
		layout = layout[len(tk):]
	}
	if s != "" {
		return time.Time{}, fmt.Errorf("unexpected text %q after the date", s)
	}
	if month < 1 || month > 12 || hour > 23 || min > 59 || sec > 59 {
		return time.Time{}, fmt.Errorf("field out of range")
	}
	t := time.Date(year, time.Month(month), day, hour, min, sec, ms*1_000_000, loc)
	if t.Day() != day || int(t.Month()) != month {
		return time.Time{}, fmt.Errorf("day %d is out of range for the month", day)
	}
	return t, nil
}
