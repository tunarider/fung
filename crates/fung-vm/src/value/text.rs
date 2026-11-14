//! A string value (02, L4): bytes, read as UTF-8 by the character
//! operations (`fung_front::text`). Whether the bytes are valid UTF-8 is
//! known from when the string is made, so the checks of the operations
//! that need text cost nothing more. The number of characters is counted
//! the first time `len` asks; `s[i]` reads only up to `i`.
//!
//! A slice of valid UTF-8 is a view of the bytes of the string it was cut
//! from, made in O(1) (VA69): the slices of a loop that eats a string from
//! either end copy nothing. A piece much smaller than the bytes it would
//! keep alive is copied instead (`Text::slice`), so a small piece of a
//! big text lets the big text go.

use std::borrow::Borrow;
use std::cell::{Cell, OnceCell};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;

use fung_front::text;

/// Valid UTF-8 is kept as a `String`, so the text is at hand without
/// another check and `str`'s own fast paths serve it; other bytes as
/// they are.
#[derive(Clone)]
enum Repr {
    Utf8(String),
    Bytes(Vec<u8>),
    /// Bytes `lo..hi` of `root`, at character boundaries. `root` is valid
    /// UTF-8 and never a view itself (VA69).
    View {
        root: Rc<Text>,
        lo: usize,
        hi: usize,
    },
}

/// What is known of whether a string is ASCII: a view of a non-ASCII text
/// may be ASCII, and is not looked at to say so until a caller needs to
/// know (`Text::is_ascii`).
const ASCII: u8 = 0;
const NOT_ASCII: u8 = 1;
const ASCII_UNKNOWN: u8 = 2;

#[derive(Clone)]
pub struct Text {
    repr: Repr,
    ascii: Cell<u8>,
    /// The number of characters once counted (`UNCOUNTED` before): a
    /// string is never changed, so `len` counts it once (VA12).
    chars: Cell<usize>,
    /// Where characters were found, kept once a lookup is not near the
    /// start (`Seek`): boxed, so a string that is never looked into that
    /// way, ASCII or short, carries one word for it.
    seek: OnceCell<Box<Seek>>,
}

/// What a non-ASCII string remembers of its lookups by character number.
#[derive(Clone, Default)]
struct Seek {
    /// The last character found by its number, and its byte offset: a
    /// loop of `s[i]` reads on from there instead of from the start each
    /// time (VA35).
    cursor: Cell<(usize, usize)>,
    /// The byte offset of every `STRIDE`th character, made the first time
    /// a character is looked up far behind the cursor (VA68): any lookup
    /// after that reads fewer than `STRIDE` characters.
    marks: OnceCell<Box<[usize]>>,
}

const UNCOUNTED: usize = usize::MAX;

/// Characters between two marks; also how far a lookup walks from the
/// cursor, the start or the end before the marks are worth having.
const STRIDE: usize = 64;

/// A piece of at most this many bytes is copied, never a view: it costs
/// no more than the view and holds nothing else alive.
const SMALL_PIECE: usize = 64;

/// A piece is a view only while it is at least this fraction (1/KEEP) of
/// the bytes it would keep alive; a smaller one is copied. Eating a text
/// from one end then copies a quarter, a sixteenth … of it: O(n) in all.
const KEEP: usize = 4;

impl Text {
    fn with(repr: Repr, ascii: u8) -> Text {
        Text {
            repr,
            ascii: Cell::new(ascii),
            chars: Cell::new(UNCOUNTED),
            seek: OnceCell::new(),
        }
    }

