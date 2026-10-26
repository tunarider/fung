//! `time` programs under pinned `TZ` values (set on each child, not on
//! the test process) against `expected/time/programs.txt`: every format
//! token, the zone tokens, normalisation blocking, and `parse`, `local`
//! and `parts` around the 2026 daylight-saving changes in New York and
//! Berlin. With `TZ` unset, the programs print what they print with
//! `TZ=/etc/localtime` (the machine's zone, so not a fixed expectation).

mod common;

use std::process::Command;

use common::expected::{self, Entry};
use common::{fung_bin, repo};

/// The `TZ` values with a fixed expected output.
const ZONES: &[&str] = &[
    "America/New_York",
    ":America/New_York",
    "/usr/share/zoneinfo/Europe/Berlin",
    "Europe/Berlin",
    "Asia/Seoul",
    "UTC",
    "",
    "No/Such_Zone",
];

/// The 2026 New York changes: 2026-03-08 07:00 UTC (02:00 EST becomes
/// 03:00 EDT) and 2026-11-01 06:00 UTC (02:00 EDT becomes 01:00 EST).
const PROGRAMS: &[&str] = &[
    // every token, around both changes and in the far past (LMT offsets
    // with seconds) and at the representable edges
    r#"fn show(t) = print(time.format(t, "YYYY YY MM MMM DD ddd HH mm ss SSS Z ZZ"), time.local(t), time.parts(t), time.unix(t))
for n in [1772953200 - 1, 1772953200, 1772955000, 1793512800 - 1, 1793512800, 1793514600, -3000000000, -2208988800, 0, 1791072307]:
  show(time.from_unix(n))
  show(time.utc(time.from_unix(n)))
;;"#,
    r#"for n in [-62135596800, -62135596801, -62167219200 - 1, -100000000000, 4611686018427387903, -4611686018427387903]:
  t = time.utc(time.from_unix(n))
  print(t, time.format(t, "YYYY YY MM DD HH mm ss ddd MMM"), time.parts(t), time.unix(t))
;;"#,
    r#"print(time.from_unix(4611686018427387904))"#,
    r#"print(time.from_unix(-4611686018427387904), time.from_unix(-4611686018427387903))"#,
    r#"print(time.from_unix("1"))"#,
    // wall times the local zone skips or repeats, and the days around them
    r#"for s in ["2026-03-08 01:59", "2026-03-08 02:00", "2026-03-08 02:30", "2026-03-08 03:00", "2026-11-01 00:59", "2026-11-01 01:00", "2026-11-01 01:30", "2026-11-01 02:00", "2026-11-01 02:30"]:
  t = time.parse(s, "YYYY-MM-DD HH:mm")!
  print(s, t, time.unix(t), time.parts(t), time.format(t, "HH:mm Z"), time.utc(t))
;;"#,
    // the 2026 Berlin changes (east of UTC): 2026-03-29 01:00 UTC skips
    // 02:00-03:00, 2026-10-25 01:00 UTC repeats 02:00-03:00
    r#"for s in ["2026-03-29 01:59", "2026-03-29 02:00", "2026-03-29 02:30", "2026-03-29 03:00", "2026-10-25 01:59", "2026-10-25 02:00", "2026-10-25 02:30", "2026-10-25 03:00"]:
  t = time.parse(s, "YYYY-MM-DD HH:mm")!
  print(s, t, time.unix(t), time.parts(t), time.format(t, "HH:mm Z"), time.utc(t))
for n in [1774746000 - 1, 1774746000, 1792890000 - 1, 1792890000]:
  show = time.from_unix(n)
  print(show, time.parts(show), time.format(show, "HH:mm Z"))
;;"#,
    // RFC 3339: an offset the local zone has at that instant keeps the local
    // zone (seen after a move across a change), any other a fixed offset
    r#"for s in ["2026-07-01T12:00:00-04:00", "2026-07-01T12:00:00-05:00", "2026-07-01T12:00:00+09:00", "2026-07-01T12:00:00Z", "2026-03-08T02:30:00-05:00", "2026-11-01T01:30:00-04:00", "2026-11-01T01:30:00-05:00"]:
  t = time.parse(s)!
  later = t + 4000h
  print(t, later, time.format(later, "Z"), time.parts(later), time.local(t), time.unix(t))
;;"#,
    r#"for s in ["2026-10-04T9:05:07Z", "2026-10-04T09:05:07,5+01:00", "2026-10-04T09:05:07+24:00", "2026-10-04T09:05:07.123456789012Z", "nope", "", "2026-02-30T00:00:00Z", "2026-10-04T23:59:59Zx", "2026-10-04T23:59:59+25:0é", "é", "2026-10-04T23:59:59\tZ", "2026-13-01T00:00:00Z", "2026-10-04T24:00:00Z"]:
  print(time.parse(s))
;;"#,
    // token parsing: zone tokens, normalisation blocking, names, literals
    r#"for p in [["2026-10-04T09:05:07+09:00", "YYYY-MM-DDTHH:mm:ssZ"], ["2026-10-04T09:05:07-0130", "YYYY-MM-DDTHH:mm:ssZZ"], ["26/10/04 09:05:07.250", "YY/MM/DD HH:mm:ss.SSS"], ["20261004", "YYYYMMDD"], ["Sun, 04 oct 2026", "ddd, DD MMM YYYY"], ["04/Dec/2026:23:59:59 +0900", "DD/MMM/YYYY:HH:mm:ss ZZ"], ["2026년10월04일", "YYYY년MM월DD일"], ["2024-02-29", "YYYY-MM-DD"], ["1900-03-01", "YYYY-MM-DD"]]:
  r = time.parse(p[0], p[1])
  print(r, r |> is_ok then time.parts(r!) else nil)
;;"#,
    r#"for p in [["2026-02-30", "YYYY-MM-DD"], ["2026-13-01", "YYYY-MM-DD"], ["2026-00-01", "YYYY-MM-DD"], ["2026-10-00", "YYYY-MM-DD"], ["2026-04-31", "YYYY-MM-DD"], ["2026-1-5", "YYYY-MM-DD"], ["2026-10-04x", "YYYY-MM-DD"], ["2026-10-04", "YYYY-MM-DD HH:mm"], ["09:60", "HH:mm"], ["24:00", "HH:mm"], ["00:00:60", "HH:mm:ss"], ["2026-10-04T09:05:07+9:00", "YYYY-MM-DDTHH:mm:ssZ"], ["2026-10-04T09:05:07+09-00", "YYYY-MM-DDTHH:mm:ssZ"], ["2026-10-04T09:05:07 0900", "YYYY-MM-DDTHH:mm:ssZZ"], ["2026-10-04T09:05:07+09", "YYYY-MM-DDTHH:mm:ssZZ"], ["Foo 04", "MMM DD"], ["Xyz, 04", "ddd, DD"], ["abé", "MMM"], ["é", "MMM"], ["ab", "ddd"], ["2026년10월", "YYYY년MM일"], ["2026\t10", "YYYY MM"], ["x", ""], ["", ""]]:
  print(time.parse(p[0], p[1]))
;;"#,
    r#"t = time.parse("2026-10-04T09:05:07.123+09:00")!
print(time.format(t, "literal text, YYYYMMDD, MMM MM, é\t ZZZ SSSS YYYYY"), time.format(t), time.format(t, ""), time.format(t, nil))
print(time.format(time.utc(t), "Z ZZ"), time.format(time.local(t), "YYYY-MM-DD HH:mm Z"))"#,
    r#"print(time.format(1))"#,
    r#"print(time.format(time.from_unix(0), 1))"#,
    r#"print(time.parse("x", 1))"#,
    r#"print(time.parse(1))"#,
    r#"print(time.utc(1))"#,
    r#"print(time.local("x"))"#,
    r#"print(time.unix(nil))"#,
    r#"print(time.parts([]))"#,
    // now: only its shape, and that it is local
    r#"n = time.now()
print(type_of(n), time.unix(n) > 1790000000, time.format(n, "Z") == time.format(time.local(n), "Z"), time.now() >= n)"#,
];

