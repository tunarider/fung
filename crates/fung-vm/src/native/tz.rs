//! Time zones read from TZif files, as Go's `time` package reads them
//! (`zoneinfo_read.go`, `zoneinfo.go`, `zoneinfo_unix.go`).
//!
//! fung only ever uses the local zone (`time.Local` in Go), resolved once
//! per process from `TZ` the way Go's `initLocal` does. A [`Zone`] holds
//! plain Rust data (no `Rc`), so any thread may read the shared one.
//!
//! The arithmetic follows Go's 64-bit `int` and `uint64` code with wrapping
//! operations, so even absurd instants give the answer Go gives.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::sync::OnceLock;

/// One zone type of a TZif file (Go `zone`): `"CET"`, seconds east of UTC.
#[derive(Clone, Debug)]
struct ZoneType {
    name: String,
    offset: i32,
    is_dst: bool,
}

/// A transition (Go `zoneTrans`): from `when` on, zone type `index` applies.
#[derive(Clone, Copy, Debug)]
struct Trans {
    when: i64,
    index: u8,
}

/// A time zone (Go `Location`).
#[derive(Clone, Debug)]
pub(crate) struct Zone {
    /// Go's `Location.String()`: `"Local"`, `"UTC"`, `"Asia/Seoul"` or a path.
    pub(crate) name: String,
    zones: Vec<ZoneType>,
    tx: Vec<Trans>,
    /// The POSIX TZ rule after the TZif data, for instants past the last
    /// transition (`EST5EDT,M3.2.0,M11.1.0`), parsed; `None` when the file
    /// has none or Go's `tzset` would not accept it.
    extend: Option<Footer>,
}

/// Go's `alpha` and `omega`: the beginning and end of time for transitions.
const ALPHA: i64 = i64::MIN;
const OMEGA: i64 = i64::MAX;

const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_HOUR: i64 = 60 * SECONDS_PER_MINUTE;
pub(crate) const SECONDS_PER_DAY: i64 = 24 * SECONDS_PER_HOUR;

/// Days from March 1 through the end of the year.
const MARCH_THRU_DECEMBER: i64 = 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 30 + 31;
/// Go's `absoluteYears`: the absolute epoch is March 1 of this many years before year 0.
const ABSOLUTE_YEARS: u64 = 292277022400;
/// Go's `absoluteToInternal` (`-(absoluteYears*365.2425 + marchThruDecember) * secondsPerDay`).
const ABSOLUTE_TO_INTERNAL: i64 =
    -((ABSOLUTE_YEARS as i64 / 400 * 146097 + MARCH_THRU_DECEMBER) * SECONDS_PER_DAY);
const INTERNAL_TO_ABSOLUTE: i64 = -ABSOLUTE_TO_INTERNAL;
/// Go's `unixToInternal`: seconds from January 1, year 1 to the Unix epoch.
pub(crate) const UNIX_TO_INTERNAL: i64 =
    (1969 * 365 + 1969 / 4 - 1969 / 100 + 1969 / 400) * SECONDS_PER_DAY;
pub(crate) const INTERNAL_TO_UNIX: i64 = -UNIX_TO_INTERNAL;
/// Go's `absoluteToUnix`.
const ABSOLUTE_TO_UNIX: i64 = ABSOLUTE_TO_INTERNAL + INTERNAL_TO_UNIX;
/// Go's `unixToAbsolute`.
pub(crate) const UNIX_TO_ABSOLUTE: i64 = UNIX_TO_INTERNAL + INTERNAL_TO_ABSOLUTE;

impl Zone {
    /// Go's `UTC` location (no zone types: every lookup is `UTC`, offset 0).
    pub(crate) fn utc() -> Zone {
        Zone {
            name: "UTC".to_string(),
            zones: Vec::new(),
            tx: Vec::new(),
            extend: None,
        }
    }

    /// The zone abbreviation and offset (seconds east of UTC) in effect at
    /// `unix_sec` (Go `Location.lookup`, name and offset only).
    pub(crate) fn lookup(&self, unix_sec: i64) -> (&str, i32) {
        let l = self.lookup_full(unix_sec);
        (l.name, l.offset)
    }

