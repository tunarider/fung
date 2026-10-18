//! Unicode Normalization Form C for names (01: 식별자): the lexer and
//! `sym` normalize a name so that its composed and decomposed spellings
//! (`é` and `e` + U+0301) are one name. The data is
//! `spec/unicode_nfc.txt`, generated with the other tables.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

const SPEC: &str = include_str!("../../../../spec/unicode_nfc.txt");

struct Data {
    ccc: HashMap<u32, u8>,
    decomp: HashMap<u32, Vec<u32>>,
    compose: HashMap<(u32, u32), u32>,
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut d = Data {
            ccc: HashMap::new(),
            decomp: HashMap::new(),
            compose: HashMap::new(),
        };
        let hex = |s: &str| u32::from_str_radix(s, 16).expect("spec/unicode_nfc.txt: hex");
        for line in SPEC.lines().filter(|l| !l.starts_with('#')) {
            let f: Vec<&str> = line.split(' ').collect();
            match f[0] {
                "ccc" => {
                    d.ccc.insert(hex(f[1]), f[2].parse().expect("a class"));
                }
                "decomp" => {
                    d.decomp
                        .insert(hex(f[1]), f[2..].iter().map(|h| hex(h)).collect());
                }
                "compose" => {
                    d.compose.insert((hex(f[1]), hex(f[2])), hex(f[3]));
                }
                _ => panic!("spec/unicode_nfc.txt: bad line {line:?}"),
            }
        }
        d
    })
}

// Hangul syllables (Unicode 3.12), composed and decomposed by arithmetic.
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

fn ccc(c: u32) -> u8 {
    data().ccc.get(&c).copied().unwrap_or(0)
}

fn decompose(c: u32, out: &mut Vec<u32>) {
    if (S_BASE..S_BASE + S_COUNT).contains(&c) {
        let s = c - S_BASE;
        out.push(L_BASE + s / N_COUNT);
        out.push(V_BASE + (s % N_COUNT) / T_COUNT);
        if !s.is_multiple_of(T_COUNT) {
            out.push(T_BASE + s % T_COUNT);
        }
    } else if let Some(to) = data().decomp.get(&c) {
        for &x in to {
            decompose(x, out);
        }
    } else {
        out.push(c);
    }
}

fn compose(a: u32, b: u32) -> Option<u32> {
    if (L_BASE..L_BASE + L_COUNT).contains(&a) && (V_BASE..V_BASE + V_COUNT).contains(&b) {
        return Some(S_BASE + ((a - L_BASE) * V_COUNT + (b - V_BASE)) * T_COUNT);
    }
    if (S_BASE..S_BASE + S_COUNT).contains(&a)
        && (a - S_BASE).is_multiple_of(T_COUNT)
        && (T_BASE + 1..T_BASE + T_COUNT).contains(&b)
    {
        return Some(a + (b - T_BASE));
    }
    data().compose.get(&(a, b)).copied()
}

/// `s` in Normalization Form C: canonical decomposition, canonical
/// ordering of combining marks, then canonical composition.
pub fn nfc(s: &str) -> Cow<'_, str> {
    if s.is_ascii() {
        return Cow::Borrowed(s);
    }
    let mut d = Vec::with_capacity(s.len());
    for c in s.chars() {
        decompose(c as u32, &mut d);
    }
    // Canonical ordering: a stable sort of each run of marks by class.
    for i in 1..d.len() {
        let mut j = i;
        while j > 0 && ccc(d[j]) != 0 && ccc(d[j - 1]) > ccc(d[j]) {
            d.swap(j - 1, j);
            j -= 1;
        }
    }
    let mut out: Vec<u32> = Vec::with_capacity(d.len());
    let mut starter: Option<usize> = None;
    // The class of the last character after the starter; None when the
    // next one would be adjacent to it.
    let mut last: Option<u8> = None;
    for c in d {
        let class = ccc(c);
        if let Some(si) = starter {
            let blocked = matches!(last, Some(l) if l == 0 || l >= class);
            if !blocked && let Some(x) = compose(out[si], c) {
                out[si] = x;
                continue;
            }
        }
        if class == 0 {
            starter = Some(out.len());
            last = None;
        } else {
            last = Some(class);
        }
        out.push(c);
    }
    let text: String = out
        .into_iter()
        .map(|c| char::from_u32(c).expect("a scalar value"))
        .collect();
    if text == s {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(text)
    }
}

#[cfg(test)]
mod tests {
    use super::nfc;

    #[test]
    fn composes_canonical_equivalents() {
        for (s, want) in [
            ("abc", "abc"),
            ("cafe\u{301}", "caf\u{e9}"),
            ("caf\u{e9}", "caf\u{e9}"),
            ("\u{1100}\u{1161}\u{11a8}", "\u{ac01}"),
            ("\u{ac01}", "\u{ac01}"),
            ("파일", "파일"),
            // An excluded composite stays decomposed.
            ("\u{958}", "\u{915}\u{93c}"),
            // A singleton maps to its target.
            ("\u{212b}", "\u{c5}"),
            // Marks are put in canonical order before composing.
            ("a\u{323}\u{302}", "\u{1ead}"),
            ("a\u{302}\u{323}", "\u{1ead}"),
            // A mark of the same class blocks the second one.
            ("a\u{301}\u{301}", "\u{e1}\u{301}"),
            ("\u{301}a", "\u{301}a"),
            ("न\u{93f}", "न\u{93f}"),
        ] {
            assert_eq!(nfc(s), want, "{s:?}");
        }
    }
}