/// `bin -e src` with `TZ` set to `tz` (or removed): stdout, stderr, status.
fn run(bin: &std::path::Path, src: &str, tz: Option<&str>) -> (String, String, Option<i32>) {
    let mut cmd = Command::new(bin);
    cmd.args(["-e", &format!("import time\n{src}")])
        .current_dir(repo());
    match tz {
        Some(tz) => cmd.env("TZ", tz),
        None => cmd.env_remove("TZ"),
    };
    let o = common::output(&mut cmd).unwrap();
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        o.status.code(),
    )
}

/// How many zone and program pairs have a fixed expected output, and
/// how many programs run with `TZ` unset.
const ZONE_CASES: usize = 168;
const UNSET_CASES: usize = 21;

fn entries(bin: &std::path::Path) -> Vec<Entry> {
    let mut pairs = Vec::new();
    for tz in ZONES {
        for src in PROGRAMS {
            pairs.push((*tz, *src));
        }
    }
    common::par_map(&pairs, |(tz, src)| {
        let (out, err, code) = run(bin, src, Some(tz));
        Entry::new(format!(
            "TZ={tz:?} {}",
            expected::short_label(&format!("{src:?}"))
        ))
        .field("stdout", out)
        .field("stderr", err)
        .field("exit", code.map_or("signal".into(), |c| c.to_string()))
    })
}

