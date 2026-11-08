//! Strings as bytes (02, L4). A fung string is a sequence of bytes, and
//! the character operations read it as UTF-8. A byte that does not start
//! a valid UTF-8 sequence is a character of its own, one byte long, so
//! cutting a string into characters and joining them again gives back
//! the same bytes.

/// One character of a string: a decoded `char`, or a byte outside valid
/// UTF-8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Char(char),
    Byte(u8),
}

impl Unit {
    /// Its length in bytes.
    pub fn byte_len(self) -> usize {
        match self {
            Unit::Char(c) => c.len_utf8(),
            Unit::Byte(_) => 1,
        }
    }
}

/// The first character of `b`, or `None` when `b` is empty.
pub fn first(b: &[u8]) -> Option<Unit> {
    let &b0 = b.first()?;
    let n = match b0 {
        0..=0x7F => return Some(Unit::Char(b0 as char)),
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return Some(Unit::Byte(b0)),
    };
    // from_utf8 rejects overlong forms, surrogates and values past U+10FFFF.
    let c = b
        .get(..n)
        .and_then(|s| std::str::from_utf8(s).ok())
        .and_then(|s| s.chars().next());
    Some(c.map_or(Unit::Byte(b0), Unit::Char))
}

/// The last character of `b`, or `None` when `b` is empty: the same one
/// `units` ends with, found from the end. A valid character starts at a
/// byte that is no continuation byte, so reading from the start always
/// reaches its first byte; the last character is the valid one ending
/// the string, else its last byte alone.
pub fn last(b: &[u8]) -> Option<Unit> {
    let &end = b.last()?;
    if end.is_ascii() {
        return Some(Unit::Char(end as char));
    }
    for n in 2..=4.min(b.len()) {
        let tail = &b[b.len() - n..];
        if let Some(Unit::Char(c)) = first(tail)
            && c.len_utf8() == n
        {
            return Some(Unit::Char(c));
        }
    }
    Some(Unit::Byte(end))
}

/// The characters of `b` with their byte offsets.
pub fn units(b: &[u8]) -> Units<'_> {
    Units { b, off: 0 }
}

pub struct Units<'a> {
    b: &'a [u8],
    off: usize,
}

impl Iterator for Units<'_> {
    type Item = (usize, Unit);

    fn next(&mut self) -> Option<(usize, Unit)> {
        let u = first(&self.b[self.off..])?;
        let at = self.off;
        self.off += u.byte_len();
        Some((at, u))
    }
}

/// The number of characters in `b`. Valid stretches are counted as
/// `str`s; every byte of an invalid stretch is one character (no
/// character can start at a continuation byte).
pub fn char_count(b: &[u8]) -> usize {
    if b.is_ascii() {
        return b.len();
    }
    b.utf8_chunks()
        .map(|c| c.valid().chars().count() + c.invalid().len())
        .sum()
}

/// The byte offset of character `i` of `b` (`b.len()` past the end).
/// Valid stretches are skipped by counting their characters a block at
/// a time; each byte of an invalid stretch is one character (VA12). The
/// bytes are checked a window at a time, so only the part up to the
/// character is read.
pub fn byte_at(b: &[u8], i: usize) -> usize {
    const WINDOW: usize = 4096;
    let (mut left, mut pos) = (i, 0);
    while pos < b.len() {
        let end = (pos + WINDOW).min(b.len());
        let (good, bad) = match std::str::from_utf8(&b[pos..end]) {
            Ok(_) => (end - pos, 0),
            // A sequence cut by the window's end is read with the next.
            Err(e) => match e.error_len() {
                Some(n) => (e.valid_up_to(), n),
                None if end == b.len() => (e.valid_up_to(), end - pos - e.valid_up_to()),
                None => (e.valid_up_to(), 0),
            },
        };
        match utf8_offset(&b[pos..pos + good], left) {
            Ok(off) => return pos + off,
            Err(n) => left -= n,
        }
        pos += good;
        if left < bad {
            return pos + left;
        }
        left -= bad;
        pos += bad;
    }
    b.len()
}

/// Whether a byte starts a character of valid UTF-8 (is no continuation
/// byte).
fn starts_char(b: u8) -> bool {
    (b as i8) >= -0x40
}

/// In valid UTF-8 `v`: the byte offset of character `n`, or `Err` with
/// the number of characters when there are no more than `n`. The
/// characters of a 64-byte block are counted at once.
pub fn utf8_offset(v: &[u8], n: usize) -> Result<usize, usize> {
    let (mut left, mut pos) = (n, 0);
    for block in v.chunks(64) {
        let k = block.iter().filter(|&&b| starts_char(b)).count();
        if k > left {
            for (j, &b) in block.iter().enumerate() {
                if starts_char(b) {
                    if left == 0 {
                        return Ok(pos + j);
                    }
                    left -= 1;
                }
            }
        }
        left -= k;
        pos += block.len();
    }
    Err(n - left)
}

/// Where the `k`th character from the end of `b` starts (`k` ≥ 1), or
/// `None` when `b` has fewer: `s[-k]` without counting the whole string.
pub fn back(b: &[u8], k: usize) -> Option<usize> {
    let mut end = b.len();
    for _ in 0..k {
        end -= last(&b[..end])?.byte_len();
    }
    Some(end)
}

