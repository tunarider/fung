//! The values a tag, a tuple or a closure holds (VM8). One or two sit in
//! the value itself, so `Ok(x)`, a pair or a closure that copied one name
//! is one allocation (its `Rc`) instead of two (the `Rc` and a `Vec`'s
//! buffer); more go in a box. It reads as a slice of its values, and a
//! `Vec` turns into one with `into()`.

use super::Value;

#[derive(Clone, Debug)]
pub enum Items {
    One([Value; 1]),
    Two([Value; 2]),
    Many(Box<[Value]>),
}

impl Default for Items {
    /// No values (a box of none allocates nothing).
    fn default() -> Items {
        Items::Many(Box::default())
    }
}

impl std::ops::Deref for Items {
    type Target = [Value];

    #[inline]
    fn deref(&self) -> &[Value] {
        match self {
            Items::One(a) => a,
            Items::Two(a) => a,
            Items::Many(b) => b,
        }
    }
}

impl From<Vec<Value>> for Items {
    fn from(mut v: Vec<Value>) -> Items {
        match v.len() {
            1 => Items::One([v.pop().expect("one value")]),
            2 => {
                let b = v.pop().expect("two values");
                let a = v.pop().expect("two values");
                Items::Two([a, b])
            }
            _ => Items::Many(v.into_boxed_slice()),
        }
    }
}

impl FromIterator<Value> for Items {
    fn from_iter<I: IntoIterator<Item = Value>>(it: I) -> Items {
        let mut it = it.into_iter();
        let Some(a) = it.next() else {
            return Items::default();
        };
        let Some(b) = it.next() else {
            return Items::One([a]);
        };
        let Some(c) = it.next() else {
            return Items::Two([a, b]);
        };
        let mut v = Vec::with_capacity(3 + it.size_hint().0);
        v.extend([a, b, c]);
        v.extend(it);
        Items::Many(v.into_boxed_slice())
    }
}

/// The values, moved out (`release`).
pub enum IntoIter {
    One(std::array::IntoIter<Value, 1>),
    Two(std::array::IntoIter<Value, 2>),
    Many(std::vec::IntoIter<Value>),
}

impl Iterator for IntoIter {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        match self {
            IntoIter::One(i) => i.next(),
            IntoIter::Two(i) => i.next(),
            IntoIter::Many(i) => i.next(),
        }
    }
}

impl IntoIterator for Items {
    type Item = Value;
    type IntoIter = IntoIter;

    fn into_iter(self) -> IntoIter {
        match self {
            Items::One(a) => IntoIter::One(a.into_iter()),
            Items::Two(a) => IntoIter::Two(a.into_iter()),
            Items::Many(b) => IntoIter::Many(b.into_vec().into_iter()),
        }
    }
}

impl<'a> IntoIterator for &'a Items {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::{Items, Value};

    /// Every length reads back as the slice it was made of, and moves out
    /// in order.
    #[test]
    fn items_of_every_length() {
        for n in 0..5 {
            let v: Vec<Value> = (0..n).map(Value::Int).collect();
            let a: Items = v.clone().into();
            let b: Items = v.clone().into_iter().collect();
            let shape = |i: &Items| match i {
                Items::One(_) => 1,
                Items::Two(_) => 2,
                Items::Many(_) => 0,
            };
            assert_eq!(shape(&a), shape(&b), "{n}");
            assert_eq!(shape(&a), if n == 1 || n == 2 { n } else { 0 }, "{n}");
            for items in [a, b] {
                assert_eq!(items.len(), n as usize);
                let back: Vec<i64> = items
                    .into_iter()
                    .map(|x| match x {
                        Value::Int(i) => i,
                        _ => unreachable!(),
                    })
                    .collect();
                assert_eq!(back, (0..n).collect::<Vec<_>>());
            }
        }
    }
}
