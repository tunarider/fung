//! Time value tests: the ISO text, date, clock and weekday at an instant
//! in UTC, fixed offsets and loaded zones, comparison, arithmetic and its
//! range, with the error codes and messages.

use std::cmp::Ordering;
use std::rc::Rc;

use fung_front::diag::{Diag, message};

use super::*;
use crate::native::tz::{self, Zone};

const NS: i64 = 1;
const MS: i64 = 1_000_000;
const SEC: i64 = 1_000_000_000;
const MIN: i64 = 60 * SEC;
const HOUR: i64 = 60 * MIN;

fn utc(sec: i64, nsec: i64) -> Time {
    Time::from_unix(sec, nsec, Loc::Utc).unwrap()
}

fn tv(t: Time) -> Value {
    Value::Time(Rc::new(t))
}

/// Parses an RFC 3339 time for the subset the tests use (`Z` or a
/// `±hh:mm` offset).
fn must_time(s: &str) -> Time {
    let num = |a: usize, b: usize| s[a..b].parse::<i64>().unwrap();
    let (y, mo, d, h, mi, sec) = (
        num(0, 4),
        num(5, 7),
        num(8, 10),
        num(11, 13),
        num(14, 16),
        num(17, 19),
    );
    let mut rest = &s[19..];
    let mut nsec = 0;
    if let Some(frac) = rest.strip_prefix('.') {
        let n = frac.find(|c: char| !c.is_ascii_digit()).unwrap();
        nsec = format!("{:0<9}", &frac[..n]).parse().unwrap();
        rest = &frac[n..];
    }
    if rest == "Z" {
        return Time::from_wall(y, mo, d, h, mi, sec, nsec, Loc::Utc).unwrap();
    }
    let sign = if rest.starts_with('-') { -1 } else { 1 };
    let off = sign
        * (rest[1..3].parse::<i32>().unwrap() * 3600 + rest[4..6].parse::<i32>().unwrap() * 60);
    Time::from_wall(y, mo, d, h, mi, sec, nsec, Loc::Fixed(off)).unwrap()
}

fn fails(r: Result<Value, Diag>, code: &str, msg: &str) {
    match r {
        Err(e) => {
            assert_eq!(e.code, code);
            assert_eq!(message(&e.code, &e.args), msg);
        }
        Ok(v) => panic!("got {}, want {code}", format(&v)),
    }
}

fn iso_of(r: Result<Value, Diag>) -> String {
    match r {
        Ok(Value::Time(t)) => t.iso(),
        Ok(v) => panic!("got {}, want a time", format(&v)),
        Err(e) => panic!("error {}", e.code),
    }
}

#[test]
fn time_format_iso() {
    for (s, want) in [
        ("2026-10-14T09:00:00+09:00", "2026-10-14T09:00:00+09:00"),
        (
            "2026-10-14T09:00:00.123+09:00",
            "2026-10-14T09:00:00.123+09:00",
        ),
        ("2026-10-14T00:00:00Z", "2026-10-14T00:00:00Z"),
    ] {
        assert_eq!(format(&tv(must_time(s))), want, "{s}");
    }
    let t = tv(must_time("2026-10-14T00:00:00Z"));
    assert_eq!(type_name(&t), "time");
    assert_eq!(format_for_print(&t), "2026-10-14T00:00:00Z");
    assert!(matches_type(&t, Some(&["time".to_string()])));
    assert!(!matches_type(&t, Some(&["dur".to_string()])));
}

/// `(unix sec, nsec, ISO, (year, month, day), (hour, min, sec), weekday)` in UTC.
type Ymd = (i64, i64, i64);
type Hms = (i64, i64, i64);

