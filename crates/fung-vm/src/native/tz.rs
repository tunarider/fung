//! Time zones read from TZif files (RFC 8536) and POSIX zone strings.
//!
//! fung only ever uses the local zone, resolved once per process from `TZ`
//! and `TZDIR` the way the C library does (`resolve_local`). A [`Zone`]
//! holds plain Rust data (no `Rc`), so any thread may read the shared one.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::sync::OnceLock;

use crate::value::{civil_from_days, days_from_civil};

/// One zone type of a TZif file: `"CET"`, seconds east of UTC.
#[derive(Clone, Debug)]
struct ZoneType {
    name: String,
    offset: i32,
    is_dst: bool,
}

/// A transition: from `when` on, zone type `index` applies.
#[derive(Clone, Copy, Debug)]
struct Trans {
    when: i64,
    index: u8,
}

/// A time zone.
#[derive(Clone, Debug)]
pub(crate) struct Zone {
    /// `"Local"`, `"UTC"`, `"Asia/Seoul"`, a path or a POSIX string.
    pub(crate) name: String,
    zones: Vec<ZoneType>,
    tx: Vec<Trans>,
    /// The POSIX TZ rule after the TZif data, for instants past the last
    /// transition (`EST5EDT,M3.2.0,M11.1.0`), parsed; `None` when the file
    /// has none or it is not a valid POSIX rule.
    extend: Option<Footer>,
}

const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_HOUR: i64 = 60 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: i64 = 24 * SECONDS_PER_HOUR;

impl Zone {
    /// UTC (no zone types: every lookup is `UTC`, offset 0).
    pub(crate) fn utc() -> Zone {
        Zone {
            name: "UTC".to_string(),
            zones: Vec::new(),
            tx: Vec::new(),
            extend: None,
        }
    }

    /// A zone made of a POSIX zone string alone, named by it.
    fn posix(tz: &[u8]) -> Option<Zone> {
        let f = parse_footer(tz)?;
        Some(Zone {
            name: String::from_utf8_lossy(tz).into_owned(),
            zones: vec![ZoneType {
                name: f.std_name.clone(),
                offset: f.std_offset as i32,
                is_dst: false,
            }],
            tx: vec![Trans {
                when: i64::MIN,
                index: 0,
            }],
            extend: Some(f),
        })
    }

    /// The zone abbreviation and offset (seconds east of UTC) in effect at
    /// `unix_sec`.
    pub(crate) fn lookup(&self, unix_sec: i64) -> (&str, i32) {
        let l = self.lookup_full(unix_sec);
        (l.name, l.offset)
    }

    /// The zone at `sec`: the last transition at or before it, the POSIX
    /// rule past the last transition, the first zone (tzcode's rules)
    /// before the first.
    fn lookup_full(&self, sec: i64) -> Lookup<'_> {
        // Times run from year 0000 to 9999; a week either side covers any
        // wall-clock arithmetic near the ends.
        const WEEK: i64 = 7 * SECONDS_PER_DAY;
        let sec = sec.clamp(crate::value::MIN_UNIX - WEEK, crate::value::MAX_UNIX + WEEK);
        if self.zones.is_empty() {
            return Lookup {
                name: "UTC",
                offset: 0,
            };
        }
        if self.tx.is_empty() || sec < self.tx[0].when {
            let z = &self.zones[self.lookup_first_zone()];
            return Lookup {
                name: &z.name,
                offset: z.offset,
            };
        }
        // Binary search for the entry with the largest time <= sec.
        let tx = &self.tx;
        let lo = tx.partition_point(|t| t.when <= sec) - 1;
        let z = &self.zones[tx[lo].index as usize];
        // At the end of the known transitions, try the POSIX rule.
        if lo == tx.len() - 1
            && let Some(f) = &self.extend
        {
            return tzset(f, sec);
        }
        Lookup {
            name: &z.name,
            offset: z.offset,
        }
    }

    /// The zone type for instants before the first transition (the tzcode
    /// `localtime.c` rules).
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

    /// The instant (unix seconds) whose wall clock in this zone reads
    /// `wall` (seconds since 1970 on the wall clock). A wall time the zone
    /// repeats (clocks set back) is its earlier instant; one the zone skips
    /// (clocks set forward) moves forward by the gap, read with the offset
    /// in effect before it (06: 시간 형식).
    pub(crate) fn date(&self, wall: i64) -> i64 {
        // The offsets a day before and after: those around any change near
        // `wall` (changes are further apart than that).
        let before = self.lookup(wall.saturating_sub(86_400)).1;
        let after = self.lookup(wall.saturating_add(86_400)).1;
        let fits = |o: i32| self.lookup(wall - i64::from(o)).1 == o;
        let offset = match (fits(before), fits(after)) {
            (true, true) => before.max(after),
            (true, false) => before,
            (false, true) => after,
            (false, false) => before,
        };
        wall - i64::from(offset)
    }
}

