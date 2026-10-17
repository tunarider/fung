//! Helpers shared by the value tests.

/// A small deterministic PRNG (xorshift64*) for the model tests.
pub(super) struct Rng(u64);

impl Rng {
    pub(super) fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub(super) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub(super) fn int_n(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub(super) fn int(&mut self) -> i64 {
        (self.next() >> 1) as i64
    }
}
