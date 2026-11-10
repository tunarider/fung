//! `_native.time` tests: the natives called directly.

use std::io::Cursor;
use std::rc::Rc;

use fung_front::source::SourceMap;

use super::module;
use crate::testutil::Buf;
use crate::value::{Loc, Time, Value, equal, format};
use crate::vm::{Runtime, Stop};

fn call(name: &str, args: Vec<Value>) -> Result<Value, Stop> {
    let m = module();
    let Some(Value::Native(n)) = m.get(name) else {
        panic!("no _native.time.{name}");
    };
    let rt = Runtime::new(
        Box::new(Buf::default()),
        Box::new(Buf::default()),
        Box::new(Cursor::new(Vec::new())),
        SourceMap::default(),
    );
    let r = (n.plain())(&mut rt.task(), args.into());
    drop(rt);
    r
}

fn ok(name: &str, args: Vec<Value>) -> Value {
    match call(name, args) {
        Ok(v) => v,
        Err(Stop::Error(d)) => panic!("{name}: {}", d.code),
        Err(Stop::Exit(n)) => panic!("{name}: exit {n}"),
    }
}

fn code(r: Result<Value, Stop>) -> String {
    match r {
        Ok(v) => format(&v),
        Err(Stop::Error(d)) => d.code.clone(),
        Err(Stop::Exit(n)) => format!("exit {n}"),
    }
}

#[test]
fn time_module() {
    let iso = Value::str("2026-10-04T09:05:07+09:00");
    let parsed = ok("parse", vec![iso.clone(), Value::Nil]);
    assert_eq!(format(&parsed), "Ok(2026-10-04T09:05:07+09:00)");
    let Value::Tag(tag) = &parsed else {
        panic!("not a tag")
    };
    let at = tag.args[0].clone();
    assert!(equal(&ok("format", vec![at.clone(), Value::Nil]), &iso));
    assert_eq!(
        format(&ok(
            "format",
            vec![at.clone(), Value::str("YYYY/MM/DD HH:mm")]
        )),
        "\"2026/10/04 09:05\""
    );
    let r = ok(
        "parse",
        vec![
            Value::str("2026/10/04 09:05"),
            Value::str("YYYY/MM/DD HH:mm"),
        ],
    );
    assert!(
        format(&r).starts_with("Ok(2026-10-04T09:05:00"),
        "{}",
        format(&r)
    );
    let r = ok("parse", vec![Value::str("nope"), Value::Nil]);
    assert!(
        format(&r).starts_with("Err({kind: 'parse, message: "),
        "{}",
        format(&r)
    );
    assert_eq!(format(&ok("utc", vec![at.clone()])), "2026-10-04T00:05:07Z");
    assert!(matches!(
        ok("unix", vec![at.clone()]),
        Value::Int(1_791_072_307)
    ));
    assert!(equal(
        &ok("from_unix", vec![Value::Int(1_791_072_307)]),
        &at
    ));
    assert_eq!(
        format(&ok("parts", vec![at.clone()])),
        "{year: 2026, month: 10, day: 4, hour: 9, minute: 5, second: 7, weekday: 'sun}"
    );

    let now = ok("now", vec![]);
    let Value::Time(n) = &now else {
        panic!("now: {}", format(&now))
    };
    let wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!((wall - n.unix()).abs() < 60, "now: {}", format(&now));
    assert_eq!(n.iso(), n.local().unwrap().iso(), "now is local");

    let r = ok("local", vec![ok("utc", vec![at.clone()])]);
    let Value::Time(l) = &r else { panic!() };
    assert!(equal(&r, &at));
    let Value::Time(a) = &at else { panic!() };
    assert_eq!(l.iso(), a.local().unwrap().iso(), "local: {}", format(&r));

    assert_eq!(
        code(call("format", vec![Value::Int(1), Value::Nil])),
        "E0403"
    );
    assert_eq!(code(call("parse", vec![iso, Value::Int(1)])), "E0403");
    // Years 0000 to 9999 in the local zone (LB37): the UTC edges hold
    // where the local offset does not push them past.
    let (min, max) = (crate::value::MIN_UNIX, crate::value::MAX_UNIX);
    let off = |at: i64| i64::from(super::tz::local().lookup(at).1);
    assert_eq!(
        code(call(
            "from_unix",
            vec![Value::Int(max + 1 - off(max).min(0))]
        )),
        "E0403"
    );
    assert_eq!(
        code(call(
            "from_unix",
            vec![Value::Int(min - 1 - off(min).max(0))]
        )),
        "E0403"
    );
    ok("from_unix", vec![Value::Int(max - off(max).max(0))]);
    ok("from_unix", vec![Value::Int(min - off(min).min(0))]);
}

/// `format(t)` without a layout is the ISO form; a time made in UTC keeps it.
#[test]
fn format_default_is_iso() {
    let t = Value::Time(Rc::new(Time::from_unix(0, 5, Loc::Utc).unwrap()));
    assert_eq!(
        format(&ok("format", vec![t, Value::Nil])),
        "\"1970-01-01T00:00:00.000000005Z\""
    );
}

/// L10: `monotonic` is a dur from a fixed point near the start, that never
/// goes back and grows with the time that passes, whatever the wall clock
/// does.
#[test]
fn monotonic_is_a_growing_dur() {
    let dur = |v: Value| match v {
        Value::Dur(d) => d,
        v => panic!("not a dur: {}", format(&v)),
    };
    let a = dur(ok("monotonic", vec![]));
    let b = dur(ok("monotonic", vec![]));
    assert!(a >= 0 && b >= a, "{a} {b}");
    assert!(a < 60_000_000_000, "near the start: {a}");
    std::thread::sleep(std::time::Duration::from_millis(20));
    let c = dur(ok("monotonic", vec![]));
    assert!(c - b >= 20_000_000, "{b} {c}");
}