    /// Go `Location.lookup`: the zone at `sec` and the instants `[start, end)`
    /// bracketing it. The bracket is exact near a transition, but under the
    /// POSIX rule it may otherwise be only the start and end of the year
    /// (which is all `date` needs).
    fn lookup_full(&self, sec: i64) -> Lookup<'_> {
        if self.zones.is_empty() {
            return Lookup {
                name: "UTC",
                offset: 0,
                start: ALPHA,
                end: OMEGA,
            };
        }
        if self.tx.is_empty() || sec < self.tx[0].when {
            let z = &self.zones[self.lookup_first_zone()];
            let end = self.tx.first().map_or(OMEGA, |t| t.when);
            return Lookup {
                name: &z.name,
                offset: z.offset,
                start: ALPHA,
                end,
            };
        }
        // Binary search for the entry with the largest time <= sec.
        let tx = &self.tx;
        let mut end = OMEGA;
        let mut lo = 0;
        let mut hi = tx.len();
        while hi - lo > 1 {
            let m = (lo + hi) / 2;
            let lim = tx[m].when;
            if sec < lim {
                end = lim;
                hi = m;
            } else {
                lo = m;
            }
        }
        let z = &self.zones[tx[lo].index as usize];
        let start = tx[lo].when;
        // At the end of the known transitions, try the POSIX rule.
        if lo == tx.len() - 1
            && let Some(f) = &self.extend
        {
            return tzset(f, start, sec);
        }
        Lookup {
            name: &z.name,
            offset: z.offset,
            start,
            end,
        }
    }

    /// Go `Location.lookupFirstZone`: the zone type for instants before the
    /// first transition (the tzcode `localtime.c` rules).
    fn lookup_first_zone(&self) -> usize {
        // Case 1: the first zone is unused by the transitions.
        if !self.tx.iter().any(|t| t.index == 0) {
            return 0;
        }
        // Case 2: the first transition is to DST; find the closest earlier standard zone.
        if let Some(first) = self.tx.first()
            && self.zones[first.index as usize].is_dst
        {
            for zi in (0..first.index as usize).rev() {
                if !self.zones[zi].is_dst {
                    return zi;
                }
            }
        }
        // Case 3: the first standard zone; case 4: the first zone.
        self.zones.iter().position(|z| !z.is_dst).unwrap_or(0)
    }

    /// The instant Go's `time.Date(year, month, day, hour, min, sec, nsec, loc)`
    /// chooses in this zone, as `(unix seconds, nanoseconds)`.
    ///
    /// Out-of-range fields are normalized (month 13 is January of the next
    /// year, second -1 is the last second of the previous minute). A wall
    /// time that the zone skips or repeats at a DST change gets Go's choice:
    /// the offset in effect just before the change is tried first, and kept
    /// unless the resulting instant falls outside that offset's span — so a
    /// skipped time (02:30 on a spring-forward day in New York) lands an
    /// hour later in UTC terms than read with the new offset, and a repeated
    /// time (01:30 on a fall-back day) takes the first, DST occurrence.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn date(
        &self,
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        min: i64,
        sec: i64,
        nsec: i64,
    ) -> (i64, i32) {
        // Normalize month, overflowing into year.
        let (year, m) = norm(year, month.wrapping_sub(1), 12);
        let month = m + 1;
        // Normalize nsec, sec, min, hour, overflowing into day.
        let (sec, nsec) = norm(sec, nsec, 1_000_000_000);
        let (min, sec) = norm(min, sec, 60);
        let (hour, min) = norm(hour, min, 60);
        let (day, hour) = norm(day, hour, 24);

        let mut unix = (date_to_abs_days(year, month, day) as i64)
            .wrapping_mul(SECONDS_PER_DAY)
            .wrapping_add(
                hour.wrapping_mul(SECONDS_PER_HOUR)
                    .wrapping_add(min.wrapping_mul(SECONDS_PER_MINUTE))
                    .wrapping_add(sec),
            )
            .wrapping_add(ABSOLUTE_TO_UNIX);

        // Look up the offset for the wall time read as UTC, then check that
        // the resulting instant is still in that offset's span.
        let l = self.lookup_full(unix);
        let mut offset = l.offset;
        if offset != 0 {
            let utc = unix.wrapping_sub(i64::from(offset));
            if utc < l.start || utc >= l.end {
                offset = self.lookup_full(utc).offset;
            }
            unix = unix.wrapping_sub(i64::from(offset));
        }
        (unix, nsec as i32)
    }
}

