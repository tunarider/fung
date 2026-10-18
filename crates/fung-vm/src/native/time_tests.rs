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
    let r = (n.func)(&mut rt.task(), args);
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
    assert_eq!(n.iso(), n.local().iso(), "now is local");

    let r = ok("local", vec![ok("utc", vec![at.clone()])]);
    let Value::Time(l) = &r else { panic!() };
    assert!(equal(&r, &at));
    let Value::Time(a) = &at else { panic!() };
    assert_eq!(l.iso(), a.local().iso(), "local: {}", format(&r));

    assert_eq!(
        code(call("format", vec![Value::Int(1), Value::Nil])),
        "E0403"
    );
    assert_eq!(code(call("parse", vec![iso, Value::Int(1)])), "E0403");
    // Years 0000 to 9999.
    let (min, max) = (crate::value::MIN_UNIX, crate::value::MAX_UNIX);
    assert_eq!(code(call("from_unix", vec![Value::Int(max + 1)])), "E0403");
    assert_eq!(code(call("from_unix", vec![Value::Int(min - 1)])), "E0403");
    ok("from_unix", vec![Value::Int(max)]);
    ok("from_unix", vec![Value::Int(min)]);
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
