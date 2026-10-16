//! The time format tokens (06: 시간 형식; Go `internal/native/timefmt.go`)
//! and Go's `time.Parse(time.RFC3339Nano, …)`, which `time.parse` uses
//! without a format, with Go's own error messages.

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

/// Go `pad`: `strconv.Itoa(n)` with zeros in front up to `width` bytes
/// (so year -1 is `00-1`).
fn pad(n: i64, width: usize) -> String {
    format!("{n:0>width$}")
}

/// `+HH:MM` (`colon`) or `+HHMM`; the offset's seconds are dropped.
fn zone(t: &Time, colon: bool) -> String {
    let (_, off) = t.zone();
    let (sign, off) = if off < 0 { ("-", -off) } else { ("+", off) };
    let (h, m) = (i64::from(off / 3600), i64::from(off % 3600 / 60));
    if colon {
        format!("{sign}{}:{}", pad(h, 2), pad(m, 2))
    } else {
        format!("{sign}{}{}", pad(h, 2), pad(m, 2))
    }
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

/// Go `strconv.Quote` of bytes that may end inside a character: each
/// byte of an invalid sequence is `\xNN`.
pub(crate) fn quote_bytes(s: &[u8]) -> String {
    let mut out = String::from("\"");
    for chunk in s.utf8_chunks() {
        let q = quote(chunk.valid());
        out.push_str(&q[1..q.len() - 1]);
        for b in chunk.invalid() {
            out.push_str(&format!("\\x{b:02x}"));
        }
    }
    out.push('"');
    out
}

/// Reads a three-letter English name from `s`, matching any (ASCII) case:
/// its index in `names` and the rest. A name is 3 ASCII bytes, so a
/// non-ASCII character never folds to one.
fn name<'a>(s: &'a [u8], names: &[&str], what: &str) -> Result<(usize, &'a [u8]), String> {
    if s.len() < 3 {
        return Err(format!("expected a {what} name"));
    }
    match names
        .iter()
        .position(|n| s[..3].eq_ignore_ascii_case(n.as_bytes()))
    {
        Some(i) => Ok((i, &s[3..])),
        None => Err(format!("unknown {what} name {}", quote_bytes(&s[..3]))),
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

/// `+HH:MM` (`colon`) or `+HHMM`: the offset in seconds and the rest.
fn parse_zone(s: &[u8], colon: bool) -> Result<(i32, &[u8]), String> {
    let sign = match s.first() {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return Err("expected a time zone offset".into()),
    };
    let (h, mut rest) = digits(&s[1..], 2, "the zone hours")?;
    if colon {
        let Some(r) = rest.strip_prefix(b":") else {
            return Err("expected `:` in the time zone offset".into());
        };
        rest = r;
    }
    let (m, rest) = digits(rest, 2, "the zone minutes")?;
    Ok((sign * (h as i32 * 3600 + m as i32 * 60), rest))
}

/// Reads `s` with the token layout. Fields the layout does not mention are
/// zero (month and day one); without a zone token the time is in `local`.
/// Out-of-range fields (month 13, Feb 30) are errors: Go `time.Date` would
/// normalise them silently.
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
            "MMM" => name(s, &MONTH_NAMES, "month").map(|(i, r)| {
                month = i as i64 + 1;
                r
            })?,
            // The weekday is read and checked as a name only: the date
            // decides the real weekday.
            "ddd" => name(s, &WEEKDAY_NAMES, "weekday")?.1,
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
        return Err(format!("unexpected text {} after the date", quote_bytes(s)));
    }
    if !(1..=12).contains(&month) || hour > 23 || min > 59 || sec > 59 {
        return Err("field out of range".into());
    }
    let t = match fixed {
        // Go `time.Date` in `FixedZone("", off)`: the wall time read as
        // UTC, moved by the offset.
        Some(off) => {
            let (u, ns) = Zone::utc().date(year, month, day, hour, min, sec, ms * 1_000_000);
            let mut t = Time::from_unix(u, ns.into(), Loc::Fixed(off));
            t.add_sec(-i64::from(off));
            t
        }
        None => {
            let (u, ns) = local.date(year, month, day, hour, min, sec, ms * 1_000_000);
            Time::from_unix(u, ns.into(), Loc::Zone(local))
        }
    };
    let (_, m, d) = t.date();
    if d != day || m != month {
        return Err(format!("day {day} is out of range for the month"));
    }
    Ok(t)
}