/// What Go's `lookup` returns, minus `isDST` (fung never shows it).
struct Lookup<'a> {
    name: &'a str,
    offset: i32,
    start: i64,
    end: i64,
}

/// Go `norm`: `(nhi, nlo)` with `hi*base + lo == nhi*base + nlo`, `0 <= nlo < base`.
fn norm(mut hi: i64, mut lo: i64, base: i64) -> (i64, i64) {
    if lo < 0 {
        let n = lo.wrapping_neg().wrapping_sub(1) / base + 1;
        hi = hi.wrapping_sub(n);
        lo = lo.wrapping_add(n.wrapping_mul(base));
    }
    if lo >= base {
        let n = lo / base;
        hi = hi.wrapping_add(n);
        lo = lo.wrapping_sub(n.wrapping_mul(base));
    }
    (hi, lo)
}

/// Go `dateToAbsDays`: days from the absolute epoch to `year-month-day`
/// (`month` in 1..=12; `day` may be out of range, even negative).
pub(crate) fn date_to_abs_days(year: i64, month: i64, day: i64) -> u64 {
    let mut amonth = month as u32;
    let jan_feb = u32::from(amonth < 3);
    amonth += 12 * jan_feb;
    let y = (year as u64)
        .wrapping_sub(u64::from(jan_feb))
        .wrapping_add(ABSOLUTE_YEARS);
    let ayday = (979 * amonth - 2919) >> 5;
    let century = y / 100;
    let cyear = (y % 100) as u32;
    let cday = 1461 * cyear / 4;
    let centurydays = 146097u64.wrapping_mul(century) / 4;
    centurydays.wrapping_add((i64::from(cday + ayday).wrapping_add(day).wrapping_sub(1)) as u64)
}

/// Go `absDays.split`: century, year in the century, day in the March-based year.
fn split_days(days: u64) -> (u64, u32, u32) {
    let d = days.wrapping_mul(4).wrapping_add(3);
    let century = d / 146097;
    let cd = (d % 146097) as u32 | 3;
    let prod = 2939745u64 * u64::from(cd);
    let cyear = (prod >> 32) as u32;
    let ayday = (prod as u32) / 2939745 / 4;
    (century, cyear, ayday)
}

/// Go `absDays.yearYday`: the standard year and 1-based day of the year.
pub(crate) fn year_yday(days: u64) -> (i64, i64) {
    let (century, cyear, ayday) = split_days(days);
    let jan_feb = i64::from(i64::from(ayday) >= MARCH_THRU_DECEMBER);
    let year = (century.wrapping_mul(100).wrapping_sub(ABSOLUTE_YEARS) as i64)
        .wrapping_add(i64::from(cyear))
        .wrapping_add(jan_feb);
    let leap = i64::from(cyear % 4 == 0 && (cyear != 0 || century % 4 == 0));
    let yday = i64::from(ayday) + (1 + 31 + 28) + (leap & !jan_feb) - 365 * jan_feb;
    (year, yday)
}

/// Go `absDays.date`: the standard year, month (1-based) and day.
pub(crate) fn abs_date(days: u64) -> (i64, i64, i64) {
    let (century, cyear, ayday) = split_days(days);
    // Go `absYday.split`.
    let d = 2141 * ayday + 197913;
    let (amonth, mday) = (i64::from(d >> 16), 1 + i64::from((d & 0xFFFF) / 2141));
    let jan_feb = i64::from(i64::from(ayday) >= MARCH_THRU_DECEMBER);
    let year = (century.wrapping_mul(100).wrapping_sub(ABSOLUTE_YEARS) as i64)
        .wrapping_add(i64::from(cyear))
        .wrapping_add(jan_feb);
    (year, amonth - 12 * jan_feb, mday)
}

