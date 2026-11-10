//! The arguments a native receives (VM15). Up to `INLINE` of them sit in
//! the value itself, so a call with few arguments allocates nothing for
//! them; more go in a `Vec`. It reads and writes as a slice of its
//! values, and gives them up one by one from the end (`pop`) or all in
//! order (`into_iter`). The unused inline places hold `nil`.

use super::Value;

/// How many arguments a native receives without an allocation.
pub const INLINE: usize = 4;

/// The arguments. Its representation is private, so the inline count
/// stays at most `INLINE` and the places past it hold `nil`.
#[derive(Clone, Debug)]
pub struct Args(Repr);

#[derive(Clone, Debug)]
enum Repr {
    Inline(usize, [Value; INLINE]),
    Heap(Vec<Value>),
}

impl Args {
    /// No arguments.
    #[inline]
    pub fn new() -> Args {
        Args(Repr::Inline(0, [const { Value::Nil }; INLINE]))
    }

    /// Room for `n` arguments: inline when they fit.
    #[inline]
    pub fn with_capacity(n: usize) -> Args {
        if n <= INLINE {
            Args::new()
        } else {
            Args(Repr::Heap(Vec::with_capacity(n)))
        }
    }

    /// Adds an argument at the end.
    #[inline]
    pub fn push(&mut self, v: Value) {
        match &mut self.0 {
            Repr::Inline(n, a) if *n < INLINE => {
                a[*n] = v;
                *n += 1;
            }
            Repr::Inline(n, a) => {
                let mut h = Vec::with_capacity(*n + 1);
                h.extend(
                    a.iter_mut()
                        .take(*n)
                        .map(|x| std::mem::replace(x, Value::Nil)),
                );
                h.push(v);
                self.0 = Repr::Heap(h);
            }
            Repr::Heap(h) => h.push(v),
        }
    }

    /// Takes the last argument.
    #[inline]
    pub fn pop(&mut self) -> Option<Value> {
        match &mut self.0 {
            Repr::Inline(0, _) => None,
            Repr::Inline(n, a) => {
                *n -= 1;
                Some(std::mem::replace(&mut a[*n], Value::Nil))
            }
            Repr::Heap(h) => h.pop(),
        }
    }

    /// Takes argument `i`; the last one takes its place.
    #[inline]
    pub fn swap_remove(&mut self, i: usize) -> Value {
        let last = self.len() - 1;
        self.swap(i, last);
        self.pop().expect("an argument")
    }

    /// The arguments as a `Vec`.
    pub fn into_vec(self) -> Vec<Value> {
        match self.0 {
            Repr::Inline(n, a) => a.into_iter().take(n).collect(),
            Repr::Heap(h) => h,
        }
    }
}

impl Default for Args {
    fn default() -> Args {
        Args::new()
    }
}

impl std::ops::Deref for Args {
    type Target = [Value];

    #[inline]
    fn deref(&self) -> &[Value] {
        match &self.0 {
            Repr::Inline(n, a) => &a[..*n],
            Repr::Heap(h) => h,
        }
    }
}

impl std::ops::DerefMut for Args {
    #[inline]
    fn deref_mut(&mut self) -> &mut [Value] {
        match &mut self.0 {
            Repr::Inline(n, a) => &mut a[..*n],
            Repr::Heap(h) => h,
        }
    }
}

impl From<Vec<Value>> for Args {
    fn from(v: Vec<Value>) -> Args {
        if v.len() <= INLINE {
            v.into_iter().collect()
        } else {
            Args(Repr::Heap(v))
        }
    }
}

impl<const N: usize> From<[Value; N]> for Args {
    fn from(v: [Value; N]) -> Args {
        v.into_iter().collect()
    }
}

impl FromIterator<Value> for Args {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Value>>(it: I) -> Args {
        let it = it.into_iter();
        let mut a = Args::with_capacity(it.size_hint().0);
        for v in it {
            a.push(v);
        }
        a
    }
}

/// The arguments, moved out in order.
pub enum IntoIter {
    Inline(std::iter::Take<std::array::IntoIter<Value, INLINE>>),
    Heap(std::vec::IntoIter<Value>),
}

