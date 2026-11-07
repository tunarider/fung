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
pub fn byte_at(b: &[u8], i: usize) -> usize {
    if b.is_ascii() {
        return i.min(b.len());
    }
    units(b).nth(i).map_or(b.len(), |(at, _)| at)
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
