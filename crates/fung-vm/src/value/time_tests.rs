//! Time value tests (Go `internal/value/time_test.go` plus edges). Every
//! expected value came from Go 1.27 (`time.Unix(s, n).In(loc)`,
//! `Format(time.RFC3339Nano)`, `Add`, `Sub`, `Zone`, `Date`, `Clock`,
//! `Weekday`) or from the Go `fung` binary (error codes and messages).

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
/// Go's `unixToInternal`.
const U2I: i64 = 62135596800;

fn utc(sec: i64, nsec: i64) -> Time {
    Time::from_unix(sec, nsec, Loc::Utc)
}

fn tv(t: Time) -> Value {
    Value::Time(Rc::new(t))
}

/// Go's `mustTime`: `time.Parse(time.RFC3339Nano, s)` for the subset the
/// tests use (`Z` or a `±hh:mm` offset).
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
    let (unix, nsec) = Zone::utc().date(y, mo, d, h, mi, sec, nsec);
    if rest == "Z" {
        return Time::from_unix(unix, i64::from(nsec), Loc::Utc);
    }
    let sign = if rest.starts_with('-') { -1 } else { 1 };
    let off = sign
        * (rest[1..3].parse::<i32>().unwrap() * 3600 + rest[4..6].parse::<i32>().unwrap() * 60);
    Time::from_unix(unix - i64::from(off), i64::from(nsec), Loc::Fixed(off))
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
fn time_format_iso_go_cases() {
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
        253402300800,
        0,
        "10000-01-01T00:00:00Z",
        (10000, 1, 1),
        (0, 0, 0),
        6,
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
    (
        -62167219201,
        0,
        "-0001-12-31T23:59:59Z",
        (-1, 12, 31),
        (23, 59, 59),
        5,
    ),
    (
        (1 << 62) - 1,
        0,
        "146138514283-06-19T07:45:03Z",
        (146138514283, 6, 19),
        (7, 45, 3),
        2,
    ),
    (
        -(1 << 62) + 1,
        0,
        "-146138510344-07-14T16:14:57Z",
        (-146138510344, 7, 14),
        (16, 14, 57),
        5,
    ),
    // The largest instant Go holds (internal seconds = i64::MAX).
    (
        i64::MAX - U2I,
        999999999,
        "292277024627-12-06T15:30:07.999999999Z",
        (292277024627, 12, 6),
        (15, 30, 7),
        4,
    ),
    (
        i64::MIN + U2I,
        0,
        "-292277020688-01-27T08:29:52Z",
        (-292277020688, 1, 27),
        (8, 29, 52),
        3,
    ),
    // Internal seconds wrap to i64::MIN; Go's unix seconds wrap back.
    (
        i64::MAX - U2I + 1,
        0,
        "292277024627-12-06T15:30:08Z",
        (292277024627, 12, 6),
        (15, 30, 8),
        4,
    ),
];