const UTC_CASES: &[(i64, i64, &str, Ymd, Hms, u32)] = &[
    (0, 0, "1970-01-01T00:00:00Z", (1970, 1, 1), (0, 0, 0), 4),
    (
        0,
        120000000,
        "1970-01-01T00:00:00.12Z",
        (1970, 1, 1),
        (0, 0, 0),
        4,
    ),
    (
        0,
        1,
        "1970-01-01T00:00:00.000000001Z",
        (1970, 1, 1),
        (0, 0, 0),
        4,
    ),
    (
        -1,
        999999999,
        "1969-12-31T23:59:59.999999999Z",
        (1969, 12, 31),
        (23, 59, 59),
        3,
    ),
    (
        253402300799,
        0,
        "9999-12-31T23:59:59Z",
        (9999, 12, 31),
        (23, 59, 59),
        5,
    ),
    (
        -62167219200,
        0,
        "0000-01-01T00:00:00Z",
        (0, 1, 1),
        (0, 0, 0),
        6,
    ),
];

#[test]
fn time_utc_parts() {
    for &(sec, nsec, iso, date, clock, wd) in UTC_CASES {
        let t = utc(sec, nsec);
        assert_eq!(t.iso(), iso, "{sec}");
        assert_eq!(t.unix(), sec, "{sec}");
        assert_eq!(i64::from(t.nanosecond()), nsec, "{sec}");
        assert_eq!(t.zone(), ("UTC", 0), "{sec}");
        assert_eq!(t.date(), date, "{sec}");
        assert_eq!(t.clock(), clock, "{sec}");
        assert_eq!(t.weekday(), wd, "{sec}");
    }
}

#[test]
fn time_unix_normalizes_nanoseconds() {
    // Nanoseconds below 0 or past a second move into the seconds.
    assert_eq!(utc(0, -1).iso(), "1969-12-31T23:59:59.999999999Z");
    assert_eq!(utc(1, 2_500_000_000).iso(), "1970-01-01T00:00:03.5Z");
}

#[test]
fn time_fixed_offsets() {
    // `(unix sec, offset, ISO, (hour, min, sec))`; an offset with seconds
    // shows them.
    for &(sec, off, iso, clock) in &[
        (0, -30, "1969-12-31T23:59:30-00:00:30", (23, 59, 30)),
        (0, 30, "1970-01-01T00:00:30+00:00:30", (0, 0, 30)),
        (
            1760400000,
            14 * 3600 + 45 * 60,
            "2025-10-14T14:45:00+14:45",
            (14, 45, 0),
        ),
        (
            -5000000000,
            -17762,
            "1811-07-23T10:10:38-04:56:02",
            (10, 10, 38),
        ),
    ] {
        let t = Time::from_unix(sec, 0, Loc::Fixed(off)).unwrap();
        assert_eq!(t.iso(), iso, "{sec} {off}");
        assert_eq!(t.zone(), ("", off));
        assert_eq!(t.clock(), clock);
    }
}

#[test]
fn time_in_a_loaded_zone() {
    if !std::path::Path::new("/usr/share/zoneinfo/America/New_York").exists() {
        eprintln!("skipping: no America/New_York");
        return;
    }
    let ny: &'static Zone = Box::leak(Box::new(tz::load("America/New_York").unwrap()));
    for &(sec, iso, zone, date, wd) in &[
        (
            -5364662400,
            "1799-12-31T19:03:58-04:56:02",
            ("LMT", -17762),
            (1799, 12, 31),
            2,
        ),
        (
            1760400000,
            "2025-10-13T20:00:00-04:00",
            ("EDT", -14400),
            (2025, 10, 13),
            1,
        ),
        (
            1767225600,
            "2025-12-31T19:00:00-05:00",
            ("EST", -18000),
            (2025, 12, 31),
            3,
        ),
    ] {
        let t = Time::from_unix(sec, 0, Loc::Zone(ny)).unwrap();
        assert_eq!(t.iso(), iso);
        assert_eq!(t.zone(), zone);
        assert_eq!(t.date(), date);
        assert_eq!(t.weekday(), wd);
        assert_eq!(t.utc().iso(), utc(sec, 0).iso());
        assert!(t.utc().equal(&t));
    }
}

#[test]
fn time_local_uses_the_process_zone() {
    let t = utc(1760400000, 0).local();
    assert_eq!(t.unix(), 1760400000);
    let (_, off) = tz::local().lookup(1760400000);
    assert_eq!(t.zone().1, off);
    assert!(equal(&tv(t), &tv(utc(1760400000, 0))));
}