/// Go `isLeap`.
fn is_leap(year: i64) -> bool {
    let mask = if year % 25 != 0 { 3 } else { 0xf };
    year & mask == 0
}

/// Go `daysBefore`: days in a non-leap year before month `m` (1-based).
fn days_before(m: i64) -> i64 {
    let adj = if m >= 3 { -2 } else { 0 };
    (214 * m - 211) / 7 + adj
}

/// Go `daysIn`.
fn days_in(m: i64, year: i64) -> i64 {
    if m == 2 {
        return if is_leap(year) { 29 } else { 28 };
    }
    30 + ((m + (m >> 3)) & 1)
}

// --- POSIX TZ rules (Go `tzset` and helpers) ---

/// A rule from a POSIX TZ string (Go `rule`).
#[derive(Clone, Copy, Debug)]
enum RuleKind {
    /// `Jn`: day 1..=365, February 29 never counted.
    Julian,
    /// `n`: day 0..=365, February 29 counted.
    Doy,
    /// `Mm.w.d`: day `d` of week `w` of month `m`.
    MonthWeekDay,
}

#[derive(Clone, Copy, Debug)]
struct Rule {
    kind: RuleKind,
    day: i64,
    week: i64,
    mon: i64,
    /// Seconds after local midnight.
    time: i64,
}

/// A parsed POSIX TZ footer. Go reparses the string on every lookup; the
/// result is the same each time, so it is parsed once here. Names are
/// decoded lossily only after parsing, which works on bytes as Go's does.
#[derive(Clone, Debug)]
struct Footer {
    std_name: String,
    /// Seconds east of UTC (already negated from the POSIX sign).
    std_offset: i64,
    dst: Option<DstRule>,
}

#[derive(Clone, Debug)]
struct DstRule {
    name: String,
    offset: i64,
    start: Rule,
    end: Rule,
}

/// The parsing half of Go `tzset`. `None` where Go's `tzset` returns
/// `ok == false` (the footer is then ignored).
fn parse_footer(s: &[u8]) -> Option<Footer> {
    let (std_name, s) = tzset_name(s)?;
    let (std_offset, s) = tzset_offset(s)?;
    // POSIX offsets are added to local time to get UTC; ours are the reverse.
    let std_offset = -std_offset;
    let std_name = String::from_utf8_lossy(std_name).into_owned();
    if s.is_empty() || s[0] == b',' {
        // No daylight saving time.
        return Some(Footer {
            std_name,
            std_offset,
            dst: None,
        });
    }
    let (dst_name, mut s) = tzset_name(s)?;
    let dst_offset;
    if s.is_empty() || s[0] == b',' {
        dst_offset = std_offset + SECONDS_PER_HOUR;
    } else {
        let (o, rest) = tzset_offset(s)?;
        dst_offset = -o;
        s = rest;
    }
    if s.is_empty() {
        // Default DST rules per tzcode.
        s = b",M3.2.0,M11.1.0";
    }
    // POSIX does not mention ';' here but tzcode accepts it.
    if s[0] != b',' && s[0] != b';' {
        return None;
    }
    let (start, s) = tzset_rule(&s[1..])?;
    if s.is_empty() || s[0] != b',' {
        return None;
    }
    let (end, s) = tzset_rule(&s[1..])?;
    if !s.is_empty() {
        return None;
    }
    let dst = DstRule {
        name: String::from_utf8_lossy(dst_name).into_owned(),
        offset: dst_offset,
        start,
        end,
    };
    Some(Footer {
        std_name,
        std_offset,
        dst: Some(dst),
    })
}

