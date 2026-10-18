//! `_native.time` (06): instants and the
//! token formats. The `time` type itself is core (`crate::value::Time`);
//! this module only makes and renders values. Times are pure values, so
//! everything here runs under the execution right like any computation.

use std::rc::Rc;
use std::sync::OnceLock;

use fung_front::diag::Arg;

use super::nat::{self, native, opt, param};
use super::timefmt::{format_time, parse_rfc3339, parse_time};
use super::tz;
use crate::value::{Lit, Loc, MapBuilder, Module, Native, Time, Value, format, intern};
use crate::vm::{Stop, Task};

/// Made once per process: natives are leaked (`Native::leak`).
static NATIVES: OnceLock<Vec<(&'static str, &'static Native)>> = OnceLock::new();

/// The `_native.time` module.
pub fn module() -> Rc<Module> {
    let natives = NATIVES.get_or_init(|| {
        vec![
            ("now", native("now", now_fn, vec![])),
            (
                "parse",
                native("parse", parse_fn, vec![param("s"), opt("format", Lit::Nil)]),
            ),
            (
                "format",
                native(
                    "format",
                    format_fn,
                    vec![param("t"), opt("format", Lit::Nil)],
                ),
            ),
            ("utc", native("utc", utc_fn, vec![param("t")])),
            ("local", native("local", local_fn, vec![param("t")])),
            ("unix", native("unix", unix_fn, vec![param("t")])),
            (
                "from_unix",
                native("from_unix", from_unix_fn, vec![param("n")]),
            ),
            ("parts", native("parts", parts_fn, vec![param("t")])),
        ]
    });
    let mut m = Module::new("time", "_native.time");
    for (name, n) in natives {
        m.set(name, Value::Native(n));
    }
    Rc::new(m)
}

fn need_time<'a>(name: &str, v: &'a Value) -> Result<&'a Time, Stop> {
    match v {
        Value::Time(t) => Ok(t),
        _ => Err(nat::wrong_arg(name, "time", v)),
    }
}

fn time(t: Time) -> Value {
    Value::Time(Rc::new(t))
}

/// The system clock now, in the local zone (no monotonic reading; see
/// `crate::value::Time`).
fn now_fn(_: &mut Task<'_>, _: Vec<Value>) -> Result<Value, Stop> {
    let (sec, nsec) = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, i64::from(d.subsec_nanos())),
        Err(e) => {
            let d = e.duration();
            (-(d.as_secs() as i64), -i64::from(d.subsec_nanos()))
        }
    };
    match Time::from_unix(sec, nsec, Loc::Zone(tz::local())) {
        Some(t) => Ok(time(t)),
        None => Err(nat::fail(
            "E0801",
            vec![nat::s("the system clock is outside years 0000 to 9999")],
        )),
    }
}

fn parse_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let s = nat::need_str("s", &a[0])?;
    let layout = nat::opt_str("format", &a[1])?;
    let r = match layout {
        Some(layout) => parse_time(s, layout, tz::local()),
        None => parse_rfc3339(s),
    };
    Ok(match r {
        Ok(t) => nat::ok(time(t)),
        Err(e) => nat::err_value(
            "parse",
            &format!("cannot parse `{s}` as a time: {e}"),
            vec![],
        ),
    })
}

fn format_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let t = need_time("t", &a[0])?;
    Ok(Value::str(&match nat::opt_str("format", &a[1])? {
        Some(layout) => format_time(t, layout),
        None => t.iso(),
    }))
}

fn utc_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(time(need_time("t", &a[0])?.utc()))
}

fn local_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(time(need_time("t", &a[0])?.local()))
}

fn unix_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    Ok(Value::Int(need_time("t", &a[0])?.unix()))
}

fn from_unix_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let n = nat::need_int("n", &a[0])?;
    match Time::from_unix(n, 0, Loc::Zone(tz::local())) {
        Some(t) => Ok(time(t)),
        None => Err(nat::fail(
            "E0403",
            vec![
                nat::s("n"),
                nat::s("a unix time from year 0000 to 9999"),
                Arg::Str(format(&a[0])),
            ],
        )),
    }
}

const WEEKDAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

fn parts_fn(_: &mut Task<'_>, a: Vec<Value>) -> Result<Value, Stop> {
    let t = need_time("t", &a[0])?;
    let (year, month, day) = t.date();
    let (hour, minute, second) = t.clock();
    let mut m = MapBuilder::default();
    for (k, v) in [
        ("year", year),
        ("month", month),
        ("day", day),
        ("hour", hour),
        ("minute", minute),
        ("second", second),
    ] {
        m.set(Rc::new(k.to_string()), Value::Int(v));
    }
    m.set(
        Rc::new("weekday".to_string()),
        Value::Sym(intern(WEEKDAYS[t.weekday() as usize])),
    );
    Ok(Value::Map(Rc::new(m.finish())))
}

#[cfg(test)]
#[path = "time_tests.rs"]
mod tests;