/// A time is one instant, so a format must name the whole date: year,
/// month and day (LB20). Without one, E0403 before anything is read.
#[test]
fn parse_formats_need_a_date() {
    for layout in ["HH:mm", "MM-DD HH:mm", "YYYY-MM", "YYYY DD", "ddd HH", ""] {
        let r = call("parse", vec![Value::str("x"), Value::str(layout)]);
        let Err(Stop::Error(d)) = r else {
            panic!("{layout:?}: {}", code(r))
        };
        assert_eq!(d.code, "E0403", "{layout:?}");
        let args = format!("{:?}", d.args);
        assert!(args.contains("year, month and day"), "{args}");
    }
    for layout in ["YYYY-MM-DD", "YY/MMM/DD HH:mm", "DD.MM.YYYY"] {
        let r = call("parse", vec![Value::str("x"), Value::str(layout)]);
        assert!(code(r).starts_with("Err("), "{layout:?}");
    }
}

/// The E0403 of `r`, as its rendered message and help.
fn arg_error(r: Result<Value, Stop>) -> (String, Option<String>) {
    let Err(Stop::Error(d)) = r else {
        panic!("not an error: {}", code(r))
    };
    assert_eq!(d.code, "E0403", "{:?}", d.args);
    (format!("{:?}", d.args), d.help.clone())
}

/// LB36 (D28): a parse format naming one field twice is E0403 (it would
/// let the last one win silently); `ddd` must be the date's weekday.
#[test]
fn parse_formats_name_each_field_once_and_check_the_weekday() {
    for layout in [
        "YYYY-MM-DD MMM",
        "YYYY-MM-DD YYYY",
        "YY YYYY-MM-DD",
        "YYYY-MM-DD HH HH",
        "YYYY-MM-DD mm mm",
        "YYYY-MM-DD ss ss",
        "YYYY-MM-DD SSS SSS",
        "YYYY-MM-DD Z ZZ",
        "YYYY-MM-DD ddd ddd",
        "YYYY-DD-MM-DD",
    ] {
        let (args, _) = arg_error(call("parse", vec![Value::str("x"), Value::str(layout)]));
        assert!(args.contains("each field once"), "{layout:?}: {args}");
    }
    assert_eq!(
        code(call(
            "parse",
            vec![Value::str("2026-01-02 Mon"), Value::str("YYYY-MM-DD ddd")]
        )),
        "Err({kind: 'parse, message: \"cannot parse `2026-01-02 Mon` as a time: weekday Mon does not match 2026-01-02 (Fri)\"})"
    );
    let r = code(call(
        "parse",
        vec![
            Value::str("2026-01-02 fri Z"),
            Value::str("YYYY-MM-DD ddd Z"),
        ],
    ));
    assert!(r.starts_with("Err("), "{r}");
    let r = code(call(
        "parse",
        vec![
            Value::str("2026-01-02 fri +00:00"),
            Value::str("YYYY-MM-DD ddd Z"),
        ],
    ));
    assert_eq!(r, "Ok(2026-01-02T00:00:00Z)");
}

/// SP25 (D28): a format with no token, or with `%`, is E0403 for
/// `format` as for `parse`: it would print itself.
#[test]
fn format_needs_a_token_and_no_percent() {
    let t = Value::Time(Rc::new(Time::from_unix(0, 0, Loc::Utc).unwrap()));
    for (layout, help) in [
        ("%Y-%m-%d", true),
        ("YYYY%", true),
        ("hh:ii", false),
        ("", false),
    ] {
        let (args, h) = arg_error(call("format", vec![t.clone(), Value::str(layout)]));
        assert!(args.contains("at least one token"), "{layout:?}: {args}");
        assert_eq!(h.is_some(), help, "{layout:?}: {h:?}");
    }
    assert_eq!(
        format(&ok("format", vec![t.clone(), Value::str("COMMIT YYYY")])),
        "\"CO01IT 1970\""
    );
}

/// LB37: the range 0000 to 9999 is of the date a time shows (its wall
/// clock): a time whose shown date would leave it cannot be made, so
/// every time prints as text `time.parse` reads back.
#[test]
fn the_range_is_of_the_shown_date() {
    // The first and last seconds of years 0000 and 9999 in UTC.
    const MIN_UNIX: i64 = -62_167_219_200;
    const MAX_UNIX: i64 = 253_402_300_799;
    assert!(Time::from_unix(MAX_UNIX, 0, Loc::Fixed(9 * 3600)).is_none());
    assert!(Time::from_unix(MIN_UNIX, 0, Loc::Fixed(-5 * 3600)).is_none());
    assert!(Time::from_unix(MAX_UNIX - 9 * 3600, 0, Loc::Fixed(9 * 3600)).is_some());
    // Shown in an offset, it is in range; in UTC it is not.
    let late = ok(
        "parse",
        vec![Value::str("9999-12-31T23:59:59-05:00"), Value::Nil],
    );
    assert_eq!(format(&late), "Ok(9999-12-31T23:59:59-05:00)");
    let Value::Tag(tag) = &late else { panic!() };
    let (args, _) = arg_error(call("utc", vec![tag.args[0].clone()]));
    assert!(args.contains("years 0000 to 9999"), "{args}");
}
