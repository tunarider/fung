//! Builds `spec/unicode.txt` from the Unicode Character Database.
//!
//! Builds `spec/unicode_nfc.txt` (the normalization data) from it too.
//!
//! The files are fixed data the front end reads; this test regenerates or
//! checks them when `FUNG_UCD` names a directory holding the UCD files
//! `UnicodeData.txt`, `DerivedCoreProperties.txt`,
//! `CompositionExclusions.txt` and `EastAsianWidth.txt` (Unicode 17.0.0,
//! <https://www.unicode.org/Public/17.0.0/ucd/>). Without `FUNG_UCD` it
//! does nothing. `FUNG_UPDATE_SPEC=1` rewrites the files instead of
//! comparing.

#[test]
fn unicode_spec_file() {
    let Some(dir) = std::env::var_os("FUNG_UCD") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let read = |name: &str| {
        std::fs::read_to_string(dir.join(name))
            .unwrap_or_else(|e| panic!("{}: {e}", dir.join(name).display()))
    };
    let data = read("UnicodeData.txt");
    let derived = read("DerivedCoreProperties.txt");
    let files = [
        (
            "unicode.txt",
            spec(&data, &derived, &read("EastAsianWidth.txt")),
        ),
        (
            "unicode_nfc.txt",
            nfc_spec(&data, &read("CompositionExclusions.txt"), &derived),
        ),
    ];
    for (name, got) in files {
        let path = format!("{}/../../spec/{name}", env!("CARGO_MANIFEST_DIR"));
        if std::env::var_os("FUNG_UPDATE_SPEC").is_some() {
            std::fs::write(&path, &got).unwrap();
            continue;
        }
        let want = std::fs::read_to_string(&path).unwrap();
        assert!(
            got == want,
            "spec/{name} differs from the UCD; rewrite it with FUNG_UPDATE_SPEC=1"
        );
    }
}

/// The UCD version named by the first line of `DerivedCoreProperties.txt`.
fn version(derived: &str) -> &str {
    derived
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("# DerivedCoreProperties-"))
        .and_then(|l| l.strip_suffix(".txt"))
        .expect("the DerivedCoreProperties version line")
}

/// `spec/unicode_nfc.txt`: canonical combining classes, one-level
/// canonical decompositions, and the primary composites (pairs that are
/// not fully excluded from composition: not listed in
/// `CompositionExclusions.txt` and not a non-starter decomposition).
/// Hangul syllables are algorithmic and not listed.
fn nfc_spec(unicode_data: &str, exclusions: &str, derived: &str) -> String {
    let mut ccc: Vec<(u32, u32)> = Vec::new();
    let mut decomp: Vec<(u32, Vec<u32>)> = Vec::new();
    for line in unicode_data.lines() {
        let f: Vec<&str> = line.split(';').collect();
        let cp = u32::from_str_radix(f[0], 16).unwrap();
        let class: u32 = f[3].parse().unwrap();
        if class != 0 {
            ccc.push((cp, class));
        }
        if !f[5].is_empty() && !f[5].starts_with('<') {
            let to = f[5]
                .split(' ')
                .map(|h| u32::from_str_radix(h, 16).unwrap())
                .collect();
            decomp.push((cp, to));
        }
    }
    let class_of = |cp: u32| ccc.iter().find(|&&(c, _)| c == cp).map_or(0, |&(_, k)| k);
    let excluded: Vec<u32> = exclusions
        .lines()
        .filter_map(|l| {
            let body = l.split('#').next().unwrap().trim();
            (!body.is_empty()).then(|| u32::from_str_radix(body, 16).unwrap())
        })
        .collect();
    let mut out = format!(
        "# Normalization (NFC) data for fung names (UCD {}). Generated: FUNG_UPDATE_SPEC=1 FUNG_UCD=<ucd dir> cargo test -p fung-front unicode_spec_file; do not edit.\n\
         # ccc <cp> <class>              canonical combining class (0 when not listed)\n\
         # decomp <cp> <cp> [<cp>]       canonical decomposition, one level\n\
         # compose <first> <second> <cp> primary composite\n",
        version(derived)
    );
    for &(cp, class) in &ccc {
        out.push_str(&format!("ccc {cp:04X} {class}\n"));
    }
    for (cp, to) in &decomp {
        let to: Vec<String> = to.iter().map(|c| format!("{c:04X}")).collect();
        out.push_str(&format!("decomp {cp:04X} {}\n", to.join(" ")));
    }
    for (cp, to) in &decomp {
        let composable =
            to.len() == 2 && !excluded.contains(cp) && class_of(*cp) == 0 && class_of(to[0]) == 0;
        if composable {
            out.push_str(&format!("compose {:04X} {:04X} {cp:04X}\n", to[0], to[1]));
        }
    }
    out
}

