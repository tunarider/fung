//! Time values (02, `time`): an instant, seconds since 1970 UTC and
//! nanoseconds, with the zone it is shown in. fung has no time literal;
//! `_native.time` makes them. Instants run from year 0000 to 9999 (UTC);
//! arithmetic past that is an overflow (E0503). The wall clock has no
//! monotonic reading: two `now()` calls straddling a clock change differ
//! by the change.

use std::cmp::Ordering;

use fung_front::diag::Diag;

use super::Value;
use super::arith::overflow;
use crate::native::tz::{self, Zone};

const NANOS_PER_SEC: i64 = 1_000_000_000;
const SECONDS_PER_DAY: i64 = 86_400;

/// The first second of year 0000 (UTC).
pub(crate) const MIN_UNIX: i64 = -62_167_219_200;
/// The last second of year 9999 (UTC).
pub(crate) const MAX_UNIX: i64 = 253_402_300_799;

/// Where a time is shown. Zone data is process-static, so
/// a zone is a plain `&'static` reference and a time copies cheaply.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Loc {
    Utc,
    /// A zone read from TZif data; the local zone is `tz::local()`.
    /// The offset is looked up at each instant.
    Zone(&'static Zone),
    /// An unnamed fixed offset in seconds east of UTC: what parsing a
    /// numeric offset makes.
    Fixed(i32),
}

