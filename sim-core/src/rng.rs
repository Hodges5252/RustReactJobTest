/// Hand-rolled SplitMix64 PRNG. Deterministic for a given seed, avoids the
/// `rand` crate's `getrandom` friction under wasm (see spec 2.7).
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform f64 in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next_f64() as f32
    }

    /// Uniform integer in [0, n). n must be > 0.
    pub fn gen_index(&mut self, n: usize) -> usize {
        ((self.next_f64() * n as f64) as usize).min(n - 1)
    }

    /// Uniform f32 in [lo, hi).
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// Approximately normal sample (Irwin-Hall sum of 12 uniforms).
    /// Plenty for departure-time spread; bounded within ±6 std.
    pub fn normal(&mut self, mean: f64, std: f64) -> f64 {
        let s: f64 = (0..12).map(|_| self.next_f64()).sum::<f64>() - 6.0;
        mean + std * s
    }
}
