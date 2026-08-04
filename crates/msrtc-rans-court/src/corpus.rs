// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # Deterministic corpus generation (Phase: corpus expansion)
//!
//! Shared LCG + generators used by the differential courts to expand their
//! corpora far beyond the hand-picked boundary cases. Everything is seeded
//! and reproducible; the same inputs drive both the Rust side and the C++
//! oracle casefiles.

/// Deterministic xorshift64* generator.
pub struct Lcg(u64);

impl Lcg {
    pub fn new(seed: u64) -> Self {
        Lcg(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    pub fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 { 0 } else { self.next() % bound }
    }
}

/// Generate a deterministic sequence of valid (start, freq) raw symbols.
///
/// Excludes the degenerate full-scale symbol (`freq == scale`), which
/// overflows the encoder x_max in both Rust and C++ (see hardening docs:
/// the safe operational domain for RansByte is scale_bits <= 23).
pub fn gen_symbols(rng: &mut Lcg, scale: u32, count: usize) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let start = rng.below(scale as u64) as u32;
        let mut max_freq = scale - start;
        if max_freq == scale {
            max_freq -= 1;
        }
        let max_freq = max_freq.max(1);
        let freq = match rng.below(4) {
            0 => 1,
            1 => max_freq,
            2 => (max_freq / 2).max(1),
            _ => 1 + rng.below(max_freq as u64) as u32,
        };
        out.push((start, freq.clamp(1, max_freq)));
    }
    out
}

/// Generate a deterministic multi-distribution PMF (frequency tables summing
/// to `scale` with every entry >= 1) plus matching value ranges.
pub fn gen_pmf(
    rng: &mut Lcg,
    scale: u32,
    center: i32,
    dist_count: usize,
) -> (Vec<i32>, Vec<i32>, Vec<i32>) {
    let mut lengths = Vec::new();
    let mut offsets = Vec::new();
    let mut table = Vec::new();
    for _ in 0..dist_count {
        let mut pmf: Vec<f64> = (0..=2 * center as usize)
            .map(|i| 1.0 / (1.0 + ((i as i64 - center as i64) as f64) * 0.25))
            .collect();
        pmf.push(pmf[0]); // tail mass
        let sum: f64 = pmf.iter().sum();
        let mut base: Vec<i64> = pmf
            .iter()
            .map(|p| ((p / sum) * scale as f64).floor() as i64)
            .collect();
        for b in base.iter_mut() {
            *b = (*b).max(1);
        }
        let mut total: i64 = base.iter().sum();
        let target = scale as i64;
        let mut idx = 0usize;
        let len = base.len();
        while total < target {
            base[idx % len] += 1;
            total += 1;
            idx += 1;
        }
        while total > target {
            if let Some((i, _)) = base.iter().enumerate().find(|(_, b)| **b > 1) {
                base[i] -= 1;
                total -= 1;
            } else {
                break;
            }
        }
        lengths.push(base.len() as i32);
        offsets.push(-center);
        table.extend(base.iter().map(|b| *b as i32));
    }
    (lengths, offsets, table)
}

/// Generate a deterministic batch of values covering in-range symbols and
/// bypass outliers, for `dist_count` distributions.
pub fn gen_values(
    rng: &mut Lcg,
    dist_count: usize,
    center: i32,
    count: usize,
) -> (Vec<i32>, Vec<i32>) {
    let mut values = Vec::with_capacity(count);
    let mut indices = Vec::with_capacity(count);
    for i in 0..count {
        indices.push((i % dist_count) as i32);
        let v = match rng.below(12) {
            0 => center + 10 + (rng.below(200) as i32), // positive outlier
            1 => -(center + 10 + (rng.below(200) as i32)), // negative outlier
            2 => center * 4 + (rng.below(400) as i32),  // extreme positive
            _ => (rng.below((2 * center + 1) as u64) as i32) - center, // in-range
        };
        values.push(v);
    }
    (values, indices)
}
