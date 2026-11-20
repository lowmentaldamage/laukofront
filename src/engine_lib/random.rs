//! Port of upstream `packages/engine-lib/src/PseudoRandom.ts` (sfc32 seeded with splitmix32).
//! All state updates are 32-bit integer operations, so the stream is bit-identical to upstream.

/// Seeded pseudo-random generator. `next()` returns the same doubles as upstream for the same seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseudoRandom {
    s0: i32,
    s1: i32,
    s2: i32,
    s3: i32,
}

impl PseudoRandom {
    /// Upstream truncates the seed to 32 bits (`seed | 0`); this does the same (wrapping).
    pub fn new(seed: i64) -> Self {
        let mut h = seed as i32;
        let mut split = || {
            h = h.wrapping_add(0x9e37_79b9_u32 as i32);
            let mut t = h ^ ((h as u32) >> 16) as i32;
            t = t.wrapping_mul(0x21f0_aaad);
            t ^= ((t as u32) >> 15) as i32;
            t = t.wrapping_mul(0x735a_2d97);
            t ^ ((t as u32) >> 15) as i32
        };
        let (s0, s1, s2, s3) = (split(), split(), split(), split());
        let mut r = Self { s0, s1, s2, s3 };
        for _ in 0..12 {
            r.next();
        }
        r
    }

    /// The four state words (for snapshots / save files).
    pub fn state(&self) -> [i32; 4] {
        [self.s0, self.s1, self.s2, self.s3]
    }

    /// Rebuilds a generator from `state()` output.
    pub fn from_state(s: [i32; 4]) -> Self {
        Self {
            s0: s[0],
            s1: s[1],
            s2: s[2],
            s3: s[3],
        }
    }

    /// Next value in `[0, 1)`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        let t = self.s0.wrapping_add(self.s1).wrapping_add(self.s3);
        self.s3 = self.s3.wrapping_add(1);
        self.s0 = self.s1 ^ ((self.s1 as u32) >> 9) as i32;
        self.s1 = self.s2.wrapping_add(self.s2 << 3);
        self.s2 = (self.s2 << 21) | ((self.s2 as u32) >> 11) as i32;
        self.s2 = self.s2.wrapping_add(t);
        (t as u32) as f64 / 4_294_967_296.0
    }

    /// `floor(next() * (max - min)) + min`, exactly as upstream `nextInt`.
    pub fn next_int(&mut self, min: i64, max: i64) -> i64 {
        (self.next() * (max - min) as f64).floor() as i64 + min
    }

    /// Float in `[min, max)`.
    pub fn next_float(&mut self, min: f64, max: f64) -> f64 {
        self.next() * (max - min) + min
    }

    /// True with probability `1 / odds`.
    pub fn chance(&mut self, odds: i64) -> bool {
        self.next_int(0, odds) == 0
    }

    /// Random index into a slice of length `len`, or `None` if empty.
    pub fn index(&mut self, len: usize) -> Option<usize> {
        (len > 0).then(|| self.next_int(0, len as i64) as usize)
    }

    /// Fisher-Yates shuffle in place, same order of draws as upstream `shuffleArray`.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.next_int(0, i as i64 + 1) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values from upstream's algorithm run in JavaScript (see docs/TESTING.md).
    /// (seed, next() bit patterns x4, then nextInt(-10, 1000) x4)
    #[rustfmt::skip]
    const VECTORS: [(i64, [u64; 4], [i64; 4]); 6] = [
        (0, [0x3fc0564fa7000000, 0x3fd05957cb400000, 0x3fe088f954200000, 0x3fd8cc7af5400000], [808, 703, 839, 107]),
        (1, [0x3fd0d8bb2f000000, 0x3fe34f11da600000, 0x3fe6769103c00000, 0x3fcca2b5a0000000], [632, 709, 332, 625]),
        (42, [0x3fec814574600000, 0x3fdb969a39c00000, 0x3fd49cb71f800000, 0x3fca88a015800000], [647, 991, 282, 409]),
        (123, [0x3f99a438bc000000, 0x3fe8d85cbd200000, 0x3fbb588e0b000000, 0x3fce8ea2e0000000], [25, 453, 369, 703]),
        (-7, [0x3fd9f4b39a800000, 0x3fe9d51a62c00000, 0x3f96457480000000, 0x3fe45780b3600000], [485, 262, 925, 635]),
        (4294967297, [0x3fd0d8bb2f000000, 0x3fe34f11da600000, 0x3fe6769103c00000, 0x3fcca2b5a0000000], [632, 709, 332, 625]),
    ];

    #[test]
    fn matches_upstream_stream() {
        for (seed, next_bits, ints) in VECTORS {
            let mut r = PseudoRandom::new(seed);
            for b in next_bits {
                assert_eq!(r.next().to_bits(), b, "seed {seed}");
            }
            for i in ints {
                assert_eq!(r.next_int(-10, 1000), i, "seed {seed}");
            }
        }
    }

    #[test]
    fn state_round_trips() {
        let mut a = PseudoRandom::new(9);
        a.next();
        let mut b = PseudoRandom::from_state(a.state());
        assert_eq!(a.next().to_bits(), b.next().to_bits());
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut v: Vec<u32> = (0..50).collect();
        PseudoRandom::new(3).shuffle(&mut v);
        let mut s = v.clone();
        s.sort_unstable();
        assert_eq!(s, (0..50).collect::<Vec<_>>());
        assert_ne!(v, (0..50).collect::<Vec<_>>());
    }
}
