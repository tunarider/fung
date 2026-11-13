//! fung's character classes, read from `spec/unicode.txt`: names
//! follow UAX #31 (`XID_Start`, `XID_Continue`), tags start with an
//! uppercase letter (Lu), and displayed strings show the printable
//! characters (L, M, N, P, S and U+0020) as themselves, and everything
//! that counts terminal columns (the formatter's alignment, the
//! diagnostics' underlines, the REPL's cursor) counts them by grapheme
//! cluster (UAX #29) with one rule, `cluster_width` (L17). The file is
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

/// A character that shows nothing: the Unicode property
/// Default_Ignorable_Code_Point (joiners, variation selectors, fillers,
/// format controls). Several of them may continue an identifier by
/// UAX #31, but a name with one looks the same as the name without it
/// and is another name, so no name or symbol holds one (D102). The
/// ranges are the property's, from DerivedCoreProperties.txt.
pub fn is_invisible(c: char) -> bool {
    matches!(
        c as u32,
        0x00AD
            | 0x034F
            | 0x061C
            | 0x115F..=0x1160
            | 0x17B4..=0x17B5
            | 0x180B..=0x180F
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x206F
            | 0x3164
            | 0xFE00..=0xFE0F
            | 0xFEFF
            | 0xFFA0
            | 0xFFF0..=0xFFF8
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0000..=0xE0FFF
    )
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

/// The columns `c` takes alone (UAX #11): 2 for a wide character, 0 for
/// a combining or format character, else 1. Only `cluster_width` uses it:
/// a terminal draws clusters, not code points.
fn char_width(c: char) -> usize {
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

/// A code point's `Grapheme_Cluster_Break` value (UAX #29), as far as
/// the rules below tell them apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gcb {
    Cr,
    Lf,
    Control,
    Extend,
    Zwj,
    Ri,
    Prepend,
    SpacingMark,
    L,
    V,
    T,
    Lv,
    Lvt,
    Other,
}

fn gcb(c: char) -> Gcb {
    if c.is_ascii() {
        return match c {
            '\r' => Gcb::Cr,
            '\n' => Gcb::Lf,
            '\0'..='\x1f' | '\x7f' => Gcb::Control,
            _ => Gcb::Other,
        };
    }
    const CLASSES: [(&str, Gcb); 13] = [
        ("gcb_cr", Gcb::Cr),
        ("gcb_lf", Gcb::Lf),
        ("gcb_control", Gcb::Control),
        ("gcb_extend", Gcb::Extend),
        ("gcb_zwj", Gcb::Zwj),
        ("gcb_ri", Gcb::Ri),
        ("gcb_prepend", Gcb::Prepend),
        ("gcb_spacingmark", Gcb::SpacingMark),
        ("gcb_l", Gcb::L),
        ("gcb_v", Gcb::V),
        ("gcb_t", Gcb::T),
        ("gcb_lv", Gcb::Lv),
        ("gcb_lvt", Gcb::Lvt),
    ];
    CLASSES
        .iter()
        .find(|(class, _)| contains(ranges(class), c))
        .map_or(Gcb::Other, |&(_, g)| g)
}

/// `Indic_Conjunct_Break` (for GB9c).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Incb {
    Consonant,
    Extend,
    Linker,
    None,
}

fn incb(c: char) -> Incb {
    if c.is_ascii() {
        Incb::None
    } else if contains(ranges("incb_linker"), c) {
        Incb::Linker
    } else if contains(ranges("incb_consonant"), c) {
        Incb::Consonant
    } else if contains(ranges("incb_extend"), c) {
        Incb::Extend
    } else {
        Incb::None
    }
}

fn ext_pict(c: char) -> bool {
    !c.is_ascii() && contains(ranges("ext_pict"), c)
}

/// What the rules remember of the cluster so far.
#[derive(Default)]
struct Run {
    /// Regional indicators in a row just before (GB12, GB13).
    ri: usize,
    /// An `Extended_Pictographic` then `Extend`* (GB11), and a ZWJ after
    /// it.
    pict: bool,
    pict_zwj: bool,
    /// An InCB consonant then [Extend Linker]* (GB9c), and a linker
    /// among them.
    conj: bool,
    conj_linker: bool,
}

impl Run {
    /// Takes `c` in: what the rules need to know after it.
    fn step(&mut self, c: char, g: Gcb) {
        self.ri = if g == Gcb::Ri { self.ri + 1 } else { 0 };
        if ext_pict(c) {
            (self.pict, self.pict_zwj) = (true, false);
        } else if self.pict && g == Gcb::Extend {
        } else if self.pict && g == Gcb::Zwj && !self.pict_zwj {
            self.pict_zwj = true;
        } else {
            (self.pict, self.pict_zwj) = (false, false);
        }
        match incb(c) {
            Incb::Consonant => (self.conj, self.conj_linker) = (true, false),
            Incb::Linker if self.conj => self.conj_linker = true,
            Incb::Extend if self.conj => {}
            _ => (self.conj, self.conj_linker) = (false, false),
        }
    }
}

/// Whether there is a cluster boundary between `prev` and `c` (UAX #29
/// extended grapheme clusters, GB3 to GB999), given what came before.
fn breaks(run: &Run, prev: Gcb, c: char, g: Gcb) -> bool {
    use Gcb::*;
    match (prev, g) {
        (Cr, Lf) => false,
        (Control | Cr | Lf, _) | (_, Control | Cr | Lf) => true,
        (L, L | V | Lv | Lvt) | (Lv | V, V | T) | (Lvt | T, T) => false,
        (_, Extend | Zwj | SpacingMark) | (Prepend, _) => false,
        _ if run.conj && run.conj_linker && incb(c) == Incb::Consonant => false,
        (Zwj, _) if run.pict_zwj && ext_pict(c) => false,
        (Ri, Ri) => run.ri.is_multiple_of(2),
        _ => true,
    }
}