    pub fn new(bytes: Vec<u8>) -> Text {
        match String::from_utf8(bytes) {
            Ok(s) => Text::from(s),
            Err(e) => Text::with(Repr::Bytes(e.into_bytes()), NOT_ASCII),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match &self.repr {
            Repr::Utf8(s) => s.as_bytes(),
            Repr::Bytes(b) => b,
            Repr::View { root, lo, hi } => &root.as_bytes()[*lo..*hi],
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        match self.repr {
            Repr::Utf8(s) => s.into_bytes(),
            Repr::Bytes(b) => b,
            Repr::View { root, lo, hi } => root.as_bytes()[lo..hi].to_vec(),
        }
    }

    /// The text, when it is valid UTF-8.
    pub fn as_str(&self) -> Option<&str> {
        match &self.repr {
            Repr::Utf8(s) => Some(s),
            Repr::Bytes(_) => None,
            Repr::View { root, lo, hi } => root.as_str().map(|s| &s[*lo..*hi]),
        }
    }

    /// Whether every byte is ASCII. A view not yet known to be is looked
    /// at once.
    pub fn is_ascii(&self) -> bool {
        match self.ascii.get() {
            ASCII => true,
            NOT_ASCII => false,
            _ => {
                let a = self.as_bytes().is_ascii();
                self.ascii.set(if a { ASCII } else { NOT_ASCII });
                a
            }
        }
    }

    /// Whether the string is known to be ASCII without looking at it: the
    /// fast paths of the lookups, which are only slower when it is not
    /// known.
    fn known_ascii(&self) -> bool {
        self.ascii.get() == ASCII
    }

    pub fn is_utf8(&self) -> bool {
        !matches!(self.repr, Repr::Bytes(_))
    }

    /// The number of characters when it is known without counting.
    pub fn counted(&self) -> Option<usize> {
        if self.known_ascii() {
            Some(self.as_bytes().len())
        } else {
            Some(self.chars.get()).filter(|&n| n != UNCOUNTED)
        }
    }

    /// Valid UTF-8 whose being ASCII is known: no pass over it (VA38).
    pub fn ascii(s: String, ascii: bool) -> Text {
        debug_assert_eq!(s.is_ascii(), ascii);
        Text::with(Repr::Utf8(s), if ascii { ASCII } else { NOT_ASCII })
    }

    /// Valid UTF-8 (a `String`): one pass to learn whether it is ASCII,
    /// none to check it (VA38). Bytes that may not be UTF-8 go through
    /// [`Text::new`].
    pub fn utf8(s: String) -> Text {
        Text::from(s)
    }

    /// Bytes `from..to` of `this` (character boundaries), which hold
    /// `chars` characters. A piece of valid UTF-8 is a view of the bytes
    /// `this` shares, unless it is small or a small part of them: then it
    /// is copied (VA69). Other bytes are copied and looked at again (a
    /// piece of them may be valid UTF-8).
    pub fn slice(this: &Rc<Text>, from: usize, to: usize, chars: usize) -> Rc<Text> {
        let (root, base) = match &this.repr {
            Repr::Bytes(b) => return Rc::new(Text::new(b[from..to].to_vec())),
            Repr::Utf8(_) => (this, 0),
            Repr::View { root, lo, .. } => (root, *lo),
        };
        let (lo, hi) = (base + from, base + to);
        let whole = root.as_bytes().len();
        let piece = if to - from <= SMALL_PIECE || (to - from) * KEEP < whole {
            let s = root.as_str().expect("a root is UTF-8")[lo..hi].to_string();
            // Copying looks at every byte anyway.
            let ascii = this.known_ascii() || s.is_ascii();
            Text::ascii(s, ascii)
        } else {
            let ascii = if this.known_ascii() {
                ASCII
            } else {
                ASCII_UNKNOWN
            };
            Text::with(
                Repr::View {
                    root: root.clone(),
                    lo,
                    hi,
                },
                ascii,
            )
        };
        piece.chars.set(chars);
        Rc::new(piece)
    }

    /// `x` followed by `y`. Two valid UTF-8 texts are joined as text, with
    /// no check of the result, and their counted characters add up (VA26,
    /// VA32); otherwise the bytes are looked at again (bytes outside UTF-8
    /// may join into characters at the seam).
    pub fn concat(x: &Text, y: &Text) -> Text {
        if let (Some(a), Some(b)) = (x.as_str(), y.as_str()) {
            let mut s = String::with_capacity(a.len() + b.len());
            s.push_str(a);
            s.push_str(b);
            let t = Text::ascii(s, x.is_ascii() && y.is_ascii());
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
    /// otherwise, and nothing changed. A view becomes a string of its own
    /// first.
    pub fn append(&mut self, y: &Text) -> bool {
        let Some(b) = y.as_str() else {
            return false;
        };
        if let Repr::View { .. } = self.repr {
            let own = self.as_str().expect("a view is UTF-8").to_string();
            self.repr = Repr::Utf8(own);
        }
        let counted = match (self.counted(), y.counted()) {
            (Some(m), Some(n)) => m + n,
            _ => UNCOUNTED,
        };
        let Repr::Utf8(a) = &mut self.repr else {
            return false;
        };
        a.push_str(b);
        let ascii = match (self.ascii.get(), y.is_ascii()) {
            (_, false) | (NOT_ASCII, _) => NOT_ASCII,
            (state, true) => state,
        };
        self.ascii.set(ascii);
        self.chars.set(counted);
        // The marks were made for the shorter text; the cursor stays good.
        if let Some(seek) = self.seek.get_mut() {
            seek.marks.take();
        }
        true
    }

    /// The number of characters, counted the first time it is asked.
    pub fn char_len(&self) -> usize {
        if let Some(n) = self.counted() {
            return n;
        }
        let n = match self.as_str() {
            Some(s) => {
                let n = s.chars().count();
                // Valid UTF-8 has as many characters as bytes only when
                // it is ASCII.
                self.ascii.set(if n == s.len() { ASCII } else { NOT_ASCII });
                n
            }
            None => text::char_count(self.as_bytes()),
        };
        self.chars.set(n);
        n
    }

    /// The byte offset of the `n`th character from byte `from` (a
    /// character boundary), or the length past the end. Only the part
    /// up to it is read.
    pub fn byte_at(&self, from: usize, n: usize) -> usize {
        let b = self.as_bytes();
        if self.known_ascii() {
            from.saturating_add(n).min(b.len())
        } else if self.is_utf8() {
            from + text::utf8_offset(&b[from..], n).unwrap_or(b.len() - from)
        } else {
            from + text::byte_at(&b[from..], n)
        }
    }

    /// The byte offset of character `n`, or the length past the end. It
    /// reads on from the character looked up last (VA35), back from it or
    /// from the end when that is near, and from the marks once a lookup
    /// went far back (VA68): a loop of `s[i]` in any order reads a few
    /// characters each.
    pub fn offset_of(&self, n: usize) -> usize {
        let len = self.as_bytes().len();
        if self.known_ascii() {
            return n.min(len);
        }
        let total = self.counted();
        if total.is_some_and(|t| n >= t) {
            return len;
        }
        // Near the start of a string not looked into yet: from the start,
        // remembering nothing (a slice looks up its first characters).
        match self.seek.get() {
            Some(seek) => self.seek_to(seek, n, total),
            None if n <= STRIDE => self.byte_at(0, n),
            None => self.seek_to(self.seek.get_or_init(Box::default), n, total),
        }
    }

    /// `offset_of` with what the string remembers.
    fn seek_to(&self, seek: &Seek, n: usize, total: Option<usize>) -> usize {
        let len = self.as_bytes().len();
        let (k, b) = seek.cursor.get();
        // the common case first: reading on a little from the cursor
        let walk = if n >= k && n - k <= STRIDE {
            Walk::Ahead(k, b)
        } else {
            self.far(seek, n, k, b, total)
        };
        let at = match walk {
            Walk::Ahead(from_char, from) => self.byte_at(from, n - from_char),
            Walk::Back(from, m) => back(self.as_bytes(), from, m),
        };
        if at < len {
            seek.cursor.set((n, at));
        }
        at
    }

    /// How `offset_of` reaches character `n` (before `total`, if counted)
    /// that is not just ahead of the cursor, character `k` at byte `b`.
    fn far(&self, seek: &Seek, n: usize, k: usize, b: usize, total: Option<usize>) -> Walk {
        let len = self.as_bytes().len();
        let utf8 = self.is_utf8();
        // (characters to read, how)
        let mut best = (n, Walk::Ahead(0, 0));
        if n >= k && n - k < best.0 {
            best = (n - k, Walk::Ahead(k, b));
        }
        if utf8 && n < k && k - n < best.0 {
            best = (k - n, Walk::Back(b, k - n));
        }
        if let (true, Some(t)) = (utf8, total)
            && t - n < best.0
        {
            best = (t - n, Walk::Back(len, t - n));
        }
        if best.0 <= STRIDE {
            best.1
        } else if let Some(marks) = seek.marks.get() {
            Self::at_mark(marks, n)
        } else if n >= k {
            // Reading on never makes the marks: one pass, as before.
            best.1
        } else {
            let marks = seek.marks.get_or_init(|| self.make_marks());
            Self::at_mark(marks, n)
        }
    }

    fn at_mark(marks: &[usize], n: usize) -> Walk {
        let j = (n / STRIDE).min(marks.len() - 1);
        Walk::Ahead(j * STRIDE, marks[j])
    }

    /// The byte offset of characters 0, `STRIDE`, 2 × `STRIDE` … in one
    /// pass.
    fn make_marks(&self) -> Box<[usize]> {
        let len = self.as_bytes().len();
        let mut marks = Vec::with_capacity(len / STRIDE / 2 + 1);
        let mut at = 0;
        loop {
            marks.push(at);
            at = self.byte_at(at, STRIDE);
            if at >= len {
                return marks.into_boxed_slice();
            }
        }
    }

    /// The text for a message; bytes outside UTF-8 become U+FFFD.
    pub fn lossy(&self) -> std::borrow::Cow<'_, str> {
        text::lossy(self.as_bytes())
    }

    /// Whether `self` keeps the bytes of `other` alive (a view of them).
    #[cfg(test)]
    pub(crate) fn views(&self, other: &Rc<Text>) -> bool {
        matches!(&self.repr, Repr::View { root, .. } if Rc::ptr_eq(root, other))
    }
}

/// How `Text::offset_of` reaches a character: reading on from a character
/// number at a byte offset, or back over a number of characters from a
/// byte offset (valid UTF-8 only).
#[derive(Clone, Copy)]
enum Walk {
    Ahead(usize, usize),
    Back(usize, usize),
}

/// The offset `m` characters before byte `from` of valid UTF-8 `b`.
fn back(b: &[u8], mut from: usize, m: usize) -> usize {
    for _ in 0..m {
        from -= 1;
        while b[from] & 0xC0 == 0x80 {
            from -= 1;
        }
    }
    from
}

impl From<String> for Text {
    fn from(s: String) -> Text {
        let ascii = if s.is_ascii() { ASCII } else { NOT_ASCII };
        Text::with(Repr::Utf8(s), ascii)
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

    /// A string carries one word for its lookups until it is looked into
    /// past its first characters: most strings are never.
    #[test]
    fn a_string_stays_small() {
        assert!(
            std::mem::size_of::<Text>() <= 56,
            "{}",
            std::mem::size_of::<Text>()
        );
    }

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

    /// The character offsets of `t`, walked from the start.
    fn offsets(t: &Text) -> Vec<usize> {
        let mut at = vec![0];
        let mut b = t.as_bytes();
        while let Some(u) = text::first(b) {
            b = &b[u.byte_len()..];
            at.push(t.len() - b.len());
        }
        at
    }

    /// `Text::slice` of `t`'s characters `lo..hi`, as `x[lo..hi]` cuts it.
    fn cut(t: &Rc<Text>, lo: usize, hi: usize) -> Rc<Text> {
        let (from, to) = (t.offset_of(lo), t.offset_of(hi));
        Text::slice(t, from, to, hi - lo)
    }

    #[test]
    fn a_big_piece_is_a_view_and_a_small_one_a_copy() {
        let big = Rc::new(Text::from("\u{d55c}a".repeat(1000)));
        let n = big.char_len();
        // three quarters, and a view of a view: both of the one buffer
        let half = cut(&big, 500, n);
        assert!(half.views(&big));
        assert_eq!(half.as_str(), Some(&"\u{d55c}a".repeat(750)[..]));
        assert_eq!(half.char_len(), 1500);
        let quarter = cut(&half, 500, 1500);
        assert!(quarter.views(&big));
        assert_eq!(quarter.as_bytes(), &big.as_bytes()[2000..]);
        // under a quarter of the buffer, or a few bytes: copies
        let tenth = cut(&quarter, 800, 1000);
        assert!(!tenth.views(&big) && !tenth.views(&quarter));
        assert_eq!(tenth.as_bytes(), &big.as_bytes()[3600..]);
        let few = cut(&half, 0, 4);
        assert!(!few.views(&big));
        assert_eq!(few.as_str(), Some("\u{d55c}a\u{d55c}a"));
        // an ASCII piece of a non-ASCII text is found ASCII when asked
        let mixed = Rc::new(Text::from(format!("\u{d55c}{}", "a".repeat(400))));
        let tail = cut(&mixed, 1, 401);
        assert!(tail.views(&mixed));
        assert!(tail.is_ascii());
        // bytes outside UTF-8 are copied and looked at again
        let bytes = Rc::new(Text::new([b"\xff".as_slice(), &[b'a'; 200]].concat()));
        let valid = cut(&bytes, 1, 201);
        assert!(valid.is_utf8() && valid.is_ascii() && !valid.views(&bytes));
    }

    #[test]
    fn a_view_grows_as_a_string_of_its_own() {
        let big = Rc::new(Text::from("\u{d55c}".repeat(300)));
        let mut piece = Rc::unwrap_or_clone(cut(&big, 10, 300));
        assert!(piece.append(&Text::from("b")));
        assert!(!piece.views(&big));
        assert_eq!(piece.char_len(), 291);
        assert_eq!(
            piece.as_str(),
            Some(&format!("{}b", "\u{d55c}".repeat(290))[..])
        );
        assert_eq!(big.char_len(), 300);
    }

    #[test]
    fn lookups_in_any_order_agree_with_a_walk_from_the_start() {
        let mixed: String = (0..700)
            .map(|i| ["a", "\u{e9}", "\u{d55c}", "\u{1f600}"][(i + i / 3) % 4])
            .collect();
        let big = Rc::new(Text::from(mixed.clone()));
        let view = cut(&big, 100, 700);
        let bytes = Text::new([mixed.as_bytes(), b"\xff\xfe", mixed.as_bytes()].concat());
        for t in [&*big, &*view, &bytes] {
            let at = offsets(t);
            let n = at.len() - 1;
            let mut order: Vec<usize> = (0..=n + 2).collect();
            order.extend((0..=n).rev());
            order.extend((0..n).map(|i| i * 389 % n));
            order.extend((0..n).map(|i| if i % 2 == 0 { i / 2 } else { n - 1 - i / 2 }));
            for i in order {
                assert_eq!(t.offset_of(i), at[i.min(n)], "character {i} of {n}");
            }
            // the far jumps back made the marks
            assert!(t.seek.get().is_some_and(|s| s.marks.get().is_some()));
        }
    }
}
