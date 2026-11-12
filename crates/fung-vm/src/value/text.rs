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
    /// The last character found by its number, and its byte offset: a
    /// loop of `s[i]` over a non-ASCII string reads on from there instead
    /// of from the start each time (VA35).
    cursor: Cell<(usize, usize)>,
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
                cursor: Cell::new((0, 0)),
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

    /// Valid UTF-8 whose being ASCII is known: no pass over it (VA38).
    pub fn ascii(s: String, ascii: bool) -> Text {
        debug_assert_eq!(s.is_ascii(), ascii);
        Text {
            repr: Repr::Utf8(s),
            ascii,
            chars: Cell::new(UNCOUNTED),
            cursor: Cell::new((0, 0)),
        }
    }

    /// Valid UTF-8 (a `String`): one pass to learn whether it is ASCII,
    /// none to check it (VA38). Bytes that may not be UTF-8 go through
    /// [`Text::new`].
    pub fn utf8(s: String) -> Text {
        Text::from(s)
    }

    /// `x` followed by `y`. Two valid UTF-8 texts are joined as text, with
    /// no check of the result, and their counted characters add up (VA26,
    /// VA32); otherwise the bytes are looked at again (bytes outside UTF-8
    /// may join into characters at the seam).
    pub fn concat(x: &Text, y: &Text) -> Text {
        if let (Repr::Utf8(a), Repr::Utf8(b)) = (&x.repr, &y.repr) {
            let mut s = String::with_capacity(a.len() + b.len());
            s.push_str(a);
            s.push_str(b);
            let t = Text::ascii(s, x.ascii && y.ascii);
            if let (Some(m), Some(n)) = (x.counted(), y.counted()) {
                t.chars.set(m + n);
            }
            return t;
        }
        let mut b = Vec::with_capacity(x.as_bytes().len() + y.as_bytes().len());
        b.extend_from_slice(x.as_bytes());
        b.extend_from_slice(y.as_bytes());
        Text::new(b)
    }

    /// Puts `y` after this text in place, when both are valid UTF-8 (a
    /// string held by one value only grows, `s = s + x`, VA32); false
    /// otherwise, and nothing changed.
    pub fn append(&mut self, y: &Text) -> bool {
        if !(self.is_utf8() && y.is_utf8()) {
            return false;
        }
        let counted = match (self.counted(), y.counted()) {
            (Some(m), Some(n)) => m + n,
            _ => UNCOUNTED,
        };
        let (Repr::Utf8(a), Repr::Utf8(b)) = (&mut self.repr, &y.repr) else {
            unreachable!("both checked to be UTF-8")
        };
        a.push_str(b);
        self.ascii = self.ascii && y.ascii;
        self.chars.set(counted);
        true
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

    /// The byte offset of character `n`, or the length past the end: from
    /// the character looked up last when `n` is not before it (VA35).
    pub fn offset_of(&self, n: usize) -> usize {
        if self.ascii {
            return n.min(self.as_bytes().len());
        }
        let (k, b) = self.cursor.get();
        let at = if n >= k {
            self.byte_at(b, n - k)
        } else {
            self.byte_at(0, n)
        };
        if at < self.as_bytes().len() {
            self.cursor.set((n, at));
        }
        at
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
            cursor: Cell::new((0, 0)),
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
