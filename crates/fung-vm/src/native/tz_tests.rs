//! Time zone tests. Every expected value came from Go 1.27
//! (`time.LoadLocation(...)`, `time.Unix(s, 0).In(loc).Zone()`,
//! `time.Date(..., loc)`, `time.LoadLocationFromTZData`, and `time.Local`
//! under each `TZ`). System zone files are read from `/usr/share/zoneinfo`;
//! a test skips a zone the system does not have.

use std::ffi::OsStr;

use super::{Zone, from_tzdata, load, resolve_local};

fn system(name: &str) -> Option<Zone> {
    if !std::path::Path::new("/usr/share/zoneinfo")
        .join(name)
        .exists()
    {
        eprintln!("skipping {name}: not in /usr/share/zoneinfo");
        return None;
    }
    Some(load(name).unwrap_or_else(|| panic!("load({name}) failed")))
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// `(zone, unix seconds, abbreviation, offset)` from Go.
const LOOKUPS: &[(&str, i64, &str, i32)] = &[
    ("Asia/Seoul", -2208988800, "LMT", 30472),
    ("Asia/Seoul", -1830414600, "JST", 32400),
    ("Asia/Seoul", -1830414601, "KST", 30600),
    ("Asia/Seoul", -1325462400, "JST", 32400),
    ("Asia/Seoul", -2717650800, "LMT", 30472),
    ("Asia/Seoul", -2717668237, "LMT", 30472),
    ("Asia/Seoul", -2717668236, "LMT", 30472),
    ("Asia/Seoul", -9223372036854775808, "LMT", 30472),
    ("Asia/Seoul", 0, "KST", 32400),
    ("Asia/Seoul", 1772953199, "KST", 32400),
    ("Asia/Seoul", 1772953200, "KST", 32400),
    ("Asia/Seoul", 1793516399, "KST", 32400),
    ("Asia/Seoul", 1793516400, "KST", 32400),
    ("Asia/Seoul", 4102444800, "KST", 32400),
    ("Asia/Seoul", 4128447600, "KST", 32400),
    ("Asia/Seoul", 4128451200, "KST", 32400),
    ("Asia/Seoul", 4110302400, "KST", 32400),
    ("Asia/Seoul", 4110307199, "KST", 32400),
    ("Asia/Seoul", 4110307200, "KST", 32400),
    ("Asia/Seoul", 2147483647, "KST", 32400),
    ("Asia/Seoul", 2147483648, "KST", 32400),
    ("Asia/Seoul", 4611686018427387903, "KST", 32400),
    ("Asia/Seoul", 1000000000000, "KST", 32400),
    ("America/New_York", -2208988800, "EST", -18000),
    ("America/New_York", -1830414600, "EST", -18000),
    ("America/New_York", -1830414601, "EST", -18000),
    ("America/New_York", -1325462400, "EST", -18000),
    ("America/New_York", -2717650800, "EST", -18000),
    ("America/New_York", -2717668237, "LMT", -17762),
    ("America/New_York", -2717668236, "LMT", -17762),
    ("America/New_York", -9223372036854775808, "LMT", -17762),
    ("America/New_York", 0, "EST", -18000),
    ("America/New_York", 1772953199, "EST", -18000),
    ("America/New_York", 1772953200, "EDT", -14400),
    ("America/New_York", 1793516399, "EST", -18000),
    ("America/New_York", 1793516400, "EST", -18000),
    ("America/New_York", 4102444800, "EST", -18000),
    ("America/New_York", 4128447600, "EDT", -14400),
    ("America/New_York", 4128451200, "EDT", -14400),
    ("America/New_York", 4110302400, "EDT", -14400),
    ("America/New_York", 4110307199, "EDT", -14400),
    ("America/New_York", 4110307200, "EDT", -14400),
    ("America/New_York", 2147483647, "EST", -18000),
    ("America/New_York", 2147483648, "EST", -18000),
    ("America/New_York", 4611686018427387903, "EDT", -14400),
    ("America/New_York", 1000000000000, "EDT", -14400),
    ("UTC", -2208988800, "UTC", 0),
    ("UTC", -1830414600, "UTC", 0),
    ("UTC", -1830414601, "UTC", 0),
    ("UTC", -1325462400, "UTC", 0),
    ("UTC", -2717650800, "UTC", 0),
    ("UTC", -2717668237, "UTC", 0),
    ("UTC", -2717668236, "UTC", 0),
    ("UTC", -9223372036854775808, "UTC", 0),
    ("UTC", 0, "UTC", 0),
    ("UTC", 1772953199, "UTC", 0),
    ("UTC", 1772953200, "UTC", 0),
    ("UTC", 1793516399, "UTC", 0),
    ("UTC", 1793516400, "UTC", 0),
    ("UTC", 4102444800, "UTC", 0),
    ("UTC", 4128447600, "UTC", 0),
    ("UTC", 4128451200, "UTC", 0),
    ("UTC", 4110302400, "UTC", 0),
    ("UTC", 4110307199, "UTC", 0),
    ("UTC", 4110307200, "UTC", 0),
    ("UTC", 2147483647, "UTC", 0),
    ("UTC", 2147483648, "UTC", 0),
    ("UTC", 4611686018427387903, "UTC", 0),
    ("UTC", 1000000000000, "UTC", 0),
    ("Australia/Sydney", -2208988800, "AEST", 36000),
    ("Australia/Sydney", -1830414600, "AEST", 36000),
    ("Australia/Sydney", -1830414601, "AEST", 36000),
    ("Australia/Sydney", -1325462400, "AEST", 36000),
    ("Australia/Sydney", -2717650800, "LMT", 36292),
    ("Australia/Sydney", -2717668237, "LMT", 36292),
    ("Australia/Sydney", -2717668236, "LMT", 36292),
    ("Australia/Sydney", -9223372036854775808, "LMT", 36292),
    ("Australia/Sydney", 0, "AEST", 36000),
    ("Australia/Sydney", 1772953199, "AEDT", 39600),
    ("Australia/Sydney", 1772953200, "AEDT", 39600),
    ("Australia/Sydney", 1793516399, "AEDT", 39600),
    ("Australia/Sydney", 1793516400, "AEDT", 39600),
    ("Australia/Sydney", 4102444800, "AEDT", 39600),
    ("Australia/Sydney", 4128447600, "AEDT", 39600),
    ("Australia/Sydney", 4128451200, "AEDT", 39600),
    ("Australia/Sydney", 4110302400, "AEDT", 39600),
    ("Australia/Sydney", 4110307199, "AEDT", 39600),
    ("Australia/Sydney", 4110307200, "AEDT", 39600),
    ("Australia/Sydney", 2147483647, "AEDT", 39600),
    ("Australia/Sydney", 2147483648, "AEDT", 39600),
    ("Australia/Sydney", 4611686018427387903, "AEST", 36000),
    ("Australia/Sydney", 1000000000000, "AEST", 36000),
    ("Europe/Dublin", -2208988800, "DMT", -1521),
    ("Europe/Dublin", -1830414600, "DMT", -1521),
    ("Europe/Dublin", -1830414601, "DMT", -1521),
    ("Europe/Dublin", -1325462400, "GMT", 0),
    ("Europe/Dublin", -2717650800, "DMT", -1521),
    ("Europe/Dublin", -2717668237, "DMT", -1521),
    ("Europe/Dublin", -2717668236, "DMT", -1521),
    ("Europe/Dublin", -9223372036854775808, "LMT", -1521),
    ("Europe/Dublin", 0, "IST", 3600),
    ("Europe/Dublin", 1772953199, "GMT", 0),
    ("Europe/Dublin", 1772953200, "GMT", 0),
    ("Europe/Dublin", 1793516399, "GMT", 0),
    ("Europe/Dublin", 1793516400, "GMT", 0),
    ("Europe/Dublin", 4102444800, "GMT", 0),
    ("Europe/Dublin", 4128447600, "IST", 3600),
    ("Europe/Dublin", 4128451200, "IST", 3600),
    ("Europe/Dublin", 4110302400, "IST", 3600),
    ("Europe/Dublin", 4110307199, "IST", 3600),
    ("Europe/Dublin", 4110307200, "IST", 3600),
    ("Europe/Dublin", 2147483647, "GMT", 0),
    ("Europe/Dublin", 2147483648, "GMT", 0),
    ("Europe/Dublin", 4611686018427387903, "IST", 3600),
    ("Europe/Dublin", 1000000000000, "IST", 3600),
];

#[test]
fn tz_lookup_matches_go() {
    let mut zone: Option<(String, Option<Zone>)> = None;
    for &(name, sec, abbrev, offset) in LOOKUPS {
        if zone.as_ref().is_none_or(|(n, _)| n != name) {
            zone = Some((name.to_string(), system(name)));
        }
        let Some((_, Some(z))) = &zone else { continue };
        assert_eq!(z.lookup(sec), (abbrev, offset), "{name} at {sec}");
    }
}

/// `(zone, (year, month, day, hour, min, sec, nsec), unix, nsec)` from Go's
/// `time.Date`: nonexistent and repeated wall times at the DST edges, the
/// POSIX footer years, and out-of-range fields that `Date` normalizes.
type DateCase = (&'static str, (i64, i64, i64, i64, i64, i64, i64), i64, i32);
const DATES: &[DateCase] = &[
    ("America/New_York", (2026, 3, 8, 2, 30, 0, 0), 1772951400, 0),
    (
        "America/New_York",
        (2026, 3, 8, 1, 59, 59, 0),
        1772953199,
        0,
    ),
    ("America/New_York", (2026, 3, 8, 3, 0, 0, 0), 1772953200, 0),
    (
        "America/New_York",
        (2026, 11, 1, 1, 30, 0, 0),
        1793511000,
        0,
    ),
    (
        "America/New_York",
        (2026, 11, 1, 0, 59, 59, 0),
        1793509199,
        0,
    ),
    ("America/New_York", (2026, 11, 1, 2, 0, 0, 0), 1793516400, 0),
    ("America/New_York", (2026, 4, 5, 2, 30, 0, 0), 1775370600, 0),
    (
        "America/New_York",
        (2026, 10, 4, 2, 30, 0, 0),
        1791095400,
        0,
    ),
    (
        "America/New_York",
        (2100, 3, 14, 2, 30, 0, 0),
        4108689000,
        0,
    ),
    (
        "America/New_York",
        (2100, 11, 7, 1, 30, 0, 0),
        4129248600,
        0,
    ),
    (
        "America/New_York",
        (2026, 13, 32, 25, 61, 61, 1500000000),
        1801551722,
        500000000,
    ),
    (
        "America/New_York",
        (2026, 0, 0, -1, -1, -1, -1),
        1764475138,
        999999999,
    ),
    ("America/New_York", (1900, 1, 1, 0, 0, 0, 0), -2208970800, 0),
    ("Australia/Sydney", (2026, 3, 8, 2, 30, 0, 0), 1772897400, 0),
    (
        "Australia/Sydney",
        (2026, 3, 8, 1, 59, 59, 0),
        1772895599,
        0,
    ),
    ("Australia/Sydney", (2026, 3, 8, 3, 0, 0, 0), 1772899200, 0),
    (
        "Australia/Sydney",
        (2026, 11, 1, 1, 30, 0, 0),
        1793457000,
        0,
    ),
    (
        "Australia/Sydney",
        (2026, 11, 1, 0, 59, 59, 0),
        1793455199,
        0,
    ),
    ("Australia/Sydney", (2026, 11, 1, 2, 0, 0, 0), 1793458800, 0),
    ("Australia/Sydney", (2026, 4, 5, 2, 30, 0, 0), 1775320200, 0),
    (
        "Australia/Sydney",
        (2026, 10, 4, 2, 30, 0, 0),
        1791045000,
        0,
    ),
    (
        "Australia/Sydney",
        (2100, 3, 14, 2, 30, 0, 0),
        4108635000,
        0,
    ),
    (
        "Australia/Sydney",
        (2100, 11, 7, 1, 30, 0, 0),
        4129194600,
        0,
    ),
    (
        "Australia/Sydney",
        (2026, 13, 32, 25, 61, 61, 1500000000),
        1801494122,
        500000000,
    ),
    (
        "Australia/Sydney",
        (2026, 0, 0, -1, -1, -1, -1),
        1764417538,
        999999999,
    ),
    ("Australia/Sydney", (1900, 1, 1, 0, 0, 0, 0), -2209024800, 0),
];

#[test]
fn tz_date_matches_go() {
    let mut zone: Option<(String, Option<Zone>)> = None;
    for &(name, (y, mo, d, h, mi, s, ns), unix, nsec) in DATES {
        if zone.as_ref().is_none_or(|(n, _)| n != name) {
            zone = Some((name.to_string(), system(name)));
        }
        let Some((_, Some(z))) = &zone else { continue };
        assert_eq!(
            z.date(y, mo, d, h, mi, s, ns),
            (unix, nsec),
            "{name} {y}-{mo}-{d} {h}:{mi}:{s}.{ns}"
        );
    }
}

#[test]
fn tz_date_in_utc_and_fixed_zones() {
    let utc = Zone::utc();
    assert_eq!(utc.date(2026, 10, 16, 12, 0, 0, 0), (1792152000, 0));
    assert_eq!(
        utc.date(1969, 12, 31, 23, 59, 59, 999_999_999),
        (-1, 999_999_999)
    );
    assert_eq!(utc.date(2000, 2, 30, 0, 0, 0, 0), (951868800, 0));
    assert_eq!(utc.lookup(0), ("UTC", 0));
    assert_eq!(utc.lookup(i64::MIN), ("UTC", 0));
}

/// `(case, TZif bytes as hex, [(unix seconds, abbreviation, offset)])`:
/// hand-built files run through Go's `LoadLocationFromTZData` — a v1 file,
/// v2/v3 files whose POSIX footer covers all time (northern and southern
/// DST, `<...>` names with minutes, `J`/day-of-year rules with times, no
/// DST, a malformed footer, and the default `M3.2.0,M11.1.0` rules).
type SynCase = (
    &'static str,
    &'static str,
    &'static [(i64, &'static str, i32)],
);
const SYNTHETIC: &[SynCase] = &[
    (
        "v1",
        "545a696600000000000000000000000000000000000000000000000000000000000000020000000200000008c46536003b9aca00010000000e10000000001c2001044141410042424200",
        &[
            (-2000000000, "AAA", 3600),
            (-1000000001, "AAA", 3600),
            (-1000000000, "BBB", 7200),
            (0, "BBB", 7200),
            (999999999, "BBB", 7200),
            (1000000000, "AAA", 3600),
            (2000000000, "AAA", 3600),
            (1772953199, "AAA", 3600),
            (1772953200, "AAA", 3600),
            (1793516399, "AAA", 3600),
            (1793516400, "AAA", 3600),
            (1775318399, "AAA", 3600),
            (1775318400, "AAA", 3600),
            (1791043199, "AAA", 3600),
            (1791043200, "AAA", 3600),
            (4102444800, "AAA", 3600),
            (-4102444800, "AAA", 3600),
            (951782400, "BBB", 7200),
            (951868800, "BBB", 7200),
            (983404800, "BBB", 7200),
        ],
    ),
    (
        "v1dstfirst",
        "545a696600000000000000000000000000000000000000000000000000000000000000020000000300000008c46536003b9aca00010200000064010000001c20010400000e1000004141410042424200",
        &[
            (-2000000000, "AAA", 100),
            (-1000000001, "AAA", 100),
            (-1000000000, "BBB", 7200),
            (0, "BBB", 7200),
            (999999999, "BBB", 7200),
            (1000000000, "AAA", 3600),
            (2000000000, "AAA", 3600),
            (1772953199, "AAA", 3600),
            (1772953200, "AAA", 3600),
            (1793516399, "AAA", 3600),
            (1793516400, "AAA", 3600),
            (1775318399, "AAA", 3600),
            (1775318400, "AAA", 3600),
            (1791043199, "AAA", 3600),
            (1791043200, "AAA", 3600),
            (4102444800, "AAA", 3600),
            (-4102444800, "AAA", 100),
            (951782400, "BBB", 7200),
            (951868800, "BBB", 7200),
            (983404800, "BBB", 7200),
        ],
    ),
    (
        "v2ny",
        "545a696632000000000000000000000000000000000000000000000000000000000000000000000100000004ffffb9b0000045535400545a696632000000000000000000000000000000000000000000000000000000000000000000000100000004ffffb9b00000455354000a455354354544542c4d332e322e302c4d31312e312e300a",
        &[
            (-2000000000, "EDT", -14400),
            (-1000000001, "EDT", -14400),
            (-1000000000, "EDT", -14400),
            (0, "EST", -18000),
            (999999999, "EDT", -14400),
            (1000000000, "EDT", -14400),
            (2000000000, "EDT", -14400),
            (1772953199, "EST", -18000),
            (1772953200, "EDT", -14400),
            (1793516399, "EST", -18000),
            (1793516400, "EST", -18000),
            (1775318399, "EDT", -14400),
            (1775318400, "EDT", -14400),
            (1791043199, "EDT", -14400),
            (1791043200, "EDT", -14400),
            (4102444800, "EST", -18000),
            (-4102444800, "EST", -18000),
            (951782400, "EST", -18000),
            (951868800, "EST", -18000),
            (983404800, "EST", -18000),
        ],
    ),
    (
        "v3syd",
        "545a69663300000000000000000000000000000000000000000000000000000000000000000000010000000500008ca000004145535400545a69663300000000000000000000000000000000000000000000000000000000000000000000010000000500008ca0000041455354000a414553542d3130414544542c4d31302e312e302c4d342e312e302f330a",
        &[
            (-2000000000, "AEST", 36000),
            (-1000000001, "AEST", 36000),
            (-1000000000, "AEST", 36000),
            (0, "AEDT", 39600),
            (999999999, "AEST", 36000),
            (1000000000, "AEST", 36000),
            (2000000000, "AEST", 36000),
            (1772953199, "AEDT", 39600),
            (1772953200, "AEDT", 39600),
            (1793516399, "AEDT", 39600),
            (1793516400, "AEDT", 39600),
            (1775318399, "AEDT", 39600),
            (1775318400, "AEST", 36000),
            (1791043199, "AEST", 36000),
            (1791043200, "AEDT", 39600),
            (4102444800, "AEDT", 39600),
            (-4102444800, "AEDT", 39600),
            (951782400, "AEDT", 39600),
            (951868800, "AEDT", 39600),
            (983404800, "AEDT", 39600),
        ],
    ),
    (
        "v2angle",
        "545a69663200000000000000000000000000000000000000000000000000000000000001000000010000000600000000000000313800002b3033333000545a6966320000000000000000000000000000000000000000000000000000000000000100000001000000060000000000000000000000313800002b30333330000a3c2b303333303e2d333a33300a",
        &[
            (-2000000000, "+0330", 12600),
            (-1000000001, "+0330", 12600),
            (-1000000000, "+0330", 12600),
            (0, "+0330", 12600),
            (999999999, "+0330", 12600),
            (1000000000, "+0330", 12600),
            (2000000000, "+0330", 12600),
            (1772953199, "+0330", 12600),
            (1772953200, "+0330", 12600),
            (1793516399, "+0330", 12600),
            (1793516400, "+0330", 12600),
            (1775318399, "+0330", 12600),
            (1775318400, "+0330", 12600),
            (1791043199, "+0330", 12600),
            (1791043200, "+0330", 12600),
            (4102444800, "+0330", 12600),
            (-4102444800, "+0330", 12600),
            (951782400, "+0330", 12600),
            (951868800, "+0330", 12600),
            (983404800, "+0330", 12600),
        ],
    ),
    (
        "v2julian",
        "545a6966320000000000000000000000000000000000000000000000000000000000000100000001000000040000000000ffffb9b0000058535400545a696632000000000000000000000000000000000000000000000000000000000000010000000100000004000000000000000000ffffb9b00000585354000a585354355844542c4a36302f313a33302c3330302f2d310a",
        &[
            (-2000000000, "XST", -18000),
            (-1000000001, "XST", -18000),
            (-1000000000, "XST", -18000),
            (0, "XST", -18000),
            (999999999, "XDT", -14400),
            (1000000000, "XDT", -14400),
            (2000000000, "XDT", -14400),
            (1772953199, "XDT", -14400),
            (1772953200, "XDT", -14400),
            (1793516399, "XST", -18000),
            (1793516400, "XST", -18000),
            (1775318399, "XDT", -14400),
            (1775318400, "XDT", -14400),
            (1791043199, "XDT", -14400),
            (1791043200, "XDT", -14400),
            (4102444800, "XST", -18000),
            (-4102444800, "XST", -18000),
            (951782400, "XST", -18000),
            (951868800, "XST", -18000),
            (983404800, "XST", -18000),
        ],
    ),
    (
        "v2nodst",
        "545a696632000000000000000000000000000000000000000000000000000000000000010000000100000004000000000000000000000055544300545a696632000000000000000000000000000000000000000000000000000000000000010000000100000004000000000000000000000000000000555443000a555443300a",
        &[
            (-2000000000, "UTC", 0),
            (-1000000001, "UTC", 0),
            (-1000000000, "UTC", 0),
            (0, "UTC", 0),
            (999999999, "UTC", 0),
            (1000000000, "UTC", 0),
            (2000000000, "UTC", 0),
            (1772953199, "UTC", 0),
            (1772953200, "UTC", 0),
            (1793516399, "UTC", 0),
            (1793516400, "UTC", 0),
            (1775318399, "UTC", 0),
            (1775318400, "UTC", 0),
            (1791043199, "UTC", 0),
            (1791043200, "UTC", 0),
            (4102444800, "UTC", 0),
            (-4102444800, "UTC", 0),
            (951782400, "UTC", 0),
            (951868800, "UTC", 0),
            (983404800, "UTC", 0),
        ],
    ),
    (
        "v2badfooter",
        "545a6966320000000000000000000000000000000000000000000000000000000000000100000001000000040000000000ffffb9b0000045535400545a696632000000000000000000000000000000000000000000000000000000000000010000000100000004000000000000000000ffffb9b00000455354000a455354354544542c4d332e322e300a",
        &[
            (-2000000000, "EST", -18000),
            (-1000000001, "EST", -18000),
            (-1000000000, "EST", -18000),
            (0, "EST", -18000),
            (999999999, "EST", -18000),
            (1000000000, "EST", -18000),
            (2000000000, "EST", -18000),
            (1772953199, "EST", -18000),
            (1772953200, "EST", -18000),
            (1793516399, "EST", -18000),
            (1793516400, "EST", -18000),
            (1775318399, "EST", -18000),
            (1775318400, "EST", -18000),
            (1791043199, "EST", -18000),
            (1791043200, "EST", -18000),
            (4102444800, "EST", -18000),
            (-4102444800, "EST", -18000),
            (951782400, "EST", -18000),
            (951868800, "EST", -18000),
            (983404800, "EST", -18000),
        ],
    ),
    (
        "v2defaultrule",
        "545a6966320000000000000000000000000000000000000000000000000000000000000100000001000000040000000000ffffb9b0000045535400545a696632000000000000000000000000000000000000000000000000000000000000010000000100000004000000000000000000ffffb9b00000455354000a455354354544540a",
        &[
            (-2000000000, "EST", -18000),
            (-1000000001, "EST", -18000),
            (-1000000000, "EST", -18000),
            (0, "EST", -18000),
            (999999999, "EDT", -14400),
            (1000000000, "EDT", -14400),
            (2000000000, "EDT", -14400),
            (1772953199, "EST", -18000),
            (1772953200, "EDT", -14400),
            (1793516399, "EST", -18000),
            (1793516400, "EST", -18000),
            (1775318399, "EDT", -14400),
            (1775318400, "EDT", -14400),
            (1791043199, "EDT", -14400),
            (1791043200, "EDT", -14400),
            (4102444800, "EST", -18000),
            (-4102444800, "EST", -18000),
            (951782400, "EST", -18000),
            (951868800, "EST", -18000),
            (983404800, "EST", -18000),
        ],
    ),
];