#[test]
fn time_equal_and_compare() {
    let a = tv(must_time("2026-10-14T09:00:00+09:00"));
    let b = tv(must_time("2026-10-14T00:00:00Z")); // same instant, other zone
    let c = tv(must_time("2026-10-14T00:00:01Z"));
    assert!(equal(&a, &b), "same instant in different zones must be ==");
    assert!(!equal(&a, &c));
    assert!(!equal(&a, &Value::Int(1)));
    assert_eq!(compare(&a, &c).unwrap(), Ordering::Less);
    assert_eq!(compare(&c, &a).unwrap(), Ordering::Greater);
    assert_eq!(compare(&a, &b).unwrap(), Ordering::Equal);
    // Nanoseconds count.
    assert_eq!(
        compare(&tv(utc(5, 1)), &tv(utc(5, 2))).unwrap(),
        Ordering::Less
    );
    let e = compare(&a, &Value::Dur(1)).unwrap_err();
    assert_eq!(e.code, "E0501");
    assert_eq!(message(&e.code, &e.args), "cannot compare time and dur");
}

#[test]
fn time_arithmetic() {
    let a = tv(must_time("2026-10-14T09:00:00+09:00"));
    let later = add(&a, &Value::Dur(90 * MIN)).unwrap();
    assert_eq!(format(&later), "2026-10-14T10:30:00+09:00");
    assert_eq!(
        iso_of(add(&Value::Dur(HOUR), &a)),
        "2026-10-14T10:00:00+09:00"
    );
    assert_eq!(
        iso_of(sub(&a, &Value::Dur(HOUR))),
        "2026-10-14T08:00:00+09:00"
    );
    match sub(&later, &a) {
        Ok(Value::Dur(d)) => assert_eq!(d, 90 * MIN),
        other => panic!("time - time = {other:?}"),
    }
    fails(add(&a, &a), "E0501", "cannot add time and time");
    fails(
        mul(&a, &Value::Int(2)),
        "E0501",
        "cannot multiply time and int",
    );
    fails(
        sub(&Value::Dur(1), &a),
        "E0501",
        "cannot subtract dur and time",
    );
    fails(
        sub(&a, &Value::Int(1)),
        "E0501",
        "cannot subtract time and int",
    );
    fails(add(&a, &Value::Int(1)), "E0501", "cannot add time and int");
    // 400 years after the epoch minus the epoch: past the dur range.
    let far = tv(utc(12622780800, 0));
    fails(sub(&far, &tv(utc(0, 0))), "E0503", "integer overflow");
}

#[test]
fn time_add_any_dur() {
    let t0 = tv(utc(0, 0));
    assert_eq!(
        iso_of(add(&t0, &Value::Dur(-1500 * MS))),
        "1969-12-31T23:59:58.5Z"
    );
    assert_eq!(
        iso_of(add(&t0, &Value::Dur(i64::MIN))),
        "1677-09-21T00:12:43.145224192Z"
    );
    assert_eq!(
        iso_of(add(&t0, &Value::Dur(i64::MAX))),
        "2262-04-11T23:47:16.854775807Z"
    );
    // The zone is kept.
    let k = tv(Time::from_unix(0, 0, Loc::Fixed(9 * 3600)).unwrap());
    assert_eq!(
        iso_of(add(&k, &Value::Dur(NS))),
        "1970-01-01T09:00:00.000000001+09:00"
    );
}