/// The evaluating half of Go `tzset`: the zone at `sec` under footer `f`,
/// given the last transition `last_tx_sec`.
fn tzset(f: &Footer, last_tx_sec: i64, sec: i64) -> Lookup<'_> {
    let Some(dst) = &f.dst else {
        return Lookup {
            name: &f.std_name,
            offset: f.std_offset as i32,
            start: last_tx_sec,
            end: OMEGA,
        };
    };
    let (mut std_name, mut std_offset) = (f.std_name.as_str(), f.std_offset);
    let (mut dst_name, mut dst_offset) = (dst.name.as_str(), dst.offset);

    // The start of the year containing sec, and seconds since then.
    let abs = sec.wrapping_add(UNIX_TO_ABSOLUTE) as u64;
    let (year, yday) = year_yday(abs / SECONDS_PER_DAY as u64);
    let ysec = (yday - 1)
        .wrapping_mul(SECONDS_PER_DAY)
        .wrapping_add(sec % SECONDS_PER_DAY);
    let ystart = sec.wrapping_sub(ysec);

    let mut start_sec = tzrule_time(year, dst.start, std_offset);
    let mut end_sec = tzrule_time(year, dst.end, dst_offset);
    // Southern hemisphere: the rule's "DST" spans the year boundary, so
    // swap the halves while keeping Go's labels.
    if end_sec < start_sec {
        std::mem::swap(&mut start_sec, &mut end_sec);
        std::mem::swap(&mut std_name, &mut dst_name);
        std::mem::swap(&mut std_offset, &mut dst_offset);
    }

    if ysec < start_sec {
        Lookup {
            name: std_name,
            offset: std_offset as i32,
            start: ystart,
            end: start_sec.wrapping_add(ystart),
        }
    } else if ysec >= end_sec {
        Lookup {
            name: std_name,
            offset: std_offset as i32,
            start: end_sec.wrapping_add(ystart),
            end: ystart.wrapping_add(365 * SECONDS_PER_DAY),
        }
    } else {
        Lookup {
            name: dst_name,
            offset: dst_offset as i32,
            start: start_sec.wrapping_add(ystart),
            end: end_sec.wrapping_add(ystart),
        }
    }
}

/// Go `tzsetName`: a zone name (`EST`, or `<+0330>` without the brackets).
/// Go ranges over runes, but every byte it tests for is ASCII, so scanning
/// bytes finds the same positions.
fn tzset_name(s: &[u8]) -> Option<(&[u8], &[u8])> {
    if s.is_empty() {
        return None;
    }
    if s[0] != b'<' {
        for (i, &b) in s.iter().enumerate() {
            if matches!(b, b'0'..=b'9' | b',' | b'-' | b'+') {
                if i < 3 {
                    return None;
                }
                return Some((&s[..i], &s[i..]));
            }
        }
        if s.len() < 3 {
            return None;
        }
        Some((s, b""))
    } else {
        let i = s.iter().position(|&b| b == b'>')?;
        Some((&s[1..i], &s[i + 1..]))
    }
}

/// Go `tzsetOffset`: `[+-]hh[:mm[:ss]]` in seconds.
fn tzset_offset(s: &[u8]) -> Option<(i64, &[u8])> {
    if s.is_empty() {
        return None;
    }
    let (neg, s) = match s[0] {
        b'+' => (false, &s[1..]),
        b'-' => (true, &s[1..]),
        _ => (false, s),
    };
    let sign = |off: i64| if neg { -off } else { off };
    // tzdata permits hours up to 24*7, although POSIX does not.
    let (hours, s) = tzset_num(s, 0, 24 * 7)?;
    let mut off = hours * SECONDS_PER_HOUR;
    let Some(s) = s.strip_prefix(b":") else {
        return Some((sign(off), s));
    };
    let (mins, s) = tzset_num(s, 0, 59)?;
    off += mins * SECONDS_PER_MINUTE;
    let Some(s) = s.strip_prefix(b":") else {
        return Some((sign(off), s));
    };
    let (secs, s) = tzset_num(s, 0, 59)?;
    off += secs;
    Some((sign(off), s))
}