/// The zone in effect at an instant: abbreviation and offset.
struct Lookup<'a> {
    name: &'a str,
    offset: i32,
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// Days in a non-leap year before month `m` (1-based).
fn days_before(m: i64) -> i64 {
    let adj = if m >= 3 { -2 } else { 0 };
    (214 * m - 211) / 7 + adj
}

fn days_in(m: i64, year: i64) -> i64 {
    if m == 2 {
        return if is_leap(year) { 29 } else { 28 };
    }
    30 + ((m + (m >> 3)) & 1)
}

// --- POSIX TZ rules ---

/// A rule from a POSIX TZ string.
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

/// A parsed POSIX TZ string (a TZif footer or `TZ` itself), parsed once.
/// Names are decoded lossily only after parsing, which works on bytes.
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

/// A POSIX TZ string, or `None` when it is not one (a footer is then
/// ignored).
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

/// The zone at `sec` under the POSIX rule `f`.
fn tzset(f: &Footer, sec: i64) -> Lookup<'_> {
    let Some(dst) = &f.dst else {
        return Lookup {
            name: &f.std_name,
            offset: f.std_offset as i32,
        };
    };
    let (mut std_name, mut std_offset) = (f.std_name.as_str(), f.std_offset);
    let (mut dst_name, mut dst_offset) = (dst.name.as_str(), dst.offset);

    // The UTC year containing sec, and seconds since its start.
    let days = sec.div_euclid(SECONDS_PER_DAY);
    let (year, _, _) = civil_from_days(days);
    let ysec = sec - days_from_civil(year, 1, 1) * SECONDS_PER_DAY;

    let mut start_sec = tzrule_time(year, dst.start, std_offset);
    let mut end_sec = tzrule_time(year, dst.end, dst_offset);
    // Southern hemisphere: the rule's "DST" spans the year boundary, so
    // swap the halves.
    if end_sec < start_sec {
        std::mem::swap(&mut start_sec, &mut end_sec);
        std::mem::swap(&mut std_name, &mut dst_name);
        std::mem::swap(&mut std_offset, &mut dst_offset);
    }

    if ysec < start_sec || ysec >= end_sec {
        Lookup {
            name: std_name,
            offset: std_offset as i32,
        }
    } else {
        Lookup {
            name: dst_name,
            offset: dst_offset as i32,
        }
    }
}

/// A zone name (`EST`, or `<+0330>` without the brackets); every byte it
/// tests for is ASCII, so it scans bytes.
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

/// `[+-]hh[:mm[:ss]]` in seconds.
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

/// `Jn`, `n` or `Mm.w.d`, then an optional `/time`.
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

/// A decimal number in `min..=max`.
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

/// Seconds since the start of `year` at which rule `r`
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

// --- TZif files ---

/// A cursor that remembers running out of data.
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

/// The bytes up to the first NUL.
fn byte_string(p: &[u8]) -> String {
    let end = p.iter().position(|&b| b == 0).unwrap_or(p.len());
    String::from_utf8_lossy(&p[..end]).into_owned()
}

