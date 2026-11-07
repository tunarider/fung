//! A string value (02, L4): bytes, read as UTF-8 by the character
//! operations (`fung_front::text`). Whether the bytes are ASCII or valid
//! UTF-8 is known from when the string is made, so `len`, `s[i]` and the
//! checks of the operations that need text cost nothing more.

use std::borrow::Borrow;
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
}

impl Text {
    pub fn new(bytes: Vec<u8>) -> Text {
        match String::from_utf8(bytes) {
            Ok(s) => Text::from(s),
            Err(e) => Text {
                repr: Repr::Bytes(e.into_bytes()),
                ascii: false,
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

    /// The number of characters.
    pub fn char_len(&self) -> usize {
        match &self.repr {
            _ if self.ascii => self.as_bytes().len(),
            Repr::Utf8(s) => s.chars().count(),
            Repr::Bytes(b) => text::char_count(b),
        }
    }

    /// The byte offset of the `n`th character from byte `from` (a
    /// character boundary), or the length past the end.
    pub fn byte_at(&self, from: usize, n: usize) -> usize {
        match &self.repr {
            _ if self.ascii => (from + n).min(self.as_bytes().len()),
            Repr::Utf8(s) => s[from..]
                .char_indices()
                .nth(n)
                .map_or(s.len(), |(b, _)| from + b),
            Repr::Bytes(b) => from + text::byte_at(&b[from..], n),
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
