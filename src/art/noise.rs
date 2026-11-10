//! Deterministic randomness: an integer hash, value noise and fractal noise built on it, and a
//! small xorshift RNG for scattering things. Everything is seeded, so every run paints the
//! same fridge.

use super::lerp;

/// Deterministic hash of an integer lattice point to 0..1 (24 bits of precision).
///
/// Multiplies each input by a large odd constant, xors them and runs a short avalanche
/// (xorshift, multiply, xorshift). Same inputs always give the same value.
pub(crate) fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut state = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    state = (state ^ (state >> 13)).wrapping_mul(1_274_126_177);
    state ^= state >> 16;

    // Keep the low 24 bits, the most an f32 mantissa holds exactly; 2^24 = 16 777 216.
    (state & 0xFF_FFFF) as f32 / 16_777_216.0
}

/// Value noise in 0..1: [`hash`] at the four surrounding lattice points, blended with a
/// smoothstep so the result is continuous with a continuous slope. One lattice cell is one
/// unit of `x` and `y`.
pub(crate) fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (frac_x, frac_y) = (x - x0, y - y0);
    // Smoothstep 3t² - 2t³ of the fractions.
    let (ease_x, ease_y) = (
        frac_x * frac_x * (3.0 - 2.0 * frac_x),
        frac_y * frac_y * (3.0 - 2.0 * frac_y),
    );

    let (ix, iy) = (x0 as i32, y0 as i32);
    let top_left = hash(ix, iy, seed);
    let top_right = hash(ix + 1, iy, seed);
    let bottom_left = hash(ix, iy + 1, seed);
    let bottom_right = hash(ix + 1, iy + 1, seed);

    let top = lerp(top_left, top_right, ease_x);
    let bottom = lerp(bottom_left, bottom_right, ease_x);
    lerp(top, bottom, ease_y)
}

/// Fractal (fBm) value noise in 0..1, summing `octaves` layers of [`vnoise`].
///
/// 1. Octave `o` is sampled at frequency `2.03^o` with seed `seed + 101 o`, so layers are
///    decorrelated. The 2.03 (rather than 2) keeps the lattices of successive octaves from
///    lining up into visible grid artefacts.
/// 2. Each octave has half the amplitude of the one before.
/// 3. The sum is divided by the total amplitude, so the result stays in 0..1.
pub(crate) fn fbm(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amplitude, mut total_amplitude, mut frequency) = (0.0, 1.0, 0.0, 1.0);
    for octave in 0..octaves {
        sum += amplitude * vnoise(x * frequency, y * frequency, seed + octave * 101);
        total_amplitude += amplitude;
        amplitude *= 0.5;
        frequency *= 2.03;
    }
    sum / total_amplitude
}

/// Small deterministic RNG for scattering things (xorshift64). The state must not be 0.
pub struct Rng(pub u64);

impl Rng {
    /// Next 32 random bits (the high half of the 64-bit state).
    pub fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }

    /// Uniform float in 0..1 (24 bits of precision).
    pub fn f(&mut self) -> f32 {
        (self.next() >> 8) as f32 / 16_777_216.0
    }

    /// Uniform float in `a..b`.
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}