/// A zone named `name` from TZif v1, v2 or v3
/// data (the 64-bit body of v2+ files, then the POSIX rule footer).
/// `None` for malformed data.
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
            when: i64::MIN,
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

// --- Finding zone files ---

/// Where relative zone names are read from when `TZDIR` is not set.
const ZONE_DIR: &str = "/usr/share/zoneinfo";

/// Zone files larger than this are refused.
const MAX_FILE_SIZE: usize = 10 << 20;

/// The zone file `name` in `dir`.
fn read_zone_file(dir: &[u8], name: &[u8]) -> Option<Vec<u8>> {
    let mut path = Vec::with_capacity(dir.len() + 1 + name.len());
    if !dir.is_empty() {
        path.extend_from_slice(dir);
        path.push(b'/');
    }
    path.extend_from_slice(name);
    // Read at most one byte past the limit, so an endless file
    // (`TZ=/dev/zero`) is refused, not read forever.
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

/// The first source holding a valid zone file `name`.
fn load_from(name: &[u8], sources: &[&str]) -> Option<Zone> {
    let display = String::from_utf8_lossy(name);
    sources.iter().find_map(|source| {
        read_zone_file(source.as_bytes(), name).and_then(|data| from_tzdata(&display, &data))
    })
}

/// The zone `name` (`"Asia/Seoul"`) from the system zone directory, as a
/// relative `TZ` loads. fung itself only uses the local zone, so only tests
/// load one by name.
#[cfg(test)]
pub(crate) fn load(name: &str) -> Option<Zone> {
    load_from(name.as_bytes(), &[ZONE_DIR])
}

/// The local zone for the value of `TZ` (`None` when unset), with
/// relative names read from `tzdir` (`TZDIR`) or `/usr/share/zoneinfo`:
///
/// - unset: `/etc/localtime`, named `"Local"`;
/// - `""` or `"UTC"`: UTC;
/// - a leading `:` is dropped; an absolute path is read directly (named by
///   the path, or `"Local"` for `/etc/localtime`); any other name is looked
///   up in the zone directory (named by itself);
/// - a value that names no zone file may be a POSIX zone string
///   (`KST-9`, `EST5EDT,M3.2.0,M11.1.0`), named by itself;
/// - anything else: UTC.
pub(crate) fn resolve_local(tz: Option<&OsStr>, tzdir: Option<&OsStr>) -> Zone {
    let Some(tz) = tz.map(OsStr::as_bytes) else {
        return match load_from(b"localtime", &["/etc"]) {
            Some(mut z) => {
                z.name = "Local".to_string();
                z
            }
            None => Zone::utc(),
        };
    };
    let tz = tz.strip_prefix(b":").unwrap_or(tz);
    if tz.is_empty() || tz == b"UTC" {
        return Zone::utc();
    }
    let file = if tz.first() == Some(&b'/') {
        load_from(tz, &[""]).map(|mut z| {
            z.name = if tz == b"/etc/localtime" {
                "Local".to_string()
            } else {
                String::from_utf8_lossy(tz).into_owned()
            };
            z
        })
    } else {
        let dir = tzdir.filter(|d| !d.is_empty()).map_or_else(
            || ZONE_DIR.to_string(),
            |d| d.to_string_lossy().into_owned(),
        );
        load_from(tz, &[dir.as_str()])
    };
    file.or_else(|| Zone::posix(tz)).unwrap_or_else(Zone::utc)
}

/// The process's local zone, resolved from `TZ` and `TZDIR` on first use.
pub(crate) fn local() -> &'static Zone {
    static LOCAL: OnceLock<Zone> = OnceLock::new();
    LOCAL.get_or_init(|| {
        resolve_local(
            std::env::var_os("TZ").as_deref(),
            std::env::var_os("TZDIR").as_deref(),
        )
    })
}

#[cfg(test)]
#[path = "tz_tests.rs"]
mod tests;