#[test]
fn tz_tzif_parser_matches_go() {
    for &(name, hex, want) in SYNTHETIC {
        let z = from_tzdata(name, &unhex(hex)).unwrap_or_else(|| panic!("{name}: rejected"));
        assert_eq!(z.name, name);
        for &(sec, abbrev, offset) in want {
            assert_eq!(z.lookup(sec), (abbrev, offset), "{name} at {sec}");
        }
    }
}

#[test]
fn tz_tzif_parser_rejects_what_go_rejects() {
    // A valid v1 file with one UTC zone and no transitions, then broken copies.
    let good = |version: u8| {
        let mut b = b"TZif".to_vec();
        b.push(version);
        b.extend([0; 15]);
        for n in [0u32, 0, 0, 1, 1, 4] {
            b.extend(n.to_be_bytes());
        }
        b.extend(1u32.to_be_bytes()); // transition at 1
        b.push(0); // to zone 0
        b.extend(0u32.to_be_bytes());
        b.extend([0, 0]);
        b.extend(b"UTC\0");
        b
    };
    assert!(from_tzdata("ok", &good(0)).is_some());
    assert!(from_tzdata("magic", b"TZiX\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0").is_none());
    assert!(from_tzdata("version", &good(b'4')).is_none());
    let mut short = good(0);
    short.truncate(50);
    assert!(from_tzdata("short", &short).is_none());
    let mut badidx = good(0);
    badidx[48] = 3; // the transition's zone index
    assert!(from_tzdata("badidx", &badidx).is_none());
    let mut badabbr = good(0);
    badabbr[54] = 9; // the zone's abbreviation index
    assert!(from_tzdata("badabbr", &badabbr).is_none());
    let mut nozones = b"TZif".to_vec();
    nozones.extend([0; 16 + 24]);
    assert!(from_tzdata("nozones", &nozones).is_none());
    // A v2 header whose 64-bit body is missing.
    assert!(from_tzdata("v2short", &good(b'2')).is_none());
}