/// Go `tzsetRule`: `Jn`, `n` or `Mm.w.d`, then an optional `/time`.
fn tzset_rule(s: &[u8]) -> Option<(Rule, &[u8])> {
    if s.is_empty() {
        return None;
    }
    let mut r = Rule {
        kind: RuleKind::Doy,
        day: 0,
        week: 0,
        mon: 0,
        time: 0,
    };
    let mut s = s;
    if let Some(rest) = s.strip_prefix(b"J") {
        let (jday, rest) = tzset_num(rest, 1, 365)?;
        r.kind = RuleKind::Julian;
        r.day = jday;
        s = rest;
    } else if let Some(rest) = s.strip_prefix(b"M") {
        let (mon, rest) = tzset_num(rest, 1, 12)?;
        let rest = rest.strip_prefix(b".")?;
        let (week, rest) = tzset_num(rest, 1, 5)?;
        let rest = rest.strip_prefix(b".")?;
        let (day, rest) = tzset_num(rest, 0, 6)?;
        r.kind = RuleKind::MonthWeekDay;
        r.day = day;
        r.week = week;
        r.mon = mon;
        s = rest;
    } else {
        let (day, rest) = tzset_num(s, 0, 365)?;
        r.day = day;
        s = rest;
    }
    let Some(rest) = s.strip_prefix(b"/") else {
        r.time = 2 * SECONDS_PER_HOUR; // 2am is the default
        return Some((r, s));
    };
    let (offset, s) = tzset_offset(rest)?;
    r.time = offset;
    Some((r, s))
}

/// Go `tzsetNum`: a decimal number in `min..=max`.
fn tzset_num(s: &[u8], min: i64, max: i64) -> Option<(i64, &[u8])> {
    if s.is_empty() {
        return None;
    }
    let mut num = 0;
    for (i, &b) in s.iter().enumerate() {
        if !b.is_ascii_digit() {
            if i == 0 || num < min {
                return None;
            }
            return Some((num, &s[i..]));
        }
        num = num * 10 + i64::from(b - b'0');
        if num > max {
            return None;
        }
    }
    if num < min {
        return None;
    }
    Some((num, b""))
}

/// Go `tzruleTime`: seconds since the start of `year` at which rule `r`
/// takes effect, for a zone at offset `off`.
fn tzrule_time(year: i64, r: Rule, off: i64) -> i64 {
    let s = match r.kind {
        RuleKind::Julian => {
            let mut s = (r.day - 1) * SECONDS_PER_DAY;
            if is_leap(year) && r.day >= 60 {
                s += SECONDS_PER_DAY;
            }
            s
        }
        RuleKind::Doy => r.day * SECONDS_PER_DAY,
        RuleKind::MonthWeekDay => {
            // Zeller's congruence.
            let m1 = (r.mon + 9) % 12 + 1;
            let yy0 = if r.mon <= 2 {
                year.wrapping_sub(1)
            } else {
                year
            };
            let yy1 = yy0 / 100;
            let yy2 = yy0 % 100;
            let mut dow = ((26 * m1 - 2) / 10 + 1 + yy2 + yy2 / 4)
                .wrapping_add(yy1 / 4)
                .wrapping_sub(yy1.wrapping_mul(2))
                % 7;
            if dow < 0 {
                dow += 7;
            }
            // dow is the weekday of the first of r.mon; find the first r.day.
            let mut d = r.day - dow;
            if d < 0 {
                d += 7;
            }
            for _ in 1..r.week {
                if d + 7 >= days_in(r.mon, year) {
                    break;
                }
                d += 7;
            }
            d += days_before(r.mon);
            if is_leap(year) && r.mon > 2 {
                d += 1;
            }
            d * SECONDS_PER_DAY
        }
    };
    s + r.time - off
}

// --- TZif files (Go `LoadLocationFromTZData`) ---

/// Go `dataIO`: a cursor that remembers running out of data.
struct DataIo<'a> {
    p: &'a [u8],
    error: bool,
}

impl<'a> DataIo<'a> {
    fn read(&mut self, n: usize) -> &'a [u8] {
        if self.p.len() < n {
            self.p = &[];
            self.error = true;
            return &[];
        }
        let (head, tail) = self.p.split_at(n);
        self.p = tail;
        head
    }

    fn big4(&mut self) -> Option<u32> {
        let p = self.read(4);
        if p.len() < 4 {
            self.error = true;
            return None;
        }
        Some(u32::from_be_bytes([p[0], p[1], p[2], p[3]]))
    }

    fn big8(&mut self) -> Option<u64> {
        let hi = self.big4();
        let lo = self.big4();
        match (hi, lo) {
            (Some(hi), Some(lo)) => Some((u64::from(hi) << 32) | u64::from(lo)),
            _ => {
                self.error = true;
                None
            }
        }
    }

    fn byte(&mut self) -> Option<u8> {
        let p = self.read(1);
        if p.is_empty() {
            self.error = true;
            return None;
        }
        Some(p[0])
    }
}

