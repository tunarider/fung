//! The time format tokens (06: 시간 형식) and the RFC 3339 form `time.parse`
//! reads without a format.

use fung_front::diag::quote;

use super::tz::Zone;
use crate::value::{Loc, Time};

/// The tokens, longest first so `YYYY` wins over `YY` and `ZZ` over `Z`.
/// Characters outside the table are literal: they print as they are and
/// must match on parse.
const TOKENS: [&str; 12] = [
    "YYYY", "MMM", "SSS", "ddd", "YY", "MM", "DD", "HH", "mm", "ss", "ZZ", "Z",
];

const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// The token at the start of `layout`, or `None` for a literal character.
fn next_token(layout: &str) -> Option<&'static str> {
    TOKENS.into_iter().find(|tk| layout.starts_with(tk))
}

/// `n` with zeros in front up to `width` digits (a year at the range's
/// edge shown in an offset may be negative: `-001`).
fn pad(n: i64, width: usize) -> String {
    format!("{n:0width$}")
}

/// `+HH:MM` (`colon`) or `+HHMM`, with the seconds when the offset has
/// them.
fn zone(t: &Time, colon: bool) -> String {
    crate::value::offset_text(t.zone().1, colon)
}

/// Renders `t` with the token layout.
pub(crate) fn format_time(t: &Time, mut layout: &str) -> String {
    let (year, month, day) = t.date();
    let (hour, min, sec) = t.clock();
    let mut b = String::new();
    while let Some(c) = layout.chars().next() {
        let Some(tk) = next_token(layout) else {
            b.push(c);
            layout = &layout[c.len_utf8()..];
            continue;
        };
        match tk {
            "YYYY" => b.push_str(&pad(year, 4)),
            "YY" => b.push_str(&pad(year % 100, 2)),
            "MM" => b.push_str(&pad(month, 2)),
            "MMM" => b.push_str(MONTH_NAMES[month as usize - 1]),
            "ddd" => b.push_str(WEEKDAY_NAMES[t.weekday() as usize]),
            "DD" => b.push_str(&pad(day, 2)),
            "HH" => b.push_str(&pad(hour, 2)),
            "mm" => b.push_str(&pad(min, 2)),
            "ss" => b.push_str(&pad(sec, 2)),
            "SSS" => b.push_str(&pad(i64::from(t.nanosecond() / 1_000_000), 3)),
            "Z" => b.push_str(&zone(t, true)),
            _ => b.push_str(&zone(t, false)), // "ZZ"
        }
        layout = &layout[tk.len()..];
    }
    b
}

/// Reads a three-letter English name from `s`, matching any (ASCII) case:
/// its index in `names` and the rest. A name is 3 ASCII bytes, so a
/// non-ASCII character never folds to one.
fn name<'a>(
    s: &'a [u8],
    names: &[&str],
    what: &str,
    like: &str,
) -> Result<(usize, &'a [u8]), String> {
    let found = s.get(..3).and_then(|head| {
        names
            .iter()
            .position(|n| head.eq_ignore_ascii_case(n.as_bytes()))
    });
    match found {
        Some(i) => Ok((i, &s[3..])),
        None => Err(format!("expected a {what} name like {like}")),
    }
}

/// Exactly `n` ASCII digits from the front of `s`.
fn digits<'a>(s: &'a [u8], n: usize, what: &str) -> Result<(i64, &'a [u8]), String> {
    if s.len() < n || !s[..n].iter().all(u8::is_ascii_digit) {
        return Err(format!("expected {n} digits for {what}"));
    }
    let v = s[..n]
        .iter()
        .fold(0i64, |v, d| v * 10 + i64::from(d - b'0'));
    Ok((v, &s[n..]))
}

/// `+HH:MM[:SS]` (`colon`) or `+HHMM`: the offset in seconds and the rest.
fn parse_zone(s: &[u8], colon: bool) -> Result<(i32, &[u8]), String> {
    let sign = match s.first() {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return Err("expected a time zone offset".into()),
    };
    let (h, mut rest) = digits(&s[1..], 2, "the offset hours")?;
    if colon {
        rest = expect(rest, b':', "in the offset")?;
    }
    let (m, mut rest) = digits(rest, 2, "the offset minutes")?;
    let mut sec = 0;
    if colon
        && rest.first() == Some(&b':')
        && rest
            .get(1..3)
            .is_some_and(|d| d.iter().all(u8::is_ascii_digit))
    {
        let (v, r) = digits(&rest[1..], 2, "the offset seconds")?;
        sec = v;
        rest = r;
    }
    if h > 23 || m > 59 || sec > 59 {
        return Err("offset out of range".into());
    }
    Ok((sign * (h as i32 * 3600 + m as i32 * 60 + sec as i32), rest))
}

