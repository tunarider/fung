//! Time format tests: formatting and parsing with layout tokens, and
//! RFC 3339 parsing (results and messages, in UTC).

use super::{format_time, parse_rfc3339, parse_time};
use crate::native::tz::{self, Zone};
use crate::value::{Loc, Time};

fn utc_zone() -> &'static Zone {
    Box::leak(Box::new(Zone::utc()))
}

/// 2026-10-04 09:05:07.123 at a fixed +09:00 (Korean standard time).
fn kst_time() -> Time {
    Time::from_unix(1_791_072_307, 123_000_000, Loc::Fixed(9 * 3600)).unwrap()
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
        // unknown letters, non-ASCII literals, the longest token first
        ("é\tZZZ SSSS YYYYY", "é\t+0900+09:00 123S 2026Y"),
        ("", ""),
    ] {
        assert_eq!(format_time(&at, layout), want, "{layout}");
    }
}

#[test]
fn format_pads_to_fixed_width() {
    let t = Time::from_unix(crate::value::MIN_UNIX, 0, Loc::Utc).unwrap();
    assert_eq!(format_time(&t, "YYYY YY MM DD"), "0000 00 01 01");
    // an offset with seconds (New York LMT, -4:56:02) shows them
    let t = Time::from_unix(0, 0, Loc::Fixed(-17_762)).unwrap();
    assert_eq!(format_time(&t, "Z ZZ"), "-04:56:02 -045602");
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
        ("2026-13-01", "YYYY-MM-DD", "month out of range"),
        ("2026-02-30", "YYYY-MM-DD", "day out of range"),
        ("2026-1-5", "YYYY-MM-DD", "expected 2 digits for the month"),
        (
            "2026-10-04x",
            "YYYY-MM-DD",
            "unexpected text after the time",
        ),
        ("2026-10-04", "YYYY-MM-DD HH:mm", "expected \" \""),
        ("09:60", "HH:mm", "minute out of range"),
        (
            "2026-10-04T09:05:07+9:00",
            "YYYY-MM-DDTHH:mm:ssZ",
            "expected 2 digits for the offset hours",
        ),
        ("Foo 04", "MMM DD", "expected a month name like Oct"),
        ("Xyz, 04", "ddd, DD", "expected a weekday name like Sun"),
        ("abé", "MMM", "expected a month name like Oct"),
        ("é", "MMM", "expected a month name like Oct"),
        ("2026년10월", "YYYY년MM일", "expected \"일\""),
        ("2026\t10", "YYYY MM", "expected \" \""),
        (
            "2026-10-04T09:05:07+09-00",
            "YYYY-MM-DDTHH:mm:ssZ",
            "expected `:` in the offset",
        ),
        (
            "2026-10-04T09:05:07 0900",
            "YYYY-MM-DDTHH:mm:ssZZ",
            "expected a time zone offset",
        ),
        ("2026-10-00", "YYYY-MM-DD", "day out of range"),
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

#[test]
fn parse_rfc3339_reads_the_strict_form() {
    for (s, want) in [
        ("2026-10-04T09:05:07+09:00", "2026-10-04T09:05:07+09:00"),
        ("2026-10-04T09:05:07Z", "2026-10-04T09:05:07Z"),
        ("2026-10-04t09:05:07z", "2026-10-04T09:05:07Z"),
        (
            "2026-10-04T09:05:07.123456789Z",
            "2026-10-04T09:05:07.123456789Z",
        ),
        ("2026-10-04T09:05:07.5+01:00", "2026-10-04T09:05:07.5+01:00"),
        ("2026-10-04T09:05:07-23:59", "2026-10-04T09:05:07-23:59"),
        (
            "1874-12-07T13:43:58-04:56:02",
            "1874-12-07T13:43:58-04:56:02",
        ),
        ("2024-02-29T00:00:00Z", "2024-02-29T00:00:00Z"),
        ("0000-01-01T00:00:00Z", "0000-01-01T00:00:00Z"),
        ("9999-12-31T23:59:59Z", "9999-12-31T23:59:59Z"),
    ] {
        let t = parse_rfc3339(s).unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!(t.iso(), want, "{s}");
    }
}

#[test]
fn parse_rfc3339_errors() {
    for (s, want) in [
        ("nope", "expected 4 digits for the year"),
        ("", "expected 4 digits for the year"),
        ("2026", "expected `-` after the year"),
        ("2026/10/04T23:59:59Z", "expected `-` after the year"),
        ("2026-1-04T00:00:00Z", "expected 2 digits for the month"),
        ("2026-10-4T00:00:00Z", "expected 2 digits for the day"),
        ("2026-13-04T00:00:00Z", "month out of range"),
        ("2026-00-04T00:00:00Z", "month out of range"),
        ("2026-02-30T00:00:00Z", "day out of range"),
        ("2026-10-00T00:00:00Z", "day out of range"),
        ("2026-10-04T24:00:00Z", "hour out of range"),
        ("2026-10-04T23:60:00Z", "minute out of range"),
        ("2026-10-04T23:59:60Z", "second out of range"),
        (
            "2026-10-04 23:59:59Z",
            "expected `T` between the date and the time",
        ),
        ("2026-10-04T9:05:07Z", "expected 2 digits for the hour"),
        ("2026-10-04T23:59:59", "expected Z or an offset like +09:00"),
        (
            "2026-10-04T23:59:59.Z",
            "expected 1 to 9 digits after the decimal point",
        ),
        (
            "2026-10-04T23:59:59,5Z",
            "expected Z or an offset like +09:00",
        ),
        (
            "2026-10-04T23:59:59.1234567891Z",
            "expected 1 to 9 digits after the decimal point",
        ),
        ("2026-10-04T23:59:59Zx", "unexpected text after the time"),
        (
            "2026-10-04T23:59:59+09:00 ",
            "unexpected text after the time",
        ),
        ("2026-10-04T23:59:59+24:00", "offset out of range"),
        ("2026-10-04T23:59:59+01:60", "offset out of range"),
        ("2026-10-04T23:59:59+0100", "expected `:` in the offset"),
        (
            "2026-10-04T23:59:59*01:00",
            "expected Z or an offset like +09:00",
        ),
        ("10000-01-01T00:00:00Z", "expected `-` after the year"),
    ] {
        match parse_rfc3339(s) {
            Ok(t) => panic!("{s:?}: parsed as {}", t.iso()),
            Err(e) => assert_eq!(e, want, "{s:?}"),
        }
    }
}

/// A numeric offset is kept as written, whatever the local zone (seen
/// after moving across a change).
#[test]
fn parse_rfc3339_keeps_the_offset() {
    let Some(ny) = tz::load("America/New_York") else {
        eprintln!("skipping: no America/New_York zone file");
        return;
    };
    let _ = ny;
    let hours = 4000 * 3_600_000_000_000;
    // The offset is kept as written, even when the local zone has it.
    let t = parse_rfc3339("2026-07-01T12:00:00-04:00").unwrap();
    assert_eq!(t.add(hours).unwrap().iso(), "2026-12-15T04:00:00-04:00");
}

/// Wall times in New York: a skipped wall time reads with the offset
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
        ("2026-03-08 02:00", "2026-03-08T03:00:00-04:00"),
        ("2026-03-08 02:30", "2026-03-08T03:30:00-04:00"),
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
        "0000-01-01T00:00:00-04:56:02"
    );
}