/// Go's `initLocal` under each `TZ`: the zone name Go gives `time.Local`
/// and the zone at four instants.
#[test]
fn tz_local_resolution_matches_go() {
    let secs = [0, 1772953200, 1793516400, 4102444800];
    let have = |n: &str| std::path::Path::new("/usr/share/zoneinfo").join(n).exists();
    let check = |tz: Option<&str>, name: &str, want: [(&str, i32); 4]| {
        let z = resolve_local(tz.map(OsStr::new));
        assert_eq!(z.name, name, "TZ={tz:?}");
        for (sec, w) in secs.iter().zip(want) {
            assert_eq!(z.lookup(*sec), w, "TZ={tz:?} at {sec}");
        }
    };
    let utc = [("UTC", 0); 4];
    check(Some(""), "UTC", utc);
    check(Some("UTC"), "UTC", utc);
    check(Some(":"), "UTC", utc);
    check(Some("Nope/Nowhere"), "UTC", utc);
    check(Some("/nonexistent"), "UTC", utc);
    check(Some("../etc/localtime"), "UTC", utc);
    check(Some("localtime"), "UTC", utc);
    check(Some("/usr/share/zoneinfo/"), "UTC", utc);
    if have("Asia/Seoul") {
        let kst = [("KST", 32400); 4];
        check(Some("Asia/Seoul"), "Asia/Seoul", kst);
        check(Some(":Asia/Seoul"), "Asia/Seoul", kst);
        check(
            Some("/usr/share/zoneinfo/Asia/Seoul"),
            "/usr/share/zoneinfo/Asia/Seoul",
            kst,
        );
        check(
            Some(":/usr/share/zoneinfo/Asia/Seoul"),
            "/usr/share/zoneinfo/Asia/Seoul",
            kst,
        );
    }
    if have("America/New_York") {
        let ny = [
            ("EST", -18000),
            ("EDT", -14400),
            ("EST", -18000),
            ("EST", -18000),
        ];
        check(Some("America/New_York"), "America/New_York", ny);
    }
    // No TZ, or TZ naming /etc/localtime: the system zone, named "Local".
    let sys = std::fs::read("/etc/localtime")
        .ok()
        .and_then(|d| from_tzdata("localtime", &d));
    for tz in [None, Some("/etc/localtime"), Some(":/etc/localtime")] {
        let z = resolve_local(tz.map(OsStr::new));
        match &sys {
            Some(s) => {
                assert_eq!(z.name, "Local", "TZ={tz:?}");
                for sec in secs {
                    assert_eq!(z.lookup(sec), s.lookup(sec), "TZ={tz:?} at {sec}");
                }
            }
            None => assert_eq!(z.name, "UTC", "TZ={tz:?}"),
        }
    }
}