#[test]
fn time_runs_match_the_expected_output_under_each_tz() {
    assert_eq!(ZONES.len() * PROGRAMS.len(), ZONE_CASES, "cases");
    let got = entries(&fung_bin());
    expected::verify(
        "time/programs.txt",
        "`time` programs under each TZ (crates/fung-cli/tests/time_expected.rs), each run with `fung -e`.\n\
         Update: FUNG_EXPECTED=bless cargo test -p fung-cli --test time_expected",
        64 * 1024,
        &got,
    );
}

/// With `TZ` unset the local zone is `/etc/localtime`.
#[test]
fn an_unset_tz_reads_etc_localtime() {
    assert_eq!(PROGRAMS.len(), UNSET_CASES, "programs");
    let bin = fung_bin();
    let bad: Vec<String> = common::par_map(PROGRAMS, |src| {
        let (unset, file) = (run(&bin, src, None), run(&bin, src, Some("/etc/localtime")));
        (unset != file).then(|| format!("{src}\n  unset {unset:?}\n  file  {file:?}"))
    })
    .into_iter()
    .flatten()
    .collect();
    assert!(bad.is_empty(), "{} differ:\n{}", bad.len(), bad.join("\n"));
}

/// East of UTC too, a skipped wall time moves forward by the gap and a
/// repeated one is its earlier instant (06: 시간 형식).
#[test]
fn berlin_changes_follow_the_wall_time_rule() {
    let src = r#"print(time.parse("2026-03-29 02:30", "YYYY-MM-DD HH:mm")!, time.parse("2026-10-25 02:30", "YYYY-MM-DD HH:mm")!)"#;
    let (out, err, code) = run(&fung_bin(), src, Some("Europe/Berlin"));
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (
            "2026-03-29T03:30:00+02:00 2026-10-25T02:30:00+02:00\n",
            "",
            Some(0)
        )
    );
}

/// In New York the skipped 02:30 moves forward by the gap (03:30 daylight
/// time), and the repeated 01:30 is its earlier (daylight) instant.
#[test]
fn new_york_changes_follow_the_wall_time_rule() {
    let src = r#"print(time.parse("2026-03-08 02:30", "YYYY-MM-DD HH:mm")!, time.parse("2026-11-01 01:30", "YYYY-MM-DD HH:mm")!)"#;
    let (out, err, code) = run(&fung_bin(), src, Some("America/New_York"));
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (
            "2026-03-08T03:30:00-04:00 2026-11-01T01:30:00-04:00\n",
            "",
            Some(0)
        )
    );
}
