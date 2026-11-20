/// SplitMix64: tiny, fast, and fully defined by this file, so the same seed produces the same
/// stream on every platform and every dependency version. Not for cryptography.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in `0..bound`. Returns 0 when `bound` is 0.
    /// Uses Lemire's multiply-shift; the tiny bias is irrelevant for game use.
    pub fn below(&mut self, bound: u32) -> u32 {
        ((self.next_u64() >> 32).wrapping_mul(bound as u64) >> 32) as u32
    }

    /// Uniform integer in `lo..=hi`. Returns `lo` when `hi <= lo`.
    pub fn range_inclusive(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            return lo;
        }
        lo + self.below(hi - lo + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_first_value() {
        // Reference value for SplitMix64 with seed 0.
        assert_eq!(Rng::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn range_stays_in_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.range_inclusive(5, 20);
            assert!((5..=20).contains(&v));
        }
    }
}