/// Go `byteString`: the bytes up to the first NUL.
fn byte_string(p: &[u8]) -> String {
    let end = p.iter().position(|&b| b == 0).unwrap_or(p.len());
    String::from_utf8_lossy(&p[..end]).into_owned()
}

/// Go `LoadLocationFromTZData`: a zone named `name` from TZif v1, v2 or v3
/// data (the 64-bit body of v2+ files, then the POSIX rule footer).
/// `None` where Go reports malformed data.
pub(crate) fn from_tzdata(name: &str, data: &[u8]) -> Option<Zone> {
    let mut d = DataIo {
        p: data,
        error: false,
    };
    if d.read(4) != b"TZif" {
        return None;
    }
    let p = d.read(16);
    if p.len() != 16 {
        return None;
    }
    let version = match p[0] {
        0 => 1,
        b'2' => 2,
        b'3' => 3,
        _ => return None,
    };

    // Counts: UTC/local indicators, standard/wall indicators, leap seconds,
    // transition times, zone types, abbreviation characters.
    const NUTC_LOCAL: usize = 0;
    const NSTD_WALL: usize = 1;
    const NLEAP: usize = 2;
    const NTIME: usize = 3;
    const NZONE: usize = 4;
    const NCHAR: usize = 5;
    let mut n = [0usize; 6];
    for c in &mut n {
        *c = d.big4()? as usize;
    }

    // Version 2+ repeats the data with 64-bit times: skip the 32-bit copy.
    let is64 = version > 1;
    if is64 {
        let skip = n[NTIME] * 4
            + n[NTIME]
            + n[NZONE] * 6
            + n[NCHAR]
            + n[NLEAP] * 8
            + n[NSTD_WALL]
            + n[NUTC_LOCAL]
            + 4
            + 16;
        d.read(skip);
        for c in &mut n {
            *c = d.big4()? as usize;
        }
    }
    let size = if is64 { 8 } else { 4 };

    let mut txtimes = DataIo {
        p: d.read(n[NTIME] * size),
        error: false,
    };
    let txzones = d.read(n[NTIME]);
    let mut zonedata = DataIo {
        p: d.read(n[NZONE] * 6),
        error: false,
    };
    let abbrev = d.read(n[NCHAR]);
    d.read(n[NLEAP] * (size + 4)); // leap seconds: ignored
    d.read(n[NSTD_WALL]); // standard/wall indicators: ignored
    d.read(n[NUTC_LOCAL]); // UTC/local indicators: ignored
    if d.error {
        return None;
    }

    let rest = std::mem::take(&mut d.p);
    let extend = if rest.len() > 2 && rest[0] == b'\n' && rest[rest.len() - 1] == b'\n' {
        parse_footer(&rest[1..rest.len() - 1])
    } else {
        None
    };

    // Reject files with no zone types: nothing useful in them.
    if n[NZONE] == 0 {
        return None;
    }
    let mut zones = Vec::with_capacity(n[NZONE]);
    for _ in 0..n[NZONE] {
        let offset = zonedata.big4()? as i32;
        let is_dst = zonedata.byte()? != 0;
        let b = zonedata.byte()? as usize;
        if b >= abbrev.len() {
            return None;
        }
        zones.push(ZoneType {
            name: byte_string(&abbrev[b..]),
            offset,
            is_dst,
        });
    }

    let mut tx = Vec::with_capacity(n[NTIME].max(1));
    for &index in txzones {
        let when = if is64 {
            txtimes.big8()? as i64
        } else {
            i64::from(txtimes.big4()? as i32)
        };
        if usize::from(index) >= zones.len() {
            return None;
        }
        tx.push(Trans { when, index });
    }
    if tx.is_empty() {
        // A fake transition covering all time (fixed zones like "Etc/GMT0").
        tx.push(Trans {
            when: ALPHA,
            index: 0,
        });
    }

    Some(Zone {
        name: name.to_string(),
        zones,
        tx,
        extend,
    })
}