/// [`back`] in valid UTF-8: steps back over continuation bytes.
pub fn utf8_back(v: &[u8], k: usize) -> Option<usize> {
    let mut end = v.len();
    for _ in 0..k {
        end = v[..end].iter().rposition(|&b| starts_char(b))?;
    }
    Some(end)
}

/// `b` as text for a message: each byte outside valid UTF-8 becomes
/// U+FFFD. Never for a value (L4).
pub fn lossy(b: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(b: &[u8]) -> Vec<Unit> {
        units(b).map(|(_, u)| u).collect()
    }

    #[test]
    fn an_invalid_byte_is_one_character() {
        use Unit::{Byte, Char};
        assert_eq!(
            all(b"a\xffb\xe2\x82c"),
            vec![
                Char('a'),
                Byte(0xFF),
                Char('b'),
                Byte(0xE2),
                Byte(0x82),
                Char('c')
            ]
        );
        assert_eq!(
            all("é€😀".as_bytes()),
            vec![Char('é'), Char('€'), Char('😀')]
        );
        // overlong, surrogate, past U+10FFFF, lone continuation
        assert_eq!(all(b"\xc0\x80"), vec![Byte(0xC0), Byte(0x80)]);
        assert_eq!(all(b"\xed\xa0\x80").len(), 3);
        assert_eq!(all(b"\xf4\x90\x80\x80").len(), 4);
        assert_eq!(all(b"\x80"), vec![Byte(0x80)]);
    }

    /// Read from the end, a string falls into the same characters as read
    /// from the start (VA5).
    /// Every string of up to four pieces, valid and broken UTF-8 mixed.
    fn samples() -> Vec<Vec<u8>> {
        let pieces: [&[u8]; 12] = [
            b"a",
            b"\xff",
            b"\xe2\x82",
            b"\xe2\x82\xac",
            "é".as_bytes(),
            "😀".as_bytes(),
            b"\x80",
            b"\xc0\x80",
            b"\xed\xa0\x80",
            b"\xf4\x90\x80\x80",
            b"\xf0\x9f",
            b" ",
        ];
        let mut all: Vec<Vec<u8>> = vec![Vec::new()];
        let mut last_round: Vec<Vec<u8>> = vec![Vec::new()];
        for _ in 0..4 {
            let mut next = Vec::new();
            for s in &last_round {
                for p in pieces {
                    let mut t = s.clone();
                    t.extend_from_slice(p);
                    next.push(t);
                }
            }
            all.extend(next.iter().cloned());
            last_round = next;
        }
        // Long enough to cross the counting blocks.
        all.push("가".repeat(100).into_bytes());
        all.push(
            [
                &b"a"[..],
                "가".repeat(70).as_bytes(),
                b"\xff",
                &b"b".repeat(130),
            ]
            .concat(),
        );
        // Characters across `byte_at`'s 4096-byte windows.
        let a = b"a".repeat(4094);
        all.push([&a[..], "가가".as_bytes(), b"\xe2\x82", b"b"].concat());
        all.push([&a[..], b"a", b"\xf0\x9f\x98"].concat());
        all.push([&a[..], b"\xe2\x82\xff", "😀".as_bytes()].concat());
        all
    }

    /// Where each character starts, found forward and from the end, is
    /// where cutting into characters puts it (VA12).
    #[test]
    fn offsets_agree_with_the_units() {
        for b in samples() {
            let starts: Vec<usize> = units(&b).map(|(at, _)| at).collect();
            let n = starts.len();
            for i in 0..=n + 1 {
                assert_eq!(
                    byte_at(&b, i),
                    starts.get(i).copied().unwrap_or(b.len()),
                    "{b:?} {i}"
                );
            }
            for k in 1..=n + 1 {
                assert_eq!(
                    back(&b, k),
                    n.checked_sub(k).map(|i| starts[i]),
                    "{b:?} -{k}"
                );
            }
            if let Ok(s) = std::str::from_utf8(&b) {
                for i in 0..=n + 1 {
                    assert_eq!(
                        utf8_offset(s.as_bytes(), i),
                        starts.get(i).copied().ok_or(n),
                        "{b:?} {i}"
                    );
                }
                for k in 1..=n + 1 {
                    assert_eq!(utf8_back(s.as_bytes(), k), back(&b, k), "{b:?} -{k}");
                }
            }
        }
    }

    #[test]
    fn the_last_character_agrees_with_the_units() {
        for b in &samples() {
            let want = units(b).last().map(|(_, u)| u);
            assert_eq!(last(b), want, "{b:?}");
        }
    }

    #[test]
    fn counting_agrees_with_the_units() {
        for b in [
            &b""[..],
            b"abc",
            b"a\xffb\xe2\x82c",
            "é€😀".as_bytes(),
            b"\xc0\x80\xed\xa0\x80\xf4\x90\x80\x80\xe2\x82",
            b"\xe2\x82\xac\xe2",
        ] {
            assert_eq!(char_count(b), units(b).count(), "{b:?}");
        }
    }

    #[test]
    fn offsets_of_characters() {
        let b = b"a\xff\xc3\xa9z";
        assert_eq!(
            (0..6).map(|i| byte_at(b, i)).collect::<Vec<_>>(),
            [0, 1, 2, 4, 5, 5]
        );
        assert_eq!(byte_at(b"abc", 7), 3);
    }
}