/// The classes, in file order.
const CLASSES: [&str; 6] = [
    "name_start",
    "name_continue",
    "upper",
    "print",
    "wide",
    "zero",
];

/// `spec/unicode.txt` from the text of `UnicodeData.txt`,
/// `DerivedCoreProperties.txt` and `EastAsianWidth.txt`.
fn spec(unicode_data: &str, derived: &str, east_asian_width: &str) -> String {
    const N: usize = 0x11_0000;
    // General category per code point ("" for unassigned).
    let mut gc: Vec<&str> = vec![""; N];
    let mut first: Option<usize> = None;
    for line in unicode_data.lines() {
        let f: Vec<&str> = line.split(';').collect();
        let cp = usize::from_str_radix(f[0], 16).unwrap();
        if f[1].ends_with(", First>") {
            first = Some(cp);
            continue;
        }
        let lo = if f[1].ends_with(", Last>") {
            first.take().expect("a First entry before Last")
        } else {
            cp
        };
        gc[lo..=cp].fill(f[2]);
    }
    let mut xid_start = vec![false; N];
    let mut xid_continue = vec![false; N];
    for line in derived.lines() {
        let body = line.split('#').next().unwrap().trim();
        let Some((cps, prop)) = body.split_once(';') else {
            continue;
        };
        let set = match prop.trim() {
            "XID_Start" => &mut xid_start,
            "XID_Continue" => &mut xid_continue,
            _ => continue,
        };
        let cps = cps.trim();
        let (lo, hi) = cps.split_once("..").unwrap_or((cps, cps));
        let lo = usize::from_str_radix(lo, 16).unwrap();
        let hi = usize::from_str_radix(hi, 16).unwrap();
        set[lo..=hi].fill(true);
    }
    // East Asian Width W or F: two columns (UAX #11)
    let mut wide = vec![false; N];
    for line in east_asian_width.lines() {
        let body = line.split('#').next().unwrap().trim();
        let Some((cps, w)) = body.split_once(';') else {
            continue;
        };
        if !matches!(w.trim(), "W" | "F") {
            continue;
        }
        let cps = cps.trim();
        let (lo, hi) = cps.split_once("..").unwrap_or((cps, cps));
        let lo = usize::from_str_radix(lo, 16).unwrap();
        let hi = usize::from_str_radix(hi, 16).unwrap();
        wide[lo..=hi].fill(true);
    }
    let version = version(derived);
    let mut out = format!(
        "# Unicode character classes for fung (UCD {version}). Generated: FUNG_UPDATE_SPEC=1 FUNG_UCD=<ucd dir> cargo test -p fung-front unicode_spec_file; do not edit.\n\
         # <class> <first> <last>   inclusive run of code points (hex): name_start XID_Start, name_continue XID_Continue, upper Lu, print L M N P S and U+0020, wide East Asian Width W or F (two columns), zero Mn Me Cf but U+00AD, the Hangul medial and final jamo U+1160..U+11FF and U+D7B0..U+D7FF, and the emoji skin tone modifiers U+1F3FB..U+1F3FF (no column)\n"
    );
    for class in CLASSES {
        let has = |cp: usize| match class {
            "name_start" => xid_start[cp],
            "name_continue" => xid_continue[cp],
            "upper" => gc[cp] == "Lu",
            "wide" => wide[cp],
            "zero" => {
                (matches!(gc[cp], "Mn" | "Me" | "Cf") && cp != 0xAD)
                    || (0x1160..=0x11FF).contains(&cp)
                    || (0xD7B0..=0xD7FF).contains(&cp)
                    || (0x1F3FB..=0x1F3FF).contains(&cp)
            }
            _ => cp == 0x20 || gc[cp].starts_with(['L', 'M', 'N', 'P', 'S']),
        };
        let mut cp = 0;
        while cp < N {
            if !has(cp) {
                cp += 1;
                continue;
            }
            let lo = cp;
            while cp + 1 < N && has(cp + 1) {
                cp += 1;
            }
            out.push_str(&format!("{class} {lo:04X} {cp:04X}\n"));
            cp += 1;
        }
    }
    out
}
