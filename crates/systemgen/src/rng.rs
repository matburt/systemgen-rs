//! The randomness every layer draws from. Callers with their own seeded streams implement
//! [`Rng`] (only [`Rng::next_u64`] is required); [`SplitMix64`] is the built-in stream that
//! [`crate::generate`] uses.

pub trait Rng {
    fn next_u64(&mut self) -> u64;

    /// Uniform in `[0, 1)`, from the top 53 bits.
    fn unit_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in `[lo, hi)`; `lo` when the range is empty.
    fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit_f64()
    }

    /// Uniform in `0..n`, by modulo. `n` must be positive.
    fn below(&mut self, n: u32) -> u32 {
        (self.next_u64() % u64::from(n)) as u32
    }

    /// True with probability `p`.
    fn chance(&mut self, p: f64) -> bool {
        self.unit_f64() < p
    }

    /// The sum of `n` six-sided dice.
    fn dice(&mut self, n: u32) -> i32 {
        (0..n).map(|_| self.below(6) as i32 + 1).sum()
    }
}

/// Steele, Lea and Flood's SplitMix64: small, fast, and identical on every platform.
#[derive(Debug, Clone)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
}

impl Rng for SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_matches_the_reference_sequence() {
        // First outputs for seed 1234567 from the reference C implementation.
        let mut rng = SplitMix64::new(1234567);
        assert_eq!(rng.next_u64(), 6457827717110365317);
        assert_eq!(rng.next_u64(), 3203168211198807973);
    }

    #[test]
    fn draws_stay_in_their_ranges() {
        let mut rng = SplitMix64::new(7);
        for _ in 0..10_000 {
            let u = rng.unit_f64();
            assert!((0.0..1.0).contains(&u));
            assert!((2.0..3.0).contains(&rng.range_f64(2.0, 3.0)));
            assert!(rng.below(6) < 6);
            assert!((2..=12).contains(&rng.dice(2)));
        }
    }
}