#[test]
fn time_arithmetic_stops_at_the_range() {
    let last = tv(utc(MAX_UNIX, 999_999_999));
    let first = tv(utc(MIN_UNIX, 0));
    fails(add(&last, &Value::Dur(NS)), "E0503", "integer overflow");
    fails(sub(&first, &Value::Dur(NS)), "E0503", "integer overflow");
    fails(add(&first, &Value::Dur(-HOUR)), "E0503", "integer overflow");
    // Subtracting the smallest dur moves forward 292 years: in range.
    assert_eq!(
        iso_of(sub(&tv(utc(0, 0)), &Value::Dur(i64::MIN))),
        "2262-04-11T23:47:16.854775808Z"
    );
    // A difference past the dur range is an overflow.
    fails(sub(&last, &first), "E0503", "integer overflow");
    let t0 = tv(utc(0, 0));
    let big = tv(utc(20000000000, 0));
    fails(sub(&big, &t0), "E0503", "integer overflow");
    fails(sub(&t0, &big), "E0503", "integer overflow");
    let dur = |r: Result<Value, Diag>| match r {
        Ok(Value::Dur(d)) => d,
        other => panic!("want a dur, got {other:?}"),
    };
    let at = |d: i64| tv(utc(0, 0).add(i128::from(d)).unwrap());
    assert_eq!(dur(sub(&at(i64::MAX), &t0)), i64::MAX);
    assert_eq!(dur(sub(&at(i64::MIN), &t0)), i64::MIN);
    // Across zones only the instant counts.
    let k = tv(Time::from_unix(3600, 5, Loc::Fixed(-5 * 3600)).unwrap());
    assert_eq!(dur(sub(&k, &t0)), HOUR + 5);
}

// --- the range: years 0000 to 9999 (02) ---

#[test]
fn range_edges() {
    assert_eq!(
        Time::from_unix(MIN_UNIX, 0, Loc::Utc).unwrap().iso(),
        "0000-01-01T00:00:00Z"
    );
    assert_eq!(
        Time::from_unix(MAX_UNIX, 999_999_999, Loc::Utc)
            .unwrap()
            .iso(),
        "9999-12-31T23:59:59.999999999Z"
    );
    assert!(Time::from_unix(MIN_UNIX - 1, 999_999_999, Loc::Utc).is_none());
    assert!(Time::from_unix(MAX_UNIX + 1, 0, Loc::Utc).is_none());
    assert!(Time::from_unix(MAX_UNIX, 1_000_000_000, Loc::Utc).is_none());
}

#[test]
fn add_and_sub_stay_in_range() {
    let last = Time::from_unix(MAX_UNIX, 999_999_999, Loc::Utc).unwrap();
    let first = Time::from_unix(MIN_UNIX, 0, Loc::Utc).unwrap();
    assert!(last.add(1).is_none());
    assert!(first.add(-1).is_none());
    assert_eq!(last.add(-1).unwrap().nanosecond(), 999_999_998);
    // Ten thousand years are more nanoseconds than a dur holds.
    assert!(last.sub(&first).is_none());
    let a = Time::from_unix(10, 5, Loc::Utc).unwrap();
    let b = Time::from_unix(7, 9, Loc::Utc).unwrap();
    assert_eq!(a.sub(&b), Some(2_999_999_996));
    assert_eq!(b.sub(&a), Some(-2_999_999_996));
}

#[test]
fn edges_show_in_any_offset() {
    let first = Time::from_unix(MIN_UNIX, 0, Loc::Fixed(-18000)).unwrap();
    assert_eq!(first.iso(), "-0001-12-31T19:00:00-05:00");
    let last = Time::from_unix(MAX_UNIX, 0, Loc::Fixed(3600)).unwrap();
    assert_eq!(last.iso(), "10000-01-01T00:59:59+01:00");
}

#[test]
fn offset_seconds_show() {
    let t = Time::from_unix(0, 0, Loc::Fixed(8 * 3600 + 27 * 60 + 52)).unwrap();
    assert_eq!(t.iso(), "1970-01-01T08:27:52+08:27:52");
    let t = Time::from_unix(0, 0, Loc::Fixed(-30)).unwrap();
    assert_eq!(t.iso(), "1969-12-31T23:59:30-00:00:30");
}

#[test]
fn dates_match_known_days() {
    for (sec, date, wd) in [
        (0, (1970, 1, 1), 4),
        (951_782_400, (2000, 2, 29), 2),
        (1_735_603_200, (2024, 12, 31), 2),
        (MIN_UNIX + 60 * 86_400, (0, 3, 1), 3),
    ] {
        let t = Time::from_unix(sec, 0, Loc::Utc).unwrap();
        assert_eq!((t.date(), t.weekday()), (date, wd), "{sec}");
    }
}
