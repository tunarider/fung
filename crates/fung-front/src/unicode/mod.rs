//! fung's character classes, read from `spec/unicode.txt`: names
//! follow UAX #31 (`XID_Start`, `XID_Continue`), tags start with an
//! uppercase letter (Lu), and displayed strings show the printable
//! characters (L, M, N, P, S and U+0020) as themselves, and the
//! formatter aligns by display width (`width`, UAX #11). The file is
//! generated from the Unicode Character Database (`generate.rs`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

const SPEC: &str = include_str!("../../../../spec/unicode.txt");

/// Class name → sorted, disjoint inclusive runs `(first, last)`.
fn tables() -> &'static BTreeMap<&'static str, Vec<(u32, u32)>> {
    static TABLES: OnceLock<BTreeMap<&'static str, Vec<(u32, u32)>>> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut out: BTreeMap<&str, Vec<(u32, u32)>> = BTreeMap::new();
        for line in SPEC.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut f = line.split(' ');
            let (Some(class), Some(lo), Some(hi)) = (f.next(), f.next(), f.next()) else {
                panic!("spec/unicode.txt: bad line {line:?}");
            };
            let hex = |s: &str| {
                u32::from_str_radix(s, 16)
                    .unwrap_or_else(|_| panic!("spec/unicode.txt: bad line {line:?}"))
            };
            out.entry(class).or_default().push((hex(lo), hex(hi)));
        }
        out
    })
}

/// The runs of one class; an unknown class is a build error in the spec.
fn ranges(class: &str) -> &'static [(u32, u32)] {
    tables()
        .get(class)
        .unwrap_or_else(|| panic!("spec/unicode.txt: no class {class}"))
}

/// Binary search for the run that would hold `c`.
fn contains(runs: &[(u32, u32)], c: char) -> bool {
    let u = c as u32;
    let i = runs.partition_point(|&(_, hi)| hi < u);
    runs.get(i).is_some_and(|&(lo, _)| lo <= u)
}

/// May start a name: `XID_Start` (UAX #31). `_` is the caller's.
pub fn is_name_start(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphabetic();
    }
    contains(ranges("name_start"), c)
}

/// May continue a name: `XID_Continue` (UAX #31), which holds the
/// combining marks, digits and `_`.
pub fn is_name_continue(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric() || c == '_';
    }
    contains(ranges("name_continue"), c)
}

/// An uppercase letter (general category Lu): what makes a tag.
pub fn is_upper(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_uppercase();
    }
    contains(ranges("upper"), c)
}

/// Shown as itself when a string is displayed: a letter, mark, number,
/// punctuation or symbol (L, M, N, P, S), or U+0020.
pub fn is_print(c: char) -> bool {
    if c.is_ascii() {
        return matches!(c, ' '..='~');
    }
    contains(ranges("print"), c)
}

/// The columns `c` takes in a terminal (UAX #11): 2 for a wide
/// character, 0 for a combining or format character, else 1. The
/// formatter aligns by it.
pub fn width(c: char) -> usize {
    if c.is_ascii() {
        return 1;
    }
    if contains(ranges("zero"), c) {
        0
    } else if contains(ranges("wide"), c) {
        2
    } else {
        1
    }
}

mod nfc;
pub use nfc::nfc;

#[cfg(test)]
mod generate;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_classes() {
        for c in ['a', 'Z', 'é', '파', 'न', 'ß', 'ǅ', 'ª', '\u{1C89}'] {
            assert!(is_name_start(c), "{c:?}");
        }
        for c in ['5', '_', '\u{301}', '\u{94D}', '²', '①', '\u{378}', ' '] {
            assert!(!is_name_start(c), "{c:?}");
        }
        for c in ['a', '5', '_', '\u{94D}', '\u{301}', '\u{93F}', '٣', '𝟘'] {
            assert!(is_name_continue(c), "{c:?}");
        }
        for c in ['-', ' ', '²', '①', '$', '\u{378}'] {
            assert!(!is_name_continue(c), "{c:?}");
        }
    }

    #[test]
    fn upper() {
        for c in ['A', 'Z', 'Σ', 'É', '\u{1C89}'] {
            assert!(is_upper(c), "{c:?}");
        }
        for c in ['a', 'ß', 'σ', 'ǅ', 'Ⅻ', 'Ⓐ', '한', '_'] {
            assert!(!is_upper(c), "{c:?}");
        }
    }

    #[test]
    fn print_classes() {
        for c in ['a', ' ', '_', '²', 'Ⅻ', '\u{93F}', '한', '😀'] {
            assert!(is_print(c), "{c:?}");
        }
        for c in [
            '\u{378}', '\u{A0}', '\u{3000}', '\u{85}', '\u{180E}', '\u{200B}', '\u{E000}', '\n',
        ] {
            assert!(!is_print(c), "{c:?}");
        }
    }

    #[test]
    fn widths() {
        for (c, w) in [
            ('a', 1),
            ('한', 2),
            ('漢', 2),
            ('😀', 2),
            ('\u{3000}', 2),
            ('\u{301}', 0),
            ('\u{200B}', 0),
            ('\u{1160}', 0),
            ('\u{D7B0}', 0),
            ('\u{1F3FD}', 0),
            ('\u{AD}', 1),
            ('é', 1),
            ('\u{FFFD}', 1),
        ] {
            assert_eq!(width(c), w, "{c:?}");
        }
    }

    /// The ASCII fast paths agree with the tables.
    #[test]
    fn ascii_matches_tables() {
        for u in 0..0x80u8 {
            let c = u as char;
            assert_eq!(is_name_start(c), contains(ranges("name_start"), c), "{c:?}");
            assert_eq!(
                is_name_continue(c),
                contains(ranges("name_continue"), c),
                "{c:?}"
            );
            assert_eq!(is_upper(c), contains(ranges("upper"), c), "{c:?}");
            assert_eq!(is_print(c), contains(ranges("print"), c), "{c:?}");
        }
    }
}
