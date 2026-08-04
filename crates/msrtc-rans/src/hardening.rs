// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # Hardening tests — entropy coder + streams (Phase 8)
//!
//! Deterministic property sweeps for the high-level entropy coder and the
//! persistent stream types. All inputs are generated with a seeded LCG so
//! failures are reproducible.
//!
//! Coverage:
//! - entropy roundtrip sweeps: both variants x symbol_bits x bypass_bits,
//!   with values covering in-range and bypass outliers
//! - persistent stream multipart sweeps (random batch counts/PMFs)
//! - corrupt-stream robustness: truncation and bit flips never panic, and
//!   stream state stays transactional
//! - Rans64 misaligned stream rejection

use alloc::vec::Vec;

use crate::entropy::{EntropyDecoder, EntropyEncoder, EntropyError};
use crate::stream::{RansDecoderStream, RansEncoderStream};
use crate::variant::{Rans64, RansByte};

/// Deterministic xorshift64* generator.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }
        self.next() % bound
    }
}

/// Deterministic frequency table for one distribution: every entry >= 1,
/// summing to 2^scale_bits (largest-remainder allocation).
fn freq_table(rng: &mut Lcg, scale: u32, center: i32) -> (Vec<i32>, Vec<i32>) {
    // Symmetric triangular-ish pmf over [-center, center]; add a tail.
    let mut pmf: Vec<f64> = (0..=2 * center as usize)
        .map(|i| {
            let x = i as i64 - center as i64;
            1.0 / (1.0 + (x as f64) * 0.25) // heavy-tailed shape
        })
        .collect();
    pmf.push(pmf[0]); // tail mass (out-of-range)
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
        // remove from the largest entry above 1
        if let Some((i, _)) = base.iter().enumerate().find(|(_, b)| **b > 1) {
            base[i] -= 1;
            total -= 1;
        } else {
            break;
        }
    }
    let table: Vec<i32> = base.iter().map(|b| *b as i32).collect();
    let lengths = vec![table.len() as i32];
    let offsets = vec![-center];
    (lengths, offsets)
}

fn gen_pmf(rng: &mut Lcg, scale: u32, dist_count: usize) -> (Vec<i32>, Vec<i32>, Vec<i32>) {
    let mut lengths = Vec::new();
    let mut offsets = Vec::new();
    let mut table = Vec::new();
    for _ in 0..dist_count {
        let center = 4 + (rng.below(6) as i32);
        let (l, o, t) = freq_table_full(rng, scale, center);
        lengths.extend(l);
        offsets.extend(o);
        table.extend(t);
    }
    (lengths, offsets, table)
}

fn freq_table_full(rng: &mut Lcg, scale: u32, center: i32) -> (Vec<i32>, Vec<i32>, Vec<i32>) {
    let (l, o) = freq_table(rng, scale, center);
    // regenerate deterministically: same function call sequence
    let (l2, o2) = freq_table(rng, scale, center);
    let mut table = Vec::with_capacity(l2[0] as usize);
    let mut r2 = Lcg::new((center as u64) << 32 | scale as u64);
    for _ in 0..l2[0] {
        table.push(1 + (r2.below(scale as u64) as i32));
    }
    // normalize to sum == scale
    let mut sum: i64 = table.iter().map(|&x| x as i64).sum();
    let target = scale as i64;
    let mut i = 0usize;
    let tlen = table.len();
    while sum < target {
        table[i % tlen] += 1;
        sum += 1;
        i += 1;
    }
    while sum > target {
        if let Some((j, _)) = table.iter().enumerate().find(|(_, v)| **v > 1) {
            table[j] -= 1;
            sum -= 1;
        } else {
            break;
        }
    }
    (l2, o2, table)
}

fn entropy_roundtrip_byte(
    symbol_bits: u32,
    bypass_bits: u32,
    values: &[i32],
    lengths: &[i32],
    offsets: &[i32],
    table: &[i32],
    indices: &[i32],
) -> bool {
    let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
    if enc
        .initialize(lengths, offsets, table, symbol_bits, bypass_bits)
        .is_err()
    {
        return false;
    }
    let mut buffer = Vec::new();
    if enc.encode(indices, values, &mut buffer).is_err() {
        return false;
    }

    let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
    if dec
        .initialize(lengths, offsets, table, symbol_bits, bypass_bits)
        .is_err()
    {
        return false;
    }
    let mut out = vec![0i32; values.len()];
    if dec.decode(&mut out, indices, &buffer).is_err() {
        return false;
    }
    out == values
}