#[test]
fn tz_local_is_made_once() {
    assert!(std::ptr::eq(super::local(), super::local()));
}

/// Go's `readFile` gives up past 10 MB, so an endless file is refused and
/// the local zone falls back to UTC instead of reading forever.
#[test]
fn tz_endless_zone_file_is_refused() {
    for tz in ["/dev/zero", ":/dev/zero"] {
        let z = resolve_local(Some(OsStr::new(tz)));
        assert_eq!(z.name, "UTC", "TZ={tz}");
        assert_eq!(z.lookup(0), ("UTC", 0));
    }
}

/// The POSIX footer is parsed as bytes, as Go does: `\xff\xff5` is a
/// two-byte name (too short, so the footer is ignored), not a six-byte
/// U+FFFD pair. Names with invalid UTF-8 are shown lossily.
#[test]
fn tz_footer_is_parsed_as_bytes() {
    type Case = (&'static str, &'static [(i64, &'static str, i32)]);
    let cases: &[Case] = &[
        (
            "545a696632000000000000000000000000000000000000000000000000000000000000010000000100000004000000000000000e10000041414100545a69663200000000000000000000000000000000000000000000000000000000000001000000010000000400000000000000000000000e100000414141000affff350a",
            &[
                (-10, "AAA", 3600),
                (10, "AAA", 3600),
                (1772953200, "AAA", 3600),
            ],
        ),
        (
            "545a696632000000000000000000000000000000000000000000000000000000000000010000000100000004000000000000000e10000041414100545a69663200000000000000000000000000000000000000000000000000000000000001000000010000000400000000000000000000000e100000414141000a3cfffe3e350a",
            &[
                (-10, "AAA", 3600),
                (10, "\u{FFFD}\u{FFFD}", -18000),
                (1772953200, "\u{FFFD}\u{FFFD}", -18000),
            ],
        ),
        (
            "545a696632000000000000000000000000000000000000000000000000000000000000010000000100000004000000000000000e10000041414100545a69663200000000000000000000000000000000000000000000000000000000000001000000010000000400000000000000000000000e100000414141000aff414235ff43442c4d332e322e302c4d31312e312e300a",
            &[
                (-10, "AAA", 3600),
                (10, "\u{FFFD}AB", -18000),
                (1772953200, "\u{FFFD}CD", -14400),
            ],
        ),
    ];
    for &(hex, want) in cases {
        let z = from_tzdata("x", &unhex(hex)).unwrap();
        for &(sec, abbrev, offset) in want {
            assert_eq!(z.lookup(sec), (abbrev, offset), "{hex} at {sec}");
        }
    }
}

/// Every truncation and every single-byte change of a valid file either
/// is rejected or gives a zone that answers lookups and dates.
#[test]
fn tz_damaged_tzif_never_panics() {
    // A v1 file with transitions and a v2 file whose footer covers all time.
    for name in ["v1", "v2ny"] {
        let (_, hex, _) = SYNTHETIC.iter().find(|c| c.0 == name).unwrap();
        let good = unhex(hex);
        let probe = |data: &[u8]| {
            if let Some(z) = from_tzdata("damaged", data) {
                for sec in [i64::MIN, -1, 0, 1772953200, i64::MAX] {
                    let _ = z.lookup(sec);
                }
                let _ = z.date(2026, 3, 8, 2, 30, 0, 0);
                let _ = z.date(i64::MAX, 13, 32, 25, 61, 61, i64::MIN);
            }
        };
        for n in 0..=good.len() {
            probe(&good[..n]);
        }
        for i in 0..good.len() {
            for flip in [0x01, 0x80, 0xff] {
                let mut d = good.clone();
                d[i] ^= flip;
                probe(&d);
            }
        }
    }
}
