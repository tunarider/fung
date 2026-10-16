//! Time values (02, `time`): an instant with its time zone, as Go's
//! `time.Time` holds one. fung has no time literal; `_native.time` makes
//! them.
//!
//! The representation is Go's wall-clock part of `time.Time`: seconds since
//! January 1, year 1 UTC (Go's `ext`, an `i64`) plus nanoseconds, and the
//! zone. Keeping Go's own fields, with Go's wrapping and saturating
//! arithmetic, gives the same range and the same answers at its edges
//! (`Add` saturating, `Sub` clamping, years beyond 9999 or before 0).
//! Go's monotonic clock reading is not kept: it only shows when the wall
//! clock jumps between two `now()` calls.

use std::cmp::Ordering;

use fung_front::diag::Diag;

use super::Value;
use super::arith::overflow;
use crate::native::tz::{
    self, INTERNAL_TO_UNIX, SECONDS_PER_DAY, UNIX_TO_ABSOLUTE, UNIX_TO_INTERNAL, Zone, abs_date,
};

const NANOS_PER_SEC: i64 = 1_000_000_000;

/// Where a time is shown (Go `*Location`). Zone data is process-static, so
/// a zone is a plain `&'static` reference and a time copies cheaply.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Loc {
    /// Go's `time.UTC`.
    Utc,
    /// A zone read from TZif data; the local zone is `tz::local()`.
    /// The offset is looked up at each instant.
    Zone(&'static Zone),
    /// An unnamed fixed offset in seconds east of UTC: Go's
    /// `time.FixedZone("", off)`, which is what parsing a numeric offset makes.
    Fixed(i32),
}

/// An instant and its zone (Go `time.Time` without the monotonic reading).
#[derive(Clone, Debug)]
pub struct Time {
    /// Seconds since January 1, year 1, 00:00:00 UTC (Go's `ext`).
    sec: i64,
    /// Nanoseconds within the second, `0..1e9`.
    nsec: u32,
    loc: Loc,
}

impl Time {
    /// Go `time.Unix(sec, nsec)` shown in `loc`: `nsec` outside `0..1e9`
    /// carries into the seconds.
    pub(crate) fn from_unix(mut sec: i64, mut nsec: i64, loc: Loc) -> Time {
        if !(0..NANOS_PER_SEC).contains(&nsec) {
            let n = nsec / NANOS_PER_SEC;
            sec = sec.wrapping_add(n);
            nsec -= n * NANOS_PER_SEC;
            if nsec < 0 {
                nsec += NANOS_PER_SEC;
                sec = sec.wrapping_sub(1);
            }
        }
        Time {
            sec: sec.wrapping_add(UNIX_TO_INTERNAL),
            nsec: nsec as u32,
            loc,
        }
    }

    /// The same instant shown in `loc` (Go `In`).
    pub(crate) fn in_loc(&self, loc: Loc) -> Time {
        Time { loc, ..*self }
    }

    /// Go `UTC()`.
    pub(crate) fn utc(&self) -> Time {
        self.in_loc(Loc::Utc)
    }

    /// Go `Local()`: the process's local zone (`tz::local()`).
    pub(crate) fn local(&self) -> Time {
        self.in_loc(Loc::Zone(tz::local()))
    }

    /// Go `Unix()`: seconds since 1970 UTC (wrapping like Go's `int64`).
    pub(crate) fn unix(&self) -> i64 {
        self.sec.wrapping_add(INTERNAL_TO_UNIX)
    }

    /// Go `Nanosecond()`.
    pub(crate) fn nanosecond(&self) -> u32 {
        self.nsec
    }

    /// Go `Zone()`: the abbreviation and offset (seconds east of UTC) in
    /// effect at this instant. A fixed offset has no name.
    pub(crate) fn zone(&self) -> (&str, i32) {
        match self.loc {
            Loc::Utc => ("UTC", 0),
            Loc::Zone(z) => z.lookup(self.unix()),
            Loc::Fixed(off) => ("", off),
        }
    }

    /// Go `absSec`: seconds since the absolute epoch, in local wall time.
    fn abs(&self) -> u64 {
        let (_, off) = self.zone();
        self.unix()
            .wrapping_add(i64::from(off))
            .wrapping_add(UNIX_TO_ABSOLUTE) as u64
    }

    /// Go `Date()`: year, month (1-based) and day in the zone.
    pub(crate) fn date(&self) -> (i64, i64, i64) {
        abs_date(self.abs() / SECONDS_PER_DAY as u64)
    }

    /// Go `Clock()`: hour, minute and second in the zone.
    pub(crate) fn clock(&self) -> (i64, i64, i64) {
        let s = (self.abs() % SECONDS_PER_DAY as u64) as i64;
        (s / 3600, s % 3600 / 60, s % 60)
    }