/// The byte offsets where `s`'s grapheme clusters start, and its length.
fn boundaries(s: &str) -> Vec<usize> {
    let mut out = vec![0];
    let mut run = Run::default();
    let mut prev: Option<Gcb> = None;
    for (at, c) in s.char_indices() {
        let g = gcb(c);
        if let Some(p) = prev
            && breaks(&run, p, c, g)
        {
            out.push(at);
        }
        run.step(c, g);
        prev = Some(g);
    }
    if !s.is_empty() {
        out.push(s.len());
    }
    out
}

/// `s`'s grapheme clusters (UAX #29 extended clusters): what a terminal
/// draws as one thing (`e` + combining acute, a ZWJ emoji family, a flag).
pub fn clusters(s: &str) -> impl Iterator<Item = &str> {
    let b = boundaries(s);
    (1..b.len()).map(move |i| &s[b[i - 1]..b[i]])
}

/// The cluster boundaries of `cs` as indices into it (the REPL's buffer
/// of characters): 0, then the start of each next cluster, then
/// `cs.len()`.
pub fn char_boundaries(cs: &[char]) -> Vec<usize> {
    let s: String = cs.iter().collect();
    let b = boundaries(&s);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    for (chars, (at, _)) in s.char_indices().enumerate() {
        while i < b.len() && b[i] == at {
            out.push(chars);
            i += 1;
        }
    }
    if !cs.is_empty() {
        out.push(cs.len());
    } else {
        out.push(0);
    }
    out.dedup();
    out
}

/// The columns a grapheme cluster takes in a terminal (L17): 2 with VS16
/// (emoji presentation) or an emoji-presentation first character or a
/// flag; 1 with VS15 (text presentation); else its first character's
/// width (UAX #11), which a base gives and its marks do not change.
pub fn cluster_width(g: &str) -> usize {
    let mut cs = g.chars();
    let Some(first) = cs.next() else {
        return 0;
    };
    if g.contains('\u{FE0F}') {
        return 2;
    }
    if g.contains('\u{FE0E}') {
        return 1;
    }
    if !first.is_ascii() && (contains(ranges("emoji_pres"), first) || gcb(first) == Gcb::Ri) {
        return 2;
    }
    char_width(first)
}

/// The columns `s` takes: the sum of its clusters' widths.
pub fn display_width(s: &str) -> usize {
    if s.is_ascii() {
        return s.len();
    }
    clusters(s).map(cluster_width).sum()
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
            assert_eq!(char_width(c), w, "{c:?}");
        }
    }

    /// A cluster's columns (L17): an emoji sequence joined by ZWJ is one
    /// cluster of 2, VS16 makes the emoji presentation (2), VS15 the text
    /// one (1), a flag (two regional indicators) is 2, a base with its
    /// combining marks takes the base's width, Hangul jamo make a
    /// syllable of 2.
    #[test]
    fn cluster_widths() {
        for (s, w) in [
            ("a", 1),
            ("한", 2),
            ("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}", 2),
            ("\u{2764}\u{FE0F}", 2),
            ("\u{2764}", 1),
            ("\u{2764}\u{FE0E}", 1),
            ("\u{1F600}\u{FE0E}", 1),
            ("\u{1F1F0}\u{1F1F7}", 2),
            ("e\u{301}", 1),
            ("\u{1100}\u{1161}\u{11A8}", 2),
            ("\u{1F44D}\u{1F3FD}", 2),
            ("1\u{FE0F}\u{20E3}", 2),
        ] {
            let g: Vec<&str> = clusters(s).collect();
            assert_eq!(g, [s], "{s:?} is one cluster");
            assert_eq!(cluster_width(s), w, "{s:?}");
        }
        assert_eq!(
            display_width("a한\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{2764}\u{FE0F}"),
            7
        );
        assert_eq!(
            clusters("ab\r\n\u{1F1F0}\u{1F1F7}\u{1F1FA}").collect::<Vec<_>>(),
            ["a", "b", "\r\n", "\u{1F1F0}\u{1F1F7}", "\u{1F1FA}"]
        );
    }

    /// The boundaries of UAX #29's own test file, when `FUNG_UCD` names a
    /// directory holding `GraphemeBreakTest.txt` (17.0.0).
    #[test]
    fn grapheme_break_test() {
        let Some(dir) = std::env::var_os("FUNG_UCD") else {
            return;
        };
        let text =
            std::fs::read_to_string(std::path::Path::new(&dir).join("GraphemeBreakTest.txt"))
                .expect("GraphemeBreakTest.txt");
        let mut n = 0;
        for line in text.lines() {
            let body = line.split('#').next().unwrap().trim();
            if body.is_empty() {
                continue;
            }
            // `÷ 0020 × 0308 ÷`: the clusters the string falls into
            let mut want: Vec<String> = Vec::new();
            let mut cur = String::new();
            for tok in body.split_whitespace() {
                match tok {
                    "÷" => {
                        if !cur.is_empty() {
                            want.push(std::mem::take(&mut cur));
                        }
                    }
                    "×" => {}
                    hex => cur.push(char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap()),
                }
            }
            let whole: String = want.concat();
            let got: Vec<String> = clusters(&whole).map(String::from).collect();
            assert_eq!(got, want, "{line}");
            n += 1;
        }
        assert!(n > 700, "{n} cases");
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
