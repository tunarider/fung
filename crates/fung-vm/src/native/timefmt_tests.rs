//! Go `TestFormatTimeTokens` and `TestParseTimeTokens`, and Go's
//! `time.Parse(time.RFC3339Nano, …)` results and messages (from a Go 1.27
//! program in UTC).

use super::{format_time, parse_rfc3339, parse_time};
use crate::native::tz::{self, Zone};
use crate::value::{Loc, Time};

fn utc_zone() -> &'static Zone {
    Box::leak(Box::new(Zone::utc()))
}

/// 2026-10-04 09:05:07.123 at a fixed +09:00 (Go's `kst`).
fn kst_time() -> Time {
    Time::from_unix(1_791_072_307, 123_000_000, Loc::Fixed(9 * 3600))
}

#[test]
fn format_time_tokens() {
    let at = kst_time();
    for (layout, want) in [
        ("YYYY-MM-DD HH:mm:ss", "2026-10-04 09:05:07"),
        ("YYYY-MM-DDTHH:mm:ssZ", "2026-10-04T09:05:07+09:00"),
        ("YY/MM/DD", "26/10/04"),
        ("HH:mm:ss.SSS ZZ", "09:05:07.123 +0900"),
        ("DD.MM.YYYY", "04.10.2026"),
        ("literal text", "literal text"),
        ("YYYYMMDD", "20261004"),
        ("ddd, DD MMM YYYY", "Sun, 04 Oct 2026"),
        ("MMM MM", "Oct 10"),
        // beyond Go's test: unknown letters, non-ASCII literals, the
        // longest token first
        ("é\tZZZ SSSS YYYYY", "é\t+0900+09:00 123S 2026Y"),
        ("", ""),
    ] {
        assert_eq!(format_time(&at, layout), want, "{layout}");
    }
}

#[test]
fn format_pads_like_go_itoa() {
    // year -1: Go pads strconv.Itoa(-1) to "00-1"; YY is -1 % 100
    let t = Time::from_unix(-62_167_219_201, 0, Loc::Utc);
    assert_eq!(format_time(&t, "YYYY YY"), "00-1 -1");
    // an offset with seconds (New York LMT, -4:56:02) drops them
    let t = Time::from_unix(0, 0, Loc::Fixed(-17_762));
    assert_eq!(format_time(&t, "Z ZZ"), "-04:56 -0456");
}

/// RFC 3339 with nanoseconds, shown with the offset only when it has one.
fn show(t: &Time, with_zone: bool) -> String {
    let iso = t.iso();
    if with_zone {
        return iso;
    }
    // drop the zone part to compare a local-zone result
    let cut = iso.rfind(['+', '-', 'Z']).filter(|&i| i >= 19).unwrap();
    iso[..cut].to_string()
}

#[test]
fn parse_time_tokens() {
    let local = utc_zone();
    for (s, layout, want, zoned) in [
        (
            "2026-10-04 09:05:07",
            "YYYY-MM-DD HH:mm:ss",
            "2026-10-04T09:05:07",
            false,
        ),
        (
            "2026-10-04T09:05:07+09:00",
            "YYYY-MM-DDTHH:mm:ssZ",
            "2026-10-04T09:05:07+09:00",
            true,
        ),
        (
            "2026-10-04T09:05:07-0130",
            "YYYY-MM-DDTHH:mm:ssZZ",
            "2026-10-04T09:05:07-01:30",
            true,
        ),
        (
            "26/10/04 09:05:07.250",
            "YY/MM/DD HH:mm:ss.SSS",
            "2026-10-04T09:05:07.25",
            false,
        ),
        ("20261004", "YYYYMMDD", "2026-10-04T00:00:00", false),
        (
            "Sun, 04 oct 2026",
            "ddd, DD MMM YYYY",
            "2026-10-04T00:00:00",
            false,
        ),
        (
            "04/Dec/2026:23:59:59 +0900",
            "DD/MMM/YYYY:HH:mm:ss ZZ",
            "2026-12-04T23:59:59+09:00",
            true,
        ),
    ] {
        let got = parse_time(s, layout, local).unwrap_or_else(|e| panic!("{s} {layout}: {e}"));
        assert_eq!(show(&got, zoned), want, "{s} {layout}");
    }
}