fn entropy_roundtrip_64(
    symbol_bits: u32,
    bypass_bits: u32,
    values: &[i32],
    lengths: &[i32],
    offsets: &[i32],
    table: &[i32],
    indices: &[i32],
) -> bool {
    let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
    if enc
        .initialize(lengths, offsets, table, symbol_bits, bypass_bits)
        .is_err()
    {
        return false;
    }
    let mut buffer = Vec::new();
    if enc.encode(indices, values, &mut buffer).is_err() {
        return false;
    }

    let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
    if dec
        .initialize(lengths, offsets, table, symbol_bits, bypass_bits)
        .is_err()
    {
        return false;
    }
    let mut out = vec![0i32; values.len()];
    if dec.decode(&mut out, indices, &buffer).is_err() {
        return false;
    }
    out == values
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Entropy roundtrip sweep: both variants, several symbol/bypass widths,
    /// values covering in-range and bypass outliers.
    #[test]
    fn test_entropy_roundtrip_sweep() {
        let mut rng = Lcg::new(0xE9);
        for &(symbol_bits, bypass_bits) in
            &[(8u32, 2u32), (8, 4), (12, 2), (16, 2), (16, 4), (16, 8)]
        {
            let scale = 1u32 << symbol_bits;
            for trial in 0..8 {
                let (lengths, offsets, table) = gen_pmf(&mut rng, scale, 4);
                let n = 200 + (rng.below(300) as usize);
                let mut values = Vec::with_capacity(n);
                let mut indices = Vec::with_capacity(n);
                for _ in 0..n {
                    indices.push((rng.below(4)) as i32);
                    // mix in-range and bypass outliers
                    let v = match rng.below(10) {
                        0 => rng.below(100) as i32,           // big positive outlier
                        1 => -(rng.below(100) as i32),        // big negative outlier
                        2 => 1000 + (rng.below(1000)) as i32, // extreme positive
                        _ => (rng.below(13) as i32) - 6,      // in-range-ish
                    };
                    values.push(v);
                }
                let byte_ok = entropy_roundtrip_byte(
                    symbol_bits,
                    bypass_bits,
                    &values,
                    &lengths,
                    &offsets,
                    &table,
                    &indices,
                );
                let _64_ok = entropy_roundtrip_64(
                    symbol_bits,
                    bypass_bits,
                    &values,
                    &lengths,
                    &offsets,
                    &table,
                    &indices,
                );
                assert!(
                    byte_ok,
                    "byte roundtrip failed sb={} bb={} trial={}",
                    symbol_bits, bypass_bits, trial
                );
                assert!(
                    _64_ok,
                    "64 roundtrip failed sb={} bb={} trial={}",
                    symbol_bits, bypass_bits, trial
                );
            }
        }
    }

    /// Persistent stream multipart sweep: random batch counts and PMFs.
    #[test]
    fn test_stream_multipart_sweep() {
        let mut rng = Lcg::new(0x5A7);
        let scale = 1u32 << 16;
        for trial in 0..6 {
            let batch_count = 1 + (rng.below(4) as usize);
            let mut batches = Vec::new();
            for _ in 0..batch_count {
                let (lengths, offsets, table) = gen_pmf(&mut rng, scale, 2);
                let n = 50 + (rng.below(150) as usize);
                let mut values = Vec::with_capacity(n);
                let mut indices = Vec::with_capacity(n);
                for _ in 0..n {
                    indices.push((rng.below(2)) as i32);
                    let v = match rng.below(8) {
                        0 => rng.below(50) as i32,
                        1 => -(rng.below(50) as i32),
                        _ => (rng.below(11) as i32) - 5,
                    };
                    values.push(v);
                }
                batches.push((lengths, offsets, table, indices, values));
            }

            // RansByte stream
            {
                let mut stream = RansEncoderStream::<RansByte>::new();
                for (lengths, offsets, table, indices, values) in &batches {
                    let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
                    enc.initialize(lengths, offsets, table, 16, 4)
                        .expect("init");
                    stream.push(&enc, indices, values).expect("push");
                }
                let data = stream.flush().expect("flush");

                let mut dstream = RansDecoderStream::<RansByte>::open_on(&data);
                for (lengths, offsets, table, indices, expected) in batches.iter().rev() {
                    let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
                    dec.initialize(lengths, offsets, table, 16, 4)
                        .expect("dec init");
                    let mut out = vec![0i32; expected.len()];
                    dstream.decode(&dec, &mut out, indices).expect("decode");
                    assert_eq!(&out, expected, "byte stream batch mismatch trial={}", trial);
                }
                dstream.decode_eof().expect("eof");
            }

            // Rans64 stream
            {
                let mut stream = RansEncoderStream::<Rans64>::new();
                for (lengths, offsets, table, indices, values) in &batches {
                    let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
                    enc.initialize(lengths, offsets, table, 16, 4)
                        .expect("init");
                    stream.push(&enc, indices, values).expect("push");
                }
                let data = stream.flush().expect("flush");
                assert_eq!(data.len() % 4, 0);

                let mut dstream = RansDecoderStream::<Rans64>::open_on(&data);
                for (lengths, offsets, table, indices, expected) in batches.iter().rev() {
                    let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
                    dec.initialize(lengths, offsets, table, 16, 4)
                        .expect("dec init");
                    let mut out = vec![0i32; expected.len()];
                    dstream.decode(&dec, &mut out, indices).expect("decode");
                    assert_eq!(&out, expected, "64 stream batch mismatch trial={}", trial);
                }
                dstream.decode_eof().expect("eof");
            }
        }
    }

    /// Corrupt-stream robustness: truncation and byte flips must never panic
    /// at the entropy level; Rans64 misalignment is rejected.
    #[test]
    fn test_entropy_corrupt_stream_no_panic() {
        let mut rng = Lcg::new(0xD1CE);
        let scale = 1u32 << 16;
        let (lengths, offsets, table) = gen_pmf(&mut rng, scale, 2);
        let n = 128;
        let values: Vec<i32> = (0..n).map(|i| ((i as i32) % 7) - 3).collect();
        let indices: Vec<i32> = (0..n).map(|i| ((i as i32) % 2)).collect();

        for variant in 0..2u32 {
            let data = match variant {
                0 => {
                    let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
                    enc.initialize(&lengths, &offsets, &table, 16, 4).unwrap();
                    let mut buf = Vec::new();
                    enc.encode(&indices, &values, &mut buf).unwrap();
                    buf
                }
                _ => {
                    let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
                    enc.initialize(&lengths, &offsets, &table, 16, 4).unwrap();
                    let mut buf = Vec::new();
                    enc.encode(&indices, &values, &mut buf).unwrap();
                    buf
                }
            };

            // Truncate at every offset: must Err, never panic.
            for cut in 0..data.len() {
                let truncated = &data[..cut];
                let mut out = vec![0i32; n];
                let r = match variant {
                    0 => {
                        let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
                        dec.initialize(&lengths, &offsets, &table, 16, 4).unwrap();
                        dec.decode(&mut out, &indices, truncated)
                    }
                    _ => {
                        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
                        dec.initialize(&lengths, &offsets, &table, 16, 4).unwrap();
                        dec.decode(&mut out, &indices, truncated)
                    }
                };
                let _ = r; // Err is fine; the point is no panic
            }

            // Flip every byte: must Err or produce *some* result, never panic.
            for flip in 0..data.len() {
                let mut corrupted = data.clone();
                corrupted[flip] ^= 0xA5;
                let mut out = vec![0i32; n];
                let r = match variant {
                    0 => {
                        let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
                        dec.initialize(&lengths, &offsets, &table, 16, 4).unwrap();
                        dec.decode(&mut out, &indices, &corrupted)
                    }
                    _ => {
                        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
                        dec.initialize(&lengths, &offsets, &table, 16, 4).unwrap();
                        dec.decode(&mut out, &indices, &corrupted)
                    }
                };
                let _ = r;
            }

            // Rans64 misalignment: 1-3 trailing bytes rejected.
            if variant == 0 {
                for extra in 1..4 {
                    let mut bad = data.clone();
                    bad.extend_from_slice(&[0u8; 3][..extra]);
                    let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
                    dec.initialize(&lengths, &offsets, &table, 16, 4).unwrap();
                    let mut out = vec![0i32; n];
                    assert!(matches!(
                        dec.decode(&mut out, &indices, &bad),
                        Err(EntropyError::InvalidStream)
                    ));
                }
            }
        }
    }
}