// --- Finding zone files (Go `zoneinfo_unix.go`, `loadLocation`) ---

/// Go `platformZoneSources`: directories searched for a zone name.
const PLATFORM_ZONE_SOURCES: &[&str] = &[
    "/usr/share/zoneinfo/",
    "/usr/share/lib/zoneinfo/",
    "/usr/lib/locale/TZ/",
    "/etc/zoneinfo",
];

/// Go `maxFileSize`: zone files larger than this are refused.
const MAX_FILE_SIZE: usize = 10 << 20;

/// Go `loadTzinfoFromDirOrZip` for a directory source, and `readFile`.
fn read_zone_file(dir: &[u8], name: &[u8]) -> Option<Vec<u8>> {
    let mut path = Vec::with_capacity(dir.len() + 1 + name.len());
    if !dir.is_empty() {
        path.extend_from_slice(dir);
        path.push(b'/');
    }
    path.extend_from_slice(name);
    // Read at most one byte past the limit, so an endless file
    // (`TZ=/dev/zero`) is refused as Go refuses it, not read forever.
    let file = std::fs::File::open(OsString::from_vec(path)).ok()?;
    let mut data = Vec::new();
    file.take(MAX_FILE_SIZE as u64 + 1)
        .read_to_end(&mut data)
        .ok()?;
    if data.len() > MAX_FILE_SIZE {
        return None;
    }
    Some(data)
}

/// Go `loadLocation`: the first source holding a valid zone file `name`.
/// (Go then tries an embedded `time/tzdata` and `$GOROOT/lib/time/zoneinfo.zip`;
/// fung's release binaries have neither, so neither do we.)
fn load_from(name: &[u8], sources: &[&str]) -> Option<Zone> {
    let display = String::from_utf8_lossy(name);
    sources.iter().find_map(|source| {
        read_zone_file(source.as_bytes(), name).and_then(|data| from_tzdata(&display, &data))
    })
}

/// The zone `name` (`"Asia/Seoul"`) from the system zone directories, as
/// Go's `initLocal` loads a relative `TZ`. fung itself only uses the local
/// zone, so only tests load one by name.
#[cfg(test)]
pub(crate) fn load(name: &str) -> Option<Zone> {
    load_from(name.as_bytes(), PLATFORM_ZONE_SOURCES)
}

/// Go `initLocal`: the local zone for the value of `TZ` (`None` when unset).
///
/// - unset: `/etc/localtime`, named `"Local"`;
/// - `""` or `"UTC"`: UTC;
/// - a leading `:` is dropped; an absolute path is read directly (named by
///   the path, or `"Local"` for `/etc/localtime`); any other name is looked
///   up in the zone directories (named by itself);
/// - anything that fails to load: UTC.
pub(crate) fn resolve_local(tz: Option<&OsStr>) -> Zone {
    match tz.map(OsStr::as_bytes) {
        None => {
            if let Some(mut z) = load_from(b"localtime", &["/etc"]) {
                z.name = "Local".to_string();
                return z;
            }
        }
        Some(tz) if !tz.is_empty() => {
            let tz = tz.strip_prefix(b":").unwrap_or(tz);
            if tz.first() == Some(&b'/') {
                if let Some(mut z) = load_from(tz, &[""]) {
                    z.name = if tz == b"/etc/localtime" {
                        "Local".to_string()
                    } else {
                        String::from_utf8_lossy(tz).into_owned()
                    };
                    return z;
                }
            } else if !tz.is_empty()
                && tz != b"UTC"
                && let Some(z) = load_from(tz, PLATFORM_ZONE_SOURCES)
            {
                return z;
            }
        }
        Some(_) => {}
    }
    Zone::utc()
}

/// The process's local zone, resolved from `TZ` on first use.
pub(crate) fn local() -> &'static Zone {
    static LOCAL: OnceLock<Zone> = OnceLock::new();
    LOCAL.get_or_init(|| resolve_local(std::env::var_os("TZ").as_deref()))
}

#[cfg(test)]
#[path = "tz_tests.rs"]
mod tests;