/// Reads `s` with the token layout. Fields the layout does not mention are
/// zero (month and day one); without a zone token the time is in `local`.
/// Out-of-range fields (month 13, Feb 30) are errors.
pub(crate) fn parse_time(s: &str, layout: &str, local: &'static Zone) -> Result<Time, String> {
    let (mut year, mut month, mut day) = (0, 1, 1);
    let (mut hour, mut min, mut sec, mut ms) = (0, 0, 0, 0);
    let mut fixed: Option<i32> = None;
    let mut s = s.as_bytes();
    let mut layout = layout;
    while let Some(c) = layout.chars().next() {
        let Some(tk) = next_token(layout) else {
            let mut buf = [0; 4];
            let lit = c.encode_utf8(&mut buf);
            let Some(rest) = s.strip_prefix(lit.as_bytes()) else {
                return Err(format!("expected {}", quote(lit)));
            };
            s = rest;
            layout = &layout[c.len_utf8()..];
            continue;
        };
        s = match tk {
            "YYYY" => digits(s, 4, "the year").map(|(v, r)| {
                year = v;
                r
            })?,
            "YY" => digits(s, 2, "the year").map(|(v, r)| {
                year = v + 2000;
                r
            })?,
            "MM" => digits(s, 2, "the month").map(|(v, r)| {
                month = v;
                r
            })?,
            "MMM" => name(s, &MONTH_NAMES, "month", "Oct").map(|(i, r)| {
                month = i as i64 + 1;
                r
            })?,
            // The weekday is read and checked as a name only: the date
            // decides the real weekday.
            "ddd" => name(s, &WEEKDAY_NAMES, "weekday", "Sun")?.1,
            "DD" => digits(s, 2, "the day").map(|(v, r)| {
                day = v;
                r
            })?,
            "HH" => digits(s, 2, "the hour").map(|(v, r)| {
                hour = v;
                r
            })?,
            "mm" => digits(s, 2, "the minute").map(|(v, r)| {
                min = v;
                r
            })?,
            "ss" => digits(s, 2, "the second").map(|(v, r)| {
                sec = v;
                r
            })?,
            "SSS" => digits(s, 3, "the milliseconds").map(|(v, r)| {
                ms = v;
                r
            })?,
            _ => parse_zone(s, tk == "Z").map(|(off, r)| {
                fixed = Some(off);
                r
            })?,
        };
        layout = &layout[tk.len()..];
    }
    if !s.is_empty() {
        return Err("unexpected text after the time".into());
    }
    check_fields(year, month, day, hour, min, sec)?;
    let loc = match fixed {
        Some(off) => Loc::Fixed(off),
        None => Loc::Zone(local),
    };
    Time::from_wall(year, month, day, hour, min, sec, ms * 1_000_000, loc)
        .ok_or_else(|| "time out of range".to_string())
}

// ---- RFC 3339 ----

/// `c` at the front of `s`, or the error naming where it was expected.
fn expect<'a>(s: &'a [u8], c: u8, place: &str) -> Result<&'a [u8], String> {
    s.strip_prefix(&[c])
        .ok_or_else(|| format!("expected `{}` {place}", c as char))
}

/// A time in RFC 3339 form (06: 시간 형식), read strictly:
/// `YYYY-MM-DDThh:mm:ss[.f]` then `Z` or `±hh:mm[:ss]`; `T` and `Z` in
/// either case, one to nine fraction digits. A numeric offset is kept as
/// written; `Z` is UTC.
pub(crate) fn parse_rfc3339(s: &str) -> Result<Time, String> {
    let b = s.as_bytes();
    let (year, r) = digits(b, 4, "the year")?;
    let r = expect(r, b'-', "after the year")?;
    let (month, r) = digits(r, 2, "the month")?;
    let r = expect(r, b'-', "after the month")?;
    let (day, r) = digits(r, 2, "the day")?;
    let r = match r.first() {
        Some(b'T' | b't') => &r[1..],
        _ => return Err("expected `T` between the date and the time".into()),
    };
    let (hour, r) = digits(r, 2, "the hour")?;
    let r = expect(r, b':', "after the hour")?;
    let (min, r) = digits(r, 2, "the minute")?;
    let r = expect(r, b':', "after the minute")?;
    let (sec, mut r) = digits(r, 2, "the second")?;
    let mut nsec = 0;
    if let Some(rest) = r.strip_prefix(b".") {
        let n = rest.iter().take_while(|c| c.is_ascii_digit()).count();
        if !(1..=9).contains(&n) {
            return Err("expected 1 to 9 digits after the decimal point".into());
        }
        let frac = rest[..n]
            .iter()
            .fold(0i64, |v, d| v * 10 + i64::from(d - b'0'));
        nsec = frac * 10i64.pow(9 - n as u32);
        r = &rest[n..];
    }
    let (loc, r) = match r.first() {
        Some(b'Z' | b'z') => (Loc::Utc, &r[1..]),
        Some(b'+' | b'-') => {
            let (off, rest) = parse_zone(r, true)?;
            (Loc::Fixed(off), rest)
        }
        _ => return Err("expected Z or an offset like +09:00".into()),
    };
    if !r.is_empty() {
        return Err("unexpected text after the time".into());
    }
    check_fields(year, month, day, hour, min, sec)?;
    Time::from_wall(year, month, day, hour, min, sec, nsec, loc)
        .ok_or_else(|| "time out of range".to_string())
}

/// The fields of a wall time, each in its range.
fn check_fields(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    min: i64,
    sec: i64,
) -> Result<(), String> {
    if !(1..=12).contains(&month) {
        return Err("month out of range".into());
    }
    if day < 1 || day > days_in(month, year) {
        return Err("day out of range".into());
    }
    if hour > 23 {
        return Err("hour out of range".into());
    }
    if min > 59 {
        return Err("minute out of range".into());
    }
    if sec > 59 {
        return Err("second out of range".into());
    }
    Ok(())
}

fn days_in(month: i64, year: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
#[path = "timefmt_tests.rs"]
mod tests;