#[test]
fn parse_time_rejects() {
    let local = utc_zone();
    for (s, layout, msg) in [
        ("2026-13-01", "YYYY-MM-DD", "field out of range"),
        (
            "2026-02-30",
            "YYYY-MM-DD",
            "day 30 is out of range for the month",
        ),
        ("2026-1-5", "YYYY-MM-DD", "expected 2 digits for the month"),
        (
            "2026-10-04x",
            "YYYY-MM-DD",
            "unexpected text \"x\" after the date",
        ),
        ("2026-10-04", "YYYY-MM-DD HH:mm", "expected \" \""),
        ("09:60", "HH:mm", "field out of range"),
        (
            "2026-10-04T09:05:07+9:00",
            "YYYY-MM-DDTHH:mm:ssZ",
            "expected 2 digits for the zone hours",
        ),
        ("Foo 04", "MMM DD", "unknown month name \"Foo\""),
        ("Xyz, 04", "ddd, DD", "unknown weekday name \"Xyz\""),
        // beyond Go's test
        ("abé", "MMM", "unknown month name \"ab\\xc3\""),
        ("é", "MMM", "expected a month name"),
        ("2026년10월", "YYYY년MM일", "expected \"일\""),
        ("2026\t10", "YYYY MM", "expected \" \""),
        (
            "2026-10-04T09:05:07+09-00",
            "YYYY-MM-DDTHH:mm:ssZ",
            "expected `:` in the time zone offset",
        ),
        (
            "2026-10-04T09:05:07 0900",
            "YYYY-MM-DDTHH:mm:ssZZ",
            "expected a time zone offset",
        ),
        (
            "2026-10-00",
            "YYYY-MM-DD",
            "day 0 is out of range for the month",
        ),
    ] {
        match parse_time(s, layout, local) {
            Ok(t) => panic!("{s} {layout}: parsed as {}", t.iso()),
            Err(e) => assert_eq!(e, msg, "{s} {layout}"),
        }
    }
}

#[test]
fn parse_time_empty_layout_is_year_zero_local() {
    let t = parse_time("", "", utc_zone()).unwrap();
    assert_eq!(t.iso(), "0000-01-01T00:00:00Z");
}

const LAYOUT: &str = "\"2006-01-02T15:04:05.999999999Z07:00\"";

#[test]
fn parse_rfc3339_like_go() {
    let local = utc_zone();
    for (s, want) in [
        ("2026-10-04T09:05:07+09:00", "2026-10-04T09:05:07+09:00"),
        ("2026-10-04T09:05:07Z", "2026-10-04T09:05:07Z"),
        (
            "2026-10-04T09:05:07.123456789012Z",
            "2026-10-04T09:05:07.123456789Z",
        ),
        ("2026-10-04T9:05:07Z", "2026-10-04T09:05:07Z"),
        ("2026-10-04T09:05:07,5+01:00", "2026-10-04T09:05:07.5+01:00"),
        ("2026-10-04T09:05:07+24:00", "2026-10-04T09:05:07+24:00"),
        ("2026-10-04T09:05:07-23:60", "2026-10-04T09:05:07-24:00"),
        ("2024-02-29T00:00:00Z", "2024-02-29T00:00:00Z"),
        ("0000-01-01T00:00:00Z", "0000-01-01T00:00:00Z"),
    ] {
        let t = parse_rfc3339(s, local).unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!(t.iso(), want, "{s}");
    }
}

