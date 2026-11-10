//! A string value (02, L4): bytes, read as UTF-8 by the character
//! operations (`fung_front::text`). Whether the bytes are ASCII or valid
//! UTF-8 is known from when the string is made, so the checks of the
//! operations that need text cost nothing more. The number of characters
//! is counted the first time `len` asks; `s[i]` reads only up to `i`.

use std::borrow::Borrow;
use std::cell::Cell;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

use fung_front::text;

/// Valid UTF-8 is kept as a `String`, so the text is at hand without
/// another check and `str`'s own fast paths serve it; other bytes as
/// they are.
#[derive(Clone)]
enum Repr {
    Utf8(String),
    Bytes(Vec<u8>),
}

#[derive(Clone)]
pub struct Text {
    repr: Repr,
    ascii: bool,
    /// The number of characters once counted (`UNCOUNTED` before): a
    /// string is never changed, so `len` counts it once (VA12).
    chars: Cell<usize>,
}

const UNCOUNTED: usize = usize::MAX;

impl Text {
    pub fn new(bytes: Vec<u8>) -> Text {
        match String::from_utf8(bytes) {
            Ok(s) => Text::from(s),
            Err(e) => Text {
                repr: Repr::Bytes(e.into_bytes()),
                ascii: false,
                chars: Cell::new(UNCOUNTED),
            },
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match &self.repr {
            Repr::Utf8(s) => s.as_bytes(),
            Repr::Bytes(b) => b,
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        match self.repr {
            Repr::Utf8(s) => s.into_bytes(),
            Repr::Bytes(b) => b,
        }
    }

    /// The text, when it is valid UTF-8.
    pub fn as_str(&self) -> Option<&str> {
        match &self.repr {
            Repr::Utf8(s) => Some(s),
            Repr::Bytes(_) => None,
        }
    }

    pub fn is_ascii(&self) -> bool {
        self.ascii
    }

    pub fn is_utf8(&self) -> bool {
        matches!(self.repr, Repr::Utf8(_))
    }

    /// The number of characters when it is known without counting.
    pub fn counted(&self) -> Option<usize> {
        if self.ascii {
            Some(self.as_bytes().len())
        } else {
            Some(self.chars.get()).filter(|&n| n != UNCOUNTED)
        }
    }

    /// `x` followed by `y`. Two valid UTF-8 texts whose characters are
    /// both counted already make a text whose count is the sum (VA26);
    /// otherwise the count waits to be asked (bytes outside UTF-8 may
    /// join into characters at the seam).
    pub fn concat(x: &Text, y: &Text) -> Text {
        let mut b = Vec::with_capacity(x.as_bytes().len() + y.as_bytes().len());
        b.extend_from_slice(x.as_bytes());
        b.extend_from_slice(y.as_bytes());
        let t = Text::new(b);
        if let (true, true, Some(m), Some(n)) = (x.is_utf8(), y.is_utf8(), x.counted(), y.counted())
        {
            t.chars.set(m + n);
        }
        t
    }

    /// The number of characters, counted the first time it is asked.
    pub fn char_len(&self) -> usize {
        if self.ascii {
            return self.as_bytes().len();
        }
        if self.chars.get() == UNCOUNTED {
            self.chars.set(match &self.repr {
                Repr::Utf8(s) => s.chars().count(),
                Repr::Bytes(b) => text::char_count(b),
            });
        }
        self.chars.get()
    }

    /// The byte offset of the `n`th character from byte `from` (a
    /// character boundary), or the length past the end. Only the part
    /// up to it is read.
    pub fn byte_at(&self, from: usize, n: usize) -> usize {
        match &self.repr {
            _ if self.ascii => from.saturating_add(n).min(self.as_bytes().len()),
            Repr::Utf8(s) => {
                from + text::utf8_offset(&s.as_bytes()[from..], n).unwrap_or(s.len() - from)
            }
            Repr::Bytes(b) => from + text::byte_at(&b[from..], n),
        }
    }

    /// Where the `k`th character from the end starts (`k` ≥ 1), or
    /// `None` when there are fewer: only the part after it is read.
    pub fn byte_back(&self, k: usize) -> Option<usize> {
        match &self.repr {
            _ if self.ascii => self.as_bytes().len().checked_sub(k),
            Repr::Utf8(s) => text::utf8_back(s.as_bytes(), k),
            Repr::Bytes(b) => text::back(b, k),
        }
    }

    /// The text for a message; bytes outside UTF-8 become U+FFFD.
    pub fn lossy(&self) -> std::borrow::Cow<'_, str> {
        text::lossy(self.as_bytes())
    }
}

impl From<String> for Text {
    fn from(s: String) -> Text {
        Text {
            ascii: s.is_ascii(),
            repr: Repr::Utf8(s),
            chars: Cell::new(UNCOUNTED),
        }
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Text {
        Text::from(s.to_string())
    }
}

impl From<Vec<u8>> for Text {
    fn from(b: Vec<u8>) -> Text {
        Text::new(b)
    }
}

impl Deref for Text {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsRef<[u8]> for Text {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Borrow<[u8]> for Text {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl PartialEq for Text {
    fn eq(&self, other: &Text) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for Text {}

impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl PartialOrd for Text {
    fn partial_cmp(&self, other: &Text) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Byte order, which is code point order for valid UTF-8.
impl Ord for Text {
    fn cmp(&self, other: &Text) -> Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl Hash for Text {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_bytes().hash(h)
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&fung_front::consts::quote_bytes(self.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_string_knows_what_its_bytes_are() {
        let ascii = Text::from("abc");
        let utf8 = Text::from("é");
        let bytes = Text::new(b"a\xff".to_vec());
        assert!(ascii.is_ascii() && ascii.is_utf8());
        assert!(!utf8.is_ascii() && utf8.is_utf8());
        assert!(!bytes.is_utf8());
        assert_eq!(utf8.as_str(), Some("é"));
        assert_eq!(bytes.as_str(), None);
        assert_eq!(bytes.char_len(), 2);
        assert_eq!(Text::new("é".as_bytes().to_vec()), utf8);
        assert_eq!(format!("{bytes:?}"), r#""a\xff""#);
    }
}