/// An instant and its zone.
#[derive(Clone, Debug)]
pub struct Time {
    /// Seconds since 1970-01-01T00:00:00Z, `MIN_UNIX..=MAX_UNIX`.
    unix: i64,
    /// Nanoseconds within the second, `0..1e9`.
    nsec: u32,
    loc: Loc,
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's
/// `days_from_civil`).
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day since 1970-01-01 (H. Hinnant's `civil_from_days`).
pub(crate) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

impl Time {
    /// The instant `sec` seconds and `nsec` nanoseconds after 1970 UTC
    /// (`nsec` outside `0..1e9` carries into the seconds), or `None`
    /// outside years 0000 to 9999.
    pub(crate) fn from_unix(sec: i64, nsec: i64, loc: Loc) -> Option<Time> {
        let sec = sec.checked_add(nsec.div_euclid(NANOS_PER_SEC))?;
        let nsec = nsec.rem_euclid(NANOS_PER_SEC) as u32;
        (MIN_UNIX..=MAX_UNIX).contains(&sec).then_some(Time {
            unix: sec,
            nsec,
            loc,
        })
    }

    /// The instant whose wall clock in `loc` reads the given fields (they
    /// must be in range). A zone resolves wall times it skips or repeats
    /// (`Zone::date`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_wall(
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        min: i64,
        sec: i64,
        nsec: i64,
        loc: Loc,
    ) -> Option<Time> {
        let wall =
            days_from_civil(year, month, day) * SECONDS_PER_DAY + hour * 3600 + min * 60 + sec;
        let unix = match loc {
            Loc::Utc => wall,
            Loc::Fixed(off) => wall - i64::from(off),
            Loc::Zone(z) => z.date(wall),
        };
        Time::from_unix(unix, nsec, loc)
    }

    /// The same instant shown in `loc`.
    pub(crate) fn in_loc(&self, loc: Loc) -> Time {
        Time { loc, ..*self }
    }

    pub(crate) fn utc(&self) -> Time {
        self.in_loc(Loc::Utc)
    }

    /// The same instant in the process's local zone (`tz::local()`).
    pub(crate) fn local(&self) -> Time {
        self.in_loc(Loc::Zone(tz::local()))
    }

    /// Seconds since 1970 UTC.
    pub(crate) fn unix(&self) -> i64 {
        self.unix
    }

    pub(crate) fn nanosecond(&self) -> u32 {
        self.nsec
    }

    /// The abbreviation and offset (seconds east of UTC) in effect at this
    /// instant. A fixed offset has no name.
    pub(crate) fn zone(&self) -> (&str, i32) {
        match self.loc {
            Loc::Utc => ("UTC", 0),
            Loc::Zone(z) => z.lookup(self.unix),
            Loc::Fixed(off) => ("", off),
        }
    }

    /// Seconds since 1970 on the wall clock of the zone.
    fn wall(&self) -> i64 {
        self.unix + i64::from(self.zone().1)
    }

    /// Year, month (1-based) and day in the zone.
    pub(crate) fn date(&self) -> (i64, i64, i64) {
        civil_from_days(self.wall().div_euclid(SECONDS_PER_DAY))
    }

    /// Hour, minute and second in the zone.
    pub(crate) fn clock(&self) -> (i64, i64, i64) {
        let s = self.wall().rem_euclid(SECONDS_PER_DAY);
        (s / 3600, s % 3600 / 60, s % 60)
    }

    /// The day of the week in the zone; 0 is Sunday.
    pub(crate) fn weekday(&self) -> u32 {
        // 1970-01-01 was a Thursday.
        (self.wall().div_euclid(SECONDS_PER_DAY) + 4).rem_euclid(7) as u32
    }

    /// The instant `d` nanoseconds later, or `None` past the range.
    pub(crate) fn add(&self, d: i128) -> Option<Time> {
        let total = i128::from(self.nsec) + d;
        let sec = i128::from(self.unix) + total.div_euclid(i128::from(NANOS_PER_SEC));
        let nsec = total.rem_euclid(i128::from(NANOS_PER_SEC)) as i64;
        Time::from_unix(i64::try_from(sec).ok()?, nsec, self.loc)
    }

    /// `self - u` in nanoseconds, or `None` when it does not fit a dur.
    pub(crate) fn sub(&self, u: &Time) -> Option<i64> {
        let d = (i128::from(self.unix) - i128::from(u.unix)) * i128::from(NANOS_PER_SEC)
            + (i128::from(self.nsec) - i128::from(u.nsec));
        i64::try_from(d).ok()
    }

    /// The same instant, whatever the zones.
    /// The instant, whatever the zone (what `==` compares).
    pub(crate) fn instant(&self) -> (i64, u32) {
        (self.unix, self.nsec)
    }

    pub(crate) fn equal(&self, u: &Time) -> bool {
        self.unix == u.unix && self.nsec == u.nsec
    }

    /// Orders by instant only.
    pub(crate) fn cmp_instant(&self, u: &Time) -> Ordering {
        (self.unix, self.nsec).cmp(&(u.unix, u.nsec))
    }

    /// RFC 3339 with fractional seconds only when they are not zero (06:
    /// 시간 형식). The offset is `Z`, `±hh:mm`, or `±hh:mm:ss` when it has
    /// seconds; a year out of 0000..9999 (only at the range's edges, shown
    /// in an offset) keeps its sign and digits.
    pub fn iso(&self) -> String {
        let (year, month, day) = self.date();
        let (hour, min, sec) = self.clock();
        let mut b = String::with_capacity(35);
        if year < 0 {
            b.push('-');
        }
        b.push_str(&format!(
            "{:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}",
            year.unsigned_abs()
        ));
        if self.nsec != 0 {
            let frac = format!("{:09}", self.nsec);
            b.push('.');
            b.push_str(frac.trim_end_matches('0'));
        }
        let (_, offset) = self.zone();
        if offset == 0 {
            b.push('Z');
        } else {
            b.push_str(&offset_text(offset, true));
        }
        b
    }
}

/// `±hh:mm` (`colon`) or `±hhmm`, then `:ss` (or `ss`) when the offset has
/// seconds.
pub(crate) fn offset_text(offset: i32, colon: bool) -> String {
    let sign = if offset < 0 { '-' } else { '+' };
    let a = offset.unsigned_abs();
    let sep = if colon { ":" } else { "" };
    let mut t = format!("{sign}{:02}{sep}{:02}", a / 3600, a % 3600 / 60);
    if !a.is_multiple_of(60) {
        t.push_str(&format!("{sep}{:02}", a % 60));
    }
    t
}

/// `time + dur` (07): an overflow past the range.
pub(super) fn add_time_dur(t: &Time, d: i64) -> Result<Value, Diag> {
    let r = t.add(i128::from(d)).ok_or_else(overflow)?;
    Ok(Value::Time(r.into()))
}

/// `time - dur` (07): an overflow past the range.
pub(super) fn sub_time_dur(t: &Time, d: i64) -> Result<Value, Diag> {
    let r = t.add(-i128::from(d)).ok_or_else(overflow)?;
    Ok(Value::Time(r.into()))
}

/// `time - time` → dur: an overflow when the difference does not fit.
pub(super) fn sub_times(a: &Time, b: &Time) -> Result<Value, Diag> {
    Ok(Value::Dur(a.sub(b).ok_or_else(overflow)?))
}