impl Iterator for IntoIter {
    type Item = Value;

    #[inline]
    fn next(&mut self) -> Option<Value> {
        match self {
            IntoIter::Inline(i) => i.next(),
            IntoIter::Heap(i) => i.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            IntoIter::Inline(i) => i.size_hint(),
            IntoIter::Heap(i) => i.size_hint(),
        }
    }
}

impl ExactSizeIterator for IntoIter {}

impl DoubleEndedIterator for IntoIter {
    fn next_back(&mut self) -> Option<Value> {
        match self {
            IntoIter::Inline(i) => i.next_back(),
            IntoIter::Heap(i) => i.next_back(),
        }
    }
}

impl IntoIterator for Args {
    type Item = Value;
    type IntoIter = IntoIter;

    #[inline]
    fn into_iter(self) -> IntoIter {
        match self.0 {
            Repr::Inline(n, a) => IntoIter::Inline(a.into_iter().take(n)),
            Repr::Heap(h) => IntoIter::Heap(h.into_iter()),
        }
    }
}

impl<'a> IntoIterator for &'a Args {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::{Args, INLINE, Repr, Value};

    fn int(x: Value) -> i64 {
        match x {
            Value::Int(i) => i,
            other => panic!("not an int: {other:?}"),
        }
    }

    fn ints(a: Args) -> Vec<i64> {
        a.into_iter().map(int).collect()
    }

    fn inline(a: &Args) -> bool {
        matches!(a.0, Repr::Inline(..))
    }

    /// A few arguments are inline (no allocation), more on the heap; every
    /// length reads back as the slice it was made of, pops from the end,
    /// moves out in order and turns back into the same `Vec`.
    #[test]
    fn args_of_every_length() {
        for n in 0..(INLINE as i64 + 3) {
            let want: Vec<i64> = (0..n).collect();
            let v: Vec<Value> = want.iter().map(|&i| Value::Int(i)).collect();
            let made = [
                Args::from(v.clone()),
                v.iter().cloned().collect::<Args>(),
                {
                    let mut a = Args::new();
                    for x in v.iter().cloned() {
                        a.push(x);
                    }
                    a
                },
            ];
            for a in made {
                assert_eq!(inline(&a), n as usize <= INLINE, "length {n}");
                assert_eq!(a.len(), n as usize);
                assert_eq!(ints(a.clone()), want);
                let back: Vec<i64> = a.clone().into_vec().into_iter().map(int).collect();
                assert_eq!(back, want, "into_vec of length {n}");
                let mut a = a;
                let mut popped = Vec::new();
                while let Some(x) = a.pop() {
                    popped.push(int(x));
                }
                assert_eq!(popped, want.iter().rev().copied().collect::<Vec<_>>());
                assert!(a.is_empty());
            }
        }
    }

    /// `swap_remove` takes an argument and puts the last in its place,
    /// inline and spilled alike.
    #[test]
    fn swap_remove_inline_and_spilled() {
        for n in [3i64, INLINE as i64 + 2] {
            let mut a: Args = (0..n).map(Value::Int).collect();
            assert_eq!(int(a.swap_remove(0)), 0);
            let mut want: Vec<i64> = (1..n).collect();
            let last = want.pop().expect("a last");
            want.insert(0, last);
            assert_eq!(ints(a.clone()), want, "length {n}");
            assert_eq!(int(a.swap_remove(a.len() - 1)), *want.last().expect("one"));
            assert_eq!(a.len(), n as usize - 2);
        }
    }

    /// An array of a few values is inline, of more spills, and keeps its
    /// order either way.
    #[test]
    fn from_an_array() {
        let a = Args::from([Value::Int(1), Value::Int(2)]);
        assert!(inline(&a));
        assert_eq!(ints(a), [1, 2]);
        let b = Args::from([0, 1, 2, 3, 4, 5].map(Value::Int));
        assert!(!inline(&b));
        assert_eq!(
            b.into_vec().into_iter().map(int).collect::<Vec<_>>(),
            [0, 1, 2, 3, 4, 5]
        );
        assert!(Args::from([] as [Value; 0]).is_empty());
    }
}