    /// Go `Weekday()`: 0 is Sunday.
    pub(crate) fn weekday(&self) -> u32 {
        // March 1 of the absolute year was a Wednesday (3).
        ((self.abs() / SECONDS_PER_DAY as u64 + 3) % 7) as u32
    }

    /// Go `addSec`: adds seconds, saturating instead of wrapping.
    pub(crate) fn add_sec(&mut self, d: i64) {
        let sum = self.sec.wrapping_add(d);
        self.sec = if (sum > self.sec) == (d > 0) {
            sum
        } else if d > 0 {
            i64::MAX
        } else {
            -i64::MAX
        };
    }

    /// Go `Add`: the instant `d` nanoseconds later, saturating at Go's range.
    pub(crate) fn add(&self, d: i64) -> Time {
        let mut dsec = d / NANOS_PER_SEC;
        let mut nsec = i64::from(self.nsec) + d % NANOS_PER_SEC;
        if nsec >= NANOS_PER_SEC {
            dsec += 1;
            nsec -= NANOS_PER_SEC;
        } else if nsec < 0 {
            dsec -= 1;
            nsec += NANOS_PER_SEC;
        }
        let mut t = Time {
            nsec: nsec as u32,
            ..*self
        };
        t.add_sec(dsec);
        t
    }

    /// Go `Sub`: `self - u` in nanoseconds, clamped to the `i64` range.
    pub(crate) fn sub(&self, u: &Time) -> i64 {
        let d = self
            .sec
            .wrapping_sub(u.sec)
            .wrapping_mul(NANOS_PER_SEC)
            .wrapping_add(i64::from(self.nsec) - i64::from(u.nsec));
        if u.add(d).equal(self) {
            d
        } else if self.before(u) {
            i64::MIN
        } else {
            i64::MAX
        }
    }

    /// Go `Equal`: the same instant, whatever the zones.
    pub(crate) fn equal(&self, u: &Time) -> bool {
        self.sec == u.sec && self.nsec == u.nsec
    }

    /// Go `Before`.
    pub(crate) fn before(&self, u: &Time) -> bool {
        self.cmp_instant(u) == Ordering::Less
    }

    /// Go `After`.
    pub(crate) fn after(&self, u: &Time) -> bool {
        self.cmp_instant(u) == Ordering::Greater
    }

    /// Orders by instant only (Go `Compare`).
    pub(crate) fn cmp_instant(&self, u: &Time) -> Ordering {
        (self.sec, self.nsec).cmp(&(u.sec, u.nsec))
    }

    /// RFC 3339 with fractional seconds only when they are not zero
    /// (Go `Format(time.RFC3339Nano)`, 06: 시간 형식). Years outside
    /// 0..=9999 print with as many digits as they need, and the offset
    /// drops its seconds.
    pub fn iso(&self) -> String {
        let (year, month, day) = self.date();
        let (hour, min, sec) = self.clock();
        let mut b = String::with_capacity(35);
        push_int(&mut b, year, 4);
        b.push('-');
        push_int(&mut b, month, 2);
        b.push('-');
        push_int(&mut b, day, 2);
        b.push('T');
        push_int(&mut b, hour, 2);
        b.push(':');
        push_int(&mut b, min, 2);
        b.push(':');
        push_int(&mut b, sec, 2);
        if self.nsec != 0 {
            let frac = format!("{:09}", self.nsec);
            b.push('.');
            b.push_str(frac.trim_end_matches('0'));
        }
        let (_, offset) = self.zone();
        if offset == 0 {
            b.push('Z');
            return b;
        }
        let mut zone = offset / 60; // minutes, truncated toward zero
        if zone < 0 {
            b.push('-');
            zone = -zone;
        } else {
            b.push('+');
        }
        push_int(&mut b, i64::from(zone / 60), 2);
        b.push(':');
        push_int(&mut b, i64::from(zone % 60), 2);
        b
    }
}

/// Go `appendInt`: a sign for negatives, then at least `width` digits.
fn push_int(b: &mut String, x: i64, width: usize) {
    if x < 0 {
        b.push('-');
    }
    b.push_str(&format!("{:0width$}", x.unsigned_abs()));
}

/// `time + dur` and `time - dur` (07): an error when Go's saturating `Add`
/// could not move the instant.
pub(super) fn add_time_dur(t: &Time, d: i64) -> Result<Value, Diag> {
    let r = t.add(d);
    if (d > 0 && !r.after(t)) || (d < 0 && !r.before(t)) {
        return Err(overflow());
    }
    Ok(Value::Time(r.into()))
}

/// `time - time` → dur. Go's `Sub` clamps at the dur range; the clamped
/// values are an overflow here (07: 범위를 넘으면 에러).
pub(super) fn sub_times(a: &Time, b: &Time) -> Result<Value, Diag> {
    let d = a.sub(b);
    if d == i64::MAX || d == i64::MIN {
        return Err(overflow());
    }
    Ok(Value::Dur(d))
}