// ---- Go `time.Parse(time.RFC3339Nano, s)` ----
//
// Go's `parse` walked over the RFC3339Nano layout's chunks; the layout is
// fixed, so its chunk list is spelled out. Go first tries a strict fast
// path (`parseRFC3339`); whenever that succeeds the general walk below
// gives the same time, and where it fails Go falls back to the walk, so
// only the walk is ported.

const RFC3339_NANO: &str = "2006-01-02T15:04:05.999999999Z07:00";

/// What a layout chunk reads (Go's `std*` codes the layout uses).
#[derive(Clone, Copy)]
enum Std {
    LongYear,
    ZeroMonth,
    ZeroDay,
    Hour,
    ZeroMinute,
    ZeroSecond,
    FracSecond9,
    Iso8601ColonTz,
}

/// The layout's chunks: (literal prefix, the chunk's layout text, what it
/// reads).
const CHUNKS: [(&str, &str, Std); 8] = [
    ("", "2006", Std::LongYear),
    ("-", "01", Std::ZeroMonth),
    ("-", "02", Std::ZeroDay),
    ("T", "15", Std::Hour),
    (":", "04", Std::ZeroMinute),
    (":", "05", Std::ZeroSecond),
    ("", ".999999999", Std::FracSecond9),
    ("", "Z07:00", Std::Iso8601ColonTz),
];

/// Go `time.quote` (not `strconv.Quote`): bytes outside printable ASCII
/// are `\xNN`; `"` and `\` are escaped; DEL stays as it is.
fn time_quote(s: &[u8]) -> String {
    let mut out = String::from("\"");
    for chunk in s.utf8_chunks() {
        for c in chunk.valid().chars() {
            if (c as u32) >= 0x80 || c < ' ' {
                let mut buf = [0; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("\\x{b:02x}"));
                }
            } else {
                if c == '"' || c == '\\' {
                    out.push('\\');
                }
                out.push(c);
            }
        }
        for b in chunk.invalid() {
            out.push_str(&format!("\\x{b:02x}"));
        }
    }
    out.push('"');
    out
}

/// Go `ParseError.Error()`.
fn parse_error(value: &[u8], layout_elem: &str, value_elem: &[u8], message: &str) -> String {
    if message.is_empty() {
        format!(
            "parsing time {} as {}: cannot parse {} as {}",
            time_quote(value),
            time_quote(RFC3339_NANO.as_bytes()),
            time_quote(value_elem),
            time_quote(layout_elem.as_bytes())
        )
    } else {
        format!("parsing time {}{message}", time_quote(value))
    }
}

fn is_digit(s: &[u8], i: usize) -> bool {
    s.get(i).is_some_and(u8::is_ascii_digit)
}

/// Go `getnum`: one or two digits (`fixed`: exactly two).
fn getnum(s: &[u8], fixed: bool) -> Option<(i64, &[u8])> {
    if !is_digit(s, 0) {
        return None;
    }
    if !is_digit(s, 1) {
        if fixed {
            return None;
        }
        return Some((i64::from(s[0] - b'0'), &s[1..]));
    }
    Some((
        i64::from(s[0] - b'0') * 10 + i64::from(s[1] - b'0'),
        &s[2..],
    ))
}

/// Go `skip` for the layout's literal prefixes (none has a space).
fn skip<'a>(value: &'a [u8], prefix: &str) -> Result<&'a [u8], &'a [u8]> {
    let mut value = value;
    for &p in prefix.as_bytes() {
        match value.split_first() {
            Some((&c, rest)) if c == p => value = rest,
            _ => return Err(value),
        }
    }
    Ok(value)
}

/// Go `parseNanoseconds` for a `.` or `,` and the `nbytes - 1` digits
/// after it: at most nine digits count.
fn parse_nanoseconds(value: &[u8], nbytes: usize) -> i64 {
    let nbytes = nbytes.min(10);
    let mut ns = value[1..nbytes]
        .iter()
        .fold(0i64, |v, d| v * 10 + i64::from(d - b'0'));
    for _ in nbytes..10 {
        ns *= 10;
    }
    ns
}