#[test]
fn time_utc_parts_like_go() {
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
fn time_unix_normalizes_nanoseconds_like_go() {
    // Go `time.Unix(0, -1)`, `time.Unix(1, 2_500_000_000)`.
    assert_eq!(utc(0, -1).iso(), "1969-12-31T23:59:59.999999999Z");
    assert_eq!(utc(1, 2_500_000_000).iso(), "1970-01-01T00:00:03.5Z");
}

#[test]
fn time_fixed_offsets_like_go() {
    // `(unix sec, offset, ISO, (hour, min, sec))`; the ISO offset drops
    // seconds and an offset under a minute shows as `+00:00`.
    for &(sec, off, iso, clock) in &[
        (0, -30, "1969-12-31T23:59:30+00:00", (23, 59, 30)),
        (0, 30, "1970-01-01T00:00:30+00:00", (0, 0, 30)),
        (
            1760400000,
            14 * 3600 + 45 * 60,
            "2025-10-14T14:45:00+14:45",
            (14, 45, 0),
        ),
        (
            -5000000000,
            -17762,
            "1811-07-23T10:10:38-04:56",
            (10, 10, 38),
        ),
    ] {
        let t = Time::from_unix(sec, 0, Loc::Fixed(off));
        assert_eq!(t.iso(), iso, "{sec} {off}");
        assert_eq!(t.zone(), ("", off));
        assert_eq!(t.clock(), clock);
    }
}

#[test]
fn time_in_a_loaded_zone_like_go() {
    if !std::path::Path::new("/usr/share/zoneinfo/America/New_York").exists() {
        eprintln!("skipping: no America/New_York");
        return;
    }
    let ny: &'static Zone = Box::leak(Box::new(tz::load("America/New_York").unwrap()));
    for &(sec, iso, zone, date, wd) in &[
        (
            -5364662400,
            "1799-12-31T19:03:58-04:56",
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
        let t = Time::from_unix(sec, 0, Loc::Zone(ny));
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
fn time_equal_and_compare_go_cases() {
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
fn time_arithmetic_go_cases() {
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
    // Go: time.Unix(0, 0).AddDate(400, 0, 0) - time.Unix(0, 0).
    let far = tv(utc(12622780800, 0));
    fails(sub(&far, &tv(utc(0, 0))), "E0503", "integer overflow");
}

#[test]
fn time_add_like_go() {
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
    let k = tv(Time::from_unix(0, 0, Loc::Fixed(9 * 3600)));
    assert_eq!(
        iso_of(add(&k, &Value::Dur(NS))),
        "1970-01-01T09:00:00.000000001+09:00"
    );
}

#[test]
fn time_add_overflow_like_go() {
    // At the largest instant Go's Add saturates, so the sum is not later.
    let max = tv(utc(i64::MAX - U2I, 999999999));
    fails(add(&max, &Value::Dur(NS)), "E0503", "integer overflow");
    fails(add(&max, &Value::Dur(HOUR)), "E0503", "integer overflow");
    // Saturating short of the end still moves forward: Go keeps it.
    let near = tv(utc(i64::MAX - U2I - 10, 0));
    assert_eq!(
        iso_of(add(&near, &Value::Dur(HOUR))),
        "292277024627-12-06T15:30:07Z"
    );
    // Internal seconds at i64::MIN: going back saturates to MIN + 1, later.
    let min = tv(utc(i64::MAX - U2I + 1, 0));
    fails(sub(&min, &Value::Dur(NS)), "E0503", "integer overflow");
    fails(add(&min, &Value::Dur(-HOUR)), "E0503", "integer overflow");
    assert_eq!(
        iso_of(add(&min, &Value::Dur(HOUR))),
        "292277024627-12-06T16:30:08Z"
    );
    let low = tv(utc(i64::MIN + U2I, 0));
    assert_eq!(
        iso_of(sub(&low, &Value::Dur(NS))),
        "-292277020688-01-27T08:29:51.999999999Z"
    );
    assert_eq!(
        iso_of(sub(&low, &Value::Dur(HOUR))),
        "-292277020688-01-27T07:29:52Z"
    );
    // Go negates the duration first.
    fails(
        sub(&tv(utc(0, 0)), &Value::Dur(i64::MIN)),
        "E0503",
        "integer overflow",
    );
}

#[test]
fn time_sub_like_go() {
    let t0 = utc(0, 0);
    let dur = |r: Result<Value, Diag>| match r {
        Ok(Value::Dur(d)) => d,
        other => panic!("want a dur, got {other:?}"),
    };
    let at = |d: i64| tv(t0.add(d));
    // Go's Sub saturates at the dur range; fung calls the bounds overflow.
    assert_eq!(dur(sub(&at(i64::MAX - 1), &tv(t0.clone()))), i64::MAX - 1);
    assert_eq!(dur(sub(&at(i64::MIN + 1), &tv(t0.clone()))), i64::MIN + 1);
    fails(
        sub(&at(i64::MAX), &tv(t0.clone())),
        "E0503",
        "integer overflow",
    );
    fails(
        sub(&at(i64::MIN), &tv(t0.clone())),
        "E0503",
        "integer overflow",
    );
    let big = tv(utc(20000000000, 0));
    fails(sub(&big, &tv(t0.clone())), "E0503", "integer overflow");
    fails(sub(&tv(t0.clone()), &big), "E0503", "integer overflow");
    // Go: max.Sub(min) and min.Sub(max) at the internal extremes saturate.
    let max = utc(i64::MAX - U2I, 999999999);
    let min = utc(i64::MAX - U2I + 1, 0);
    assert_eq!(max.sub(&min), i64::MAX);
    assert_eq!(min.sub(&max), i64::MIN);
    // Across zones only the instant counts.
    let k = tv(Time::from_unix(3600, 5, Loc::Fixed(-5 * 3600)));
    assert_eq!(dur(sub(&k, &tv(t0))), HOUR + 5);
}