#[test]
fn parse_rfc3339_errors_like_go() {
    let local = utc_zone();
    let elem = |v: &str, ve: &str, le: &str| {
        format!("parsing time {v} as {LAYOUT}: cannot parse {ve} as \"{le}\"")
    };
    let msg = |v: &str, m: &str| format!("parsing time {v}: {m}");
    for (s, want) in [
        ("nope", elem("\"nope\"", "\"nope\"", "2006")),
        ("", elem("\"\"", "\"\"", "2006")),
        ("2026", elem("\"2026\"", "\"\"", "-")),
        (
            "2026-13-04T00:00:00Z",
            msg("\"2026-13-04T00:00:00Z\"", "month out of range"),
        ),
        (
            "2026-00-04T00:00:00Z",
            msg("\"2026-00-04T00:00:00Z\"", "month out of range"),
        ),
        (
            "2026-02-30T00:00:00Z",
            msg("\"2026-02-30T00:00:00Z\"", "day out of range"),
        ),
        (
            "2026-10-00T00:00:00Z",
            msg("\"2026-10-00T00:00:00Z\"", "day out of range"),
        ),
        (
            "2026-10-04T24:00:00Z",
            msg("\"2026-10-04T24:00:00Z\"", "hour out of range"),
        ),
        (
            "2026-10-04T23:60:00Z",
            msg("\"2026-10-04T23:60:00Z\"", "minute out of range"),
        ),
        (
            "2026-10-04T23:59:60Z",
            msg("\"2026-10-04T23:59:60Z\"", "second out of range"),
        ),
        (
            "2026-10-04T23:59:59",
            elem("\"2026-10-04T23:59:59\"", "\"\"", "Z07:00"),
        ),
        (
            "2026-10-04T23:59:59.Z",
            elem("\"2026-10-04T23:59:59.Z\"", "\".Z\"", "Z07:00"),
        ),
        (
            "2026-10-04T23:59:59Zx",
            msg("\"2026-10-04T23:59:59Zx\"", "extra text: \"x\""),
        ),
        (
            "2026-10-04T23:59:59+25:00",
            msg(
                "\"2026-10-04T23:59:59+25:00\"",
                "time zone offset hour out of range",
            ),
        ),
        (
            "2026-10-04T23:59:59+01:61",
            msg(
                "\"2026-10-04T23:59:59+01:61\"",
                "time zone offset minute out of range",
            ),
        ),
        (
            "2026-10-04T23:59:59*01:00",
            elem("\"2026-10-04T23:59:59*01:00\"", "\"*01:00\"", "Z07:00"),
        ),
        (
            "2026-10-04T23:59:59+0100",
            elem("\"2026-10-04T23:59:59+0100\"", "\"+0100\"", "Z07:00"),
        ),
        (
            "2026-10-04 23:59:59Z",
            elem("\"2026-10-04 23:59:59Z\"", "\" 23:59:59Z\"", "T"),
        ),
        (
            "2026/10/04T23:59:59Z",
            elem("\"2026/10/04T23:59:59Z\"", "\"/10/04T23:59:59Z\"", "-"),
        ),
        (
            "20x6-10-04T23:59:59Z",
            elem(
                "\"20x6-10-04T23:59:59Z\"",
                "\"20x6-10-04T23:59:59Z\"",
                "2006",
            ),
        ),
        ("é", elem("\"\\xc3\\xa9\"", "\"\\xc3\\xa9\"", "2006")),
        (
            "2026-10-04T23:59:59\tZ",
            elem("\"2026-10-04T23:59:59\\x09Z\"", "\"\\x09Z\"", "Z07:00"),
        ),
        (
            "2026-1-04T00:00:00Z",
            elem("\"2026-1-04T00:00:00Z\"", "\"1-04T00:00:00Z\"", "01"),
        ),
        (
            "2026-10-4T00:00:00Z",
            elem("\"2026-10-4T00:00:00Z\"", "\"4T00:00:00Z\"", "02"),
        ),
        (
            "2026-10-04T23:59:59+1é:00",
            elem(
                "\"2026-10-04T23:59:59+1\\xc3\\xa9:00\"",
                "\"+1\\xc3\\xa9:00\"",
                "Z07:00",
            ),
        ),
        (
            "2026-10-04T23:59:59+25:0é",
            msg(
                "\"2026-10-04T23:59:59+25:0\\xc3\\xa9\"",
                "time zone offset hour out of range",
            ),
        ),
        (
            "2026-10-04T23:59:59.1234567891",
            elem("\"2026-10-04T23:59:59.1234567891\"", "\"\"", "Z07:00"),
        ),
        (
            "2026-10-04T23:59:59+09:00 ",
            msg("\"2026-10-04T23:59:59+09:00 \"", "extra text: \" \""),
        ),
        // DEL is not escaped by Go's time.quote; `"` and `\` are
        (
            "\"q\\\u{7f}",
            elem("\"\\\"q\\\\\u{7f}\"", "\"\\\"q\\\\\u{7f}\"", "2006"),
        ),
    ] {
        match parse_rfc3339(s, local) {
            Ok(t) => panic!("{s:?}: parsed as {}", t.iso()),
            Err(e) => assert_eq!(e, want, "{s:?}"),
        }
    }
}

/// An offset the local zone has at that instant keeps the local zone; any
/// other is a fixed offset (seen after moving across a change).
#[test]
fn parse_rfc3339_picks_the_local_zone_by_offset() {
    let Some(ny) = tz::load("America/New_York") else {
        eprintln!("skipping: no America/New_York zone file");
        return;
    };
    let ny: &'static Zone = Box::leak(Box::new(ny));
    let hours = 4000 * 3_600_000_000_000;
    let local = parse_rfc3339("2026-07-01T12:00:00-04:00", ny).unwrap();
    assert_eq!(local.add(hours).iso(), "2026-12-15T03:00:00-05:00");
    let fixed = parse_rfc3339("2026-07-01T12:00:00-05:00", ny).unwrap();
    assert_eq!(fixed.add(hours).iso(), "2026-12-15T04:00:00-05:00");
}

/// Go `time.Date` in New York: a skipped wall time reads with the offset
/// before the change, a repeated one takes the first (daylight) occurrence.
#[test]
fn parse_time_across_new_york_changes() {
    let Some(ny) = tz::load("America/New_York") else {
        eprintln!("skipping: no America/New_York zone file");
        return;
    };
    let ny: &'static Zone = Box::leak(Box::new(ny));
    for (s, want) in [
        ("2026-03-08 01:59", "2026-03-08T01:59:00-05:00"),
        ("2026-03-08 02:00", "2026-03-08T01:00:00-05:00"),
        ("2026-03-08 02:30", "2026-03-08T01:30:00-05:00"),
        ("2026-03-08 03:00", "2026-03-08T03:00:00-04:00"),
        ("2026-11-01 01:30", "2026-11-01T01:30:00-04:00"),
        ("2026-11-01 02:00", "2026-11-01T02:00:00-05:00"),
    ] {
        let t = parse_time(s, "YYYY-MM-DD HH:mm", ny).unwrap();
        assert_eq!(t.iso(), want, "{s}");
    }
    // year 0 local is LMT, an offset with seconds
    assert_eq!(
        parse_time("", "", ny).unwrap().iso(),
        "0000-01-01T00:00:00-04:56"
    );
}