/// Go `daysIn`.
fn days_in(month: i64, year: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Go `time.Parse(time.RFC3339Nano, s)` with `local` as `time.Local`, with
/// Go's error text.
pub(crate) fn parse_rfc3339(s: &str, local: &'static Zone) -> Result<Time, String> {
    let all = s.as_bytes();
    let mut value = all;
    let (mut year, mut month, mut day) = (0, 0, 0);
    let (mut hour, mut min, mut sec, mut nsec) = (0, 0, 0, 0);
    // `Some(None)` is `Z` (UTC); `Some(Some(off))` a numeric offset.
    let mut zone: Option<Option<i32>> = None;
    for (prefix, elem, std) in CHUNKS {
        value = skip(value, prefix).map_err(|v| parse_error(all, prefix, v, ""))?;
        let hold = value;
        let mut range = "";
        let bad = || parse_error(all, elem, hold, "");
        match std {
            Std::LongYear => {
                // Go checks the first digit, then `atoi` refuses the rest
                // unless all four are digits.
                if value.len() < 4 || !value[..4].iter().all(u8::is_ascii_digit) {
                    return Err(bad());
                }
                year = value[..4]
                    .iter()
                    .fold(0i64, |v, d| v * 10 + i64::from(d - b'0'));
                value = &value[4..];
            }
            Std::ZeroMonth => {
                (month, value) = getnum(value, true).ok_or_else(bad)?;
                if !(1..=12).contains(&month) {
                    range = "month";
                }
            }
            Std::ZeroDay => (day, value) = getnum(value, true).ok_or_else(bad)?,
            Std::Hour => {
                (hour, value) = getnum(value, false).ok_or_else(bad)?;
                if hour >= 24 {
                    range = "hour";
                }
            }
            Std::ZeroMinute => {
                (min, value) = getnum(value, true).ok_or_else(bad)?;
                if min >= 60 {
                    range = "minute";
                }
            }
            Std::ZeroSecond => {
                (sec, value) = getnum(value, true).ok_or_else(bad)?;
                // A fraction after the seconds is read by the next chunk.
                if sec >= 60 {
                    range = "second";
                }
            }
            Std::FracSecond9 => {
                if value.len() >= 2 && matches!(value[0], b'.' | b',') && is_digit(value, 1) {
                    let mut i = 0;
                    while is_digit(value, i + 1) {
                        i += 1;
                    }
                    nsec = parse_nanoseconds(value, 1 + i);
                    value = &value[1 + i..];
                }
            }
            Std::Iso8601ColonTz => {
                if value.first() == Some(&b'Z') {
                    value = &value[1..];
                    zone = Some(None);
                } else {
                    if value.len() < 6 || value[3] != b':' {
                        return Err(bad());
                    }
                    let (sign, h, m) = (value[0], &value[1..3], &value[4..6]);
                    value = &value[6..];
                    let hr = getnum(h, true);
                    let mm = hr.and_then(|_| getnum(m, true));
                    let (hr, mm) = (hr.map_or(0, |x| x.0), mm.map_or(0, |x| x.0));
                    // `>` rather than `>=`: Go takes offsets of 24 hours
                    // or 60 minutes.
                    if hr > 24 {
                        range = "time zone offset hour";
                    }
                    if mm > 60 {
                        range = "time zone offset minute";
                    }
                    let off = ((hr * 60 + mm) * 60) as i32;
                    if range.is_empty() {
                        if getnum(h, true).is_none() || getnum(m, true).is_none() {
                            return Err(bad());
                        }
                        zone = Some(Some(match sign {
                            b'+' => off,
                            b'-' => -off,
                            _ => return Err(bad()),
                        }));
                    }
                }
            }
        }
        if !range.is_empty() {
            return Err(parse_error(
                all,
                elem,
                value,
                &format!(": {range} out of range"),
            ));
        }
    }
    if !value.is_empty() {
        return Err(parse_error(
            all,
            "",
            value,
            &format!(": extra text: {}", time_quote(value)),
        ));
    }
    if day < 1 || day > days_in(month, year) {
        return Err(parse_error(all, "", value, ": day out of range"));
    }
    let (u, ns) = Zone::utc().date(year, month, day, hour, min, sec, nsec);
    match zone {
        Some(Some(off)) => {
            let mut t = Time::from_unix(u, ns.into(), Loc::Utc);
            t.add_sec(-i64::from(off));
            // The local zone when it has this offset at that instant.
            let loc = if local.lookup(t.unix()).1 == off {
                Loc::Zone(local)
            } else {
                Loc::Fixed(off)
            };
            Ok(t.in_loc(loc))
        }
        // `Z`; the layout always ends in a zone, so there is no other case
        _ => Ok(Time::from_unix(u, ns.into(), Loc::Utc)),
    }
}

#[cfg(test)]
#[path = "timefmt_tests.rs"]
mod tests;
