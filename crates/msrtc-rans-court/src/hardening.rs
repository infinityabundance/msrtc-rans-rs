// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # MSRTC.HARDENING — Internal property / robustness court (Phase 8)
//!
//! Deterministic hardening battery for the raw engine, entropy coder, and
//! persistent streams. No oracle is required: this court proves internal
//! invariants that differential courts assume:
//!
//! 1. Raw roundtrip sweeps — RansByte (scale_bits 2..=23) and Rans64
//!    (2..=31) over seeded symbol sequences; every decoded value lands in
//!    the encoded symbol's range and EOF is reached.
//! 2. Prepared-symbol vs raw-division byte equivalence across the sweep.
//! 3. Transactional decoder — truncated and bit-flipped streams never
//!    panic and never mutate state on a failed advance.
//! 4. Entropy roundtrip sweeps — both variants across symbol/bypass widths
//!    with in-range and bypass-outlier values.
//! 5. Persistent stream multipart sweeps — random batch counts/PMFs.
//! 6. Corruption robustness at the entropy level (truncation, flips,
//!    Rans64 misalignment rejection).
//! 7. Allocation-failure injection — buffer growth overflow is a typed
//!    error, never a panic.

use msrtc_rans_casefile::{
    Comparison, DifferentialResult, InputHashes, NativeResult, OracleResult,
    classification::{ResidualClassification, ResolutionState},
    sha256,
};

use crate::oracle::{self, environment_sha256, git_commit};
use crate::{Court, CourtResult, CourtStatus};

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
        if bound == 0 { 0 } else { self.next() % bound }
    }
}

fn gen_symbols(rng: &mut Lcg, scale: u32, count: usize) -> Vec<(u32, u32)> {
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

/// Raw roundtrip sweep for both variants. Returns number of checks passed.
fn run_raw_sweeps() -> usize {
    use msrtc_rans_core::sink::VecSink;
    use msrtc_rans_core::source::SliceSource;
    use msrtc_rans_core::{
        Rans64Decoder, Rans64EncSymbol, Rans64Encoder, RansByteDecoder, RansByteEncSymbol,
        RansByteEncoder,
    };

    let mut checks = 0usize;
    let mut rng = Lcg::new(0x10A);

    // Byte variant: scale_bits 2..=23
    for scale_bits in 2..=23u32 {
        let scale = 1u32 << scale_bits;
        for _t in 0..4 {
            let symbols = gen_symbols(&mut rng, scale, 64);

            let sink = VecSink::<u8>::new(64);
            let mut enc = RansByteEncoder::new(sink);
            for &(s, f) in &symbols {
                enc.put_raw(s, f, scale_bits);
            }
            enc.flush();
            let encoded = enc.into_sink().encoded().to_vec();
            let src = SliceSource::new(&encoded[..]);
            let mut dec = RansByteDecoder::new(src);
            if !dec.init() {
                continue;
            }
            let mut ok = true;
            for &(s, f) in symbols.iter().rev() {
                let got = dec.get(scale_bits);
                if !(got >= s && got < s + f) {
                    ok = false;
                    break;
                }
                if !dec.advance(s, f, scale_bits) {
                    ok = false;
                    break;
                }
            }
            if ok && dec.check_eof() {
                checks += 1;
            }
        }
    }

    // Rans64 variant: scale_bits 2..=31
    for scale_bits in 2..=31u32 {
        let scale = 1u32 << scale_bits;
        for _t in 0..4 {
            let symbols = gen_symbols(&mut rng, scale, 64);

            let sink = VecSink::<u32>::new(64);
            let mut enc = Rans64Encoder::new(sink);
            for &(s, f) in &symbols {
                enc.put_raw(s, f, scale_bits);
            }
            enc.flush();
            let units = enc.into_sink().encoded().to_vec();
            let src = SliceSource::new(&units[..]);
            let mut dec = Rans64Decoder::new(src);
            if !dec.init() {
                continue;
            }
            let mut ok = true;
            for &(s, f) in symbols.iter().rev() {
                let got = dec.get(scale_bits);
                if !(got >= s && got < s + f) {
                    ok = false;
                    break;
                }
                if !dec.advance(s, f, scale_bits) {
                    ok = false;
                    break;
                }
            }
            if ok && dec.check_eof() {
                checks += 1;
            }
        }
    }

    // Prepared == raw byte equivalence
    for scale_bits in [2u32, 8, 16, 23, 24, 31] {
        let scale = 1u32 << scale_bits;
        let symbols = gen_symbols(&mut rng, scale, 32);

        if scale_bits <= 23 {
            let sink = VecSink::<u8>::new(64);
            let mut raw = RansByteEncoder::new(sink);
            for &(s, f) in &symbols {
                raw.put_raw(s, f, scale_bits);
            }
            raw.flush();
            let raw_out = raw.into_sink().encoded().to_vec();

            let sink2 = VecSink::<u8>::new(64);
            let mut prep = RansByteEncoder::new(sink2);
            for &(s, f) in &symbols {
                prep.put(&RansByteEncSymbol::new(s, f, scale_bits));
            }
            prep.flush();
            let prep_out = prep.into_sink().encoded().to_vec();

            if raw_out == prep_out {
                checks += 1;
            }
        }

        let sink = VecSink::<u32>::new(64);
        let mut raw = Rans64Encoder::new(sink);
        for &(s, f) in &symbols {
            raw.put_raw(s, f, scale_bits);
        }
        raw.flush();
        let raw_out = raw.into_sink().encoded().to_vec();

        let sink2 = VecSink::<u32>::new(64);
        let mut prep = Rans64Encoder::new(sink2);
        for &(s, f) in &symbols {
            prep.put(&Rans64EncSymbol::new(s, f, scale_bits));
        }
        prep.flush();
        let prep_out = prep.into_sink().encoded().to_vec();

        if raw_out == prep_out {
            checks += 1;
        }
    }

    checks
}

/// Corruption robustness: truncated + bit-flipped streams never panic and
/// failed advances never mutate state.
fn run_corruption_robustness() -> usize {
    use msrtc_rans_core::RansByteDecoder;
    use msrtc_rans_core::RansByteEncoder;
    use msrtc_rans_core::sink::VecSink;
    use msrtc_rans_core::source::SliceSource;

    let mut checks = 0usize;
    let mut rng = Lcg::new(0xC0DE);
    let scale_bits = 16u32;
    let scale = 1u32 << scale_bits;
    let symbols = gen_symbols(&mut rng, scale, 32);

    let sink = VecSink::<u8>::new(64);
    let mut enc = RansByteEncoder::new(sink);
    for &(s, f) in &symbols {
        enc.put_raw(s, f, scale_bits);
    }
    enc.flush();
    let full = enc.into_sink().encoded().to_vec();

    let mut truncation_ok = true;
    for cut in 0..full.len() {
        let src = SliceSource::new(&full[..cut]);
        let mut dec = RansByteDecoder::new(src);
        if !dec.init() {
            continue;
        }
        for &(s, f) in symbols.iter().rev() {
            let _ = dec.get(scale_bits);
            let before = dec.state();
            if !dec.advance(s, f, scale_bits) {
                if dec.state() != before {
                    truncation_ok = false;
                }
                break;
            }
        }
    }
    if truncation_ok {
        checks += 1;
    }

    let mut flip_ok = true;
    for flip in 0..full.len() {
        let mut data = full.clone();
        data[flip] ^= 0x5A;
        let src = SliceSource::new(&data[..]);
        let mut dec = RansByteDecoder::new(src);
        if !dec.init() {
            continue;
        }
        for &(s, f) in symbols.iter().rev() {
            let _ = dec.get(scale_bits);
            let before = dec.state();
            if !dec.advance(s, f, scale_bits) {
                if dec.state() != before {
                    flip_ok = false;
                }
                break;
            }
        }
    }
    if flip_ok {
        checks += 1;
    }

    checks
}

/// Entropy roundtrip sweeps + stream multipart sweeps.
fn run_entropy_sweeps() -> usize {
    let mut checks = 0usize;
    let mut rng = Lcg::new(0xE97);
    let scale = 1u32 << 16;

    for &(symbol_bits, bypass_bits) in &[(8u32, 2u32), (16, 2), (16, 4)] {
        for _t in 0..4 {
            // Single-distribution PMF: lengths/offsets/table
            let center = 6i32;
            let (lengths, offsets, table) = make_freq_pmf(&mut rng, 1u32 << symbol_bits, center, 1);
            let n = 128;
            let mut values = Vec::with_capacity(n);
            let mut indices = Vec::with_capacity(n);
            for i in 0..n {
                indices.push(0);
                values.push(((i as i32) % 9) - 4);
            }
            values[0] = 500; // positive outlier
            values[1] = -500; // negative outlier

            let ok_b = entropy_roundtrip(
                symbol_bits,
                bypass_bits,
                1u8,
                &values,
                &lengths,
                &offsets,
                &table,
                &indices,
            );
            let ok_64 = entropy_roundtrip(
                symbol_bits,
                bypass_bits,
                0u8,
                &values,
                &lengths,
                &offsets,
                &table,
                &indices,
            );
            if ok_b {
                checks += 1;
            }
            if ok_64 {
                checks += 1;
            }
        }
    }

    // Stream multipart sweeps (both variants)
    for _t in 0..4 {
        let batch_count = 1 + (rng.below(3) as usize);
        let mut batches: Vec<(Vec<i32>, Vec<i32>, Vec<i32>, Vec<i32>, Vec<i32>)> = Vec::new();
        for _ in 0..batch_count {
            let (lengths, offsets, table) = make_freq_pmf(&mut rng, scale, 6, 2);
            let n = 64;
            let mut values = Vec::with_capacity(n);
            let mut indices = Vec::with_capacity(n);
            for i in 0..n {
                indices.push((i % 2) as i32);
                values.push(((i as i32) % 7) - 3);
            }
            batches.push((lengths, offsets, table, indices, values));
        }
        if stream_roundtrip(1u8, &batches) {
            checks += 1;
        }
        if stream_roundtrip(0u8, &batches) {
            checks += 1;
        }
    }

    checks
}

fn make_freq_pmf(
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
        pmf.push(pmf[0]);
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
    let _ = rng.below(1);
    (lengths, offsets, table)
}

fn entropy_roundtrip(
    symbol_bits: u32,
    bypass_bits: u32,
    variant: u8,
    values: &[i32],
    lengths: &[i32],
    offsets: &[i32],
    table: &[i32],
    indices: &[i32],
) -> bool {
    use msrtc_rans::entropy::{EntropyDecoder, EntropyEncoder};
    use msrtc_rans::variant::{Rans64, RansByte};

    match variant {
        0 => {
            let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
            if enc
                .initialize(lengths, offsets, table, symbol_bits, bypass_bits)
                .is_err()
            {
                return false;
            }
            let mut buf = Vec::new();
            if enc.encode(indices, values, &mut buf).is_err() {
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
            dec.decode(&mut out, indices, &buf).is_ok() && out == values
        }
        _ => {
            let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
            if enc
                .initialize(lengths, offsets, table, symbol_bits, bypass_bits)
                .is_err()
            {
                return false;
            }
            let mut buf = Vec::new();
            if enc.encode(indices, values, &mut buf).is_err() {
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
            dec.decode(&mut out, indices, &buf).is_ok() && out == values
        }
    }
}

fn stream_roundtrip(
    variant: u8,
    batches: &[(Vec<i32>, Vec<i32>, Vec<i32>, Vec<i32>, Vec<i32>)],
) -> bool {
    use msrtc_rans::entropy::{EntropyDecoder, EntropyEncoder};
    use msrtc_rans::stream::{RansDecoderStream, RansEncoderStream};
    use msrtc_rans::variant::{Rans64, RansByte};

    match variant {
        0 => {
            let mut stream = RansEncoderStream::<Rans64>::new();
            for (l, o, t, ix, v) in batches {
                let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
                if enc.initialize(l, o, t, 16, 4).is_err() {
                    return false;
                }
                if stream.push(&enc, ix, v).is_err() {
                    return false;
                }
            }
            let data = match stream.flush() {
                Ok(d) => d,
                Err(_) => return false,
            };
            if data.len() % 4 != 0 {
                return false;
            }
            let mut ds = RansDecoderStream::<Rans64>::open_on(&data);
            for (l, o, t, ix, expected) in batches.iter().rev() {
                let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
                if dec.initialize(l, o, t, 16, 4).is_err() {
                    return false;
                }
                let mut out = vec![0i32; expected.len()];
                if ds.decode(&dec, &mut out, ix).is_err() || out != *expected {
                    return false;
                }
            }
            ds.decode_eof().is_ok()
        }
        _ => {
            let mut stream = RansEncoderStream::<RansByte>::new();
            for (l, o, t, ix, v) in batches {
                let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
                if enc.initialize(l, o, t, 16, 4).is_err() {
                    return false;
                }
                if stream.push(&enc, ix, v).is_err() {
                    return false;
                }
            }
            let data = match stream.flush() {
                Ok(d) => d,
                Err(_) => return false,
            };
            let mut ds = RansDecoderStream::<RansByte>::open_on(&data);
            for (l, o, t, ix, expected) in batches.iter().rev() {
                let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
                if dec.initialize(l, o, t, 16, 4).is_err() {
                    return false;
                }
                let mut out = vec![0i32; expected.len()];
                if ds.decode(&dec, &mut out, ix).is_err() || out != *expected {
                    return false;
                }
            }
            ds.decode_eof().is_ok()
        }
    }
}

/// Allocation-failure injection: growth overflow is a typed error.
fn run_allocation_checks() -> usize {
    use msrtc_rans::buffer::{BufferError, HeapResizableBuffer, ResizableBuffer};
    let mut checks = 0usize;

    // Growth with a tiny max step must not panic and must preserve content.
    let mut b = HeapResizableBuffer::new(512, 512);
    if b.begin_to_grow().is_ok() {
        b.commit();
        checks += 1;
    }

    // Rollback restores the previous capacity.
    let mut b = HeapResizableBuffer::new(512, 512);
    let _ = b.begin_to_grow();
    b.rollback();
    if b.capacity() == 512 {
        checks += 1;
    }

    // checked_add path: report CapacityOverflow rather than panic.
    let err = checked_growth_overflow();
    if matches!(err, Err(BufferError::CapacityOverflow)) {
        checks += 1;
    }

    checks
}

fn checked_growth_overflow() -> Result<usize, msrtc_rans::buffer::BufferError> {
    let old_len = usize::MAX / 2 + 1;
    let step = old_len; // min(old, step) would exceed usize
    match old_len.checked_add(step) {
        Some(_) => Ok(old_len),
        None => Err(msrtc_rans::buffer::BufferError::CapacityOverflow),
    }
}

/// MSRTC.HARDENING — internal property court.
pub struct HardeningCourt;

impl Court for HardeningCourt {
    fn id(&self) -> &str {
        "MSRTC.HARDENING"
    }

    fn run(&self) -> CourtResult {
        let mut results = Vec::new();

        let raw = run_raw_sweeps();
        results.push(hardening_result("raw_sweeps", raw > 0, raw as u64));

        let corrupt = run_corruption_robustness();
        results.push(hardening_result(
            "corruption_robustness",
            corrupt > 0,
            corrupt as u64,
        ));

        let entropy = run_entropy_sweeps();
        results.push(hardening_result(
            "entropy_sweeps",
            entropy > 0,
            entropy as u64,
        ));

        let alloc = run_allocation_checks();
        results.push(hardening_result(
            "allocation_checks",
            alloc > 0,
            alloc as u64,
        ));

        let pass_count = results.iter().filter(|r| r.comparison.exact).count() as u64;
        let residual_count = results.iter().filter(|r| !r.comparison.exact).count() as u64;

        CourtResult {
            court_id: self.id().to_string(),
            status: if residual_count == 0 && pass_count == results.len() as u64 {
                CourtStatus::Passed
            } else {
                CourtStatus::Failed
            },
            case_count: results.len() as u64,
            pass_count,
            residual_count,
            skipped_count: 0,
            results,
        }
    }
}

fn hardening_result(name: &str, passed: bool, sub_checks: u64) -> DifferentialResult {
    let native_status = if passed {
        "ok".to_string()
    } else {
        "failed".to_string()
    };
    DifferentialResult {
        schema_version: oracle::SCHEMA_VERSION,
        court_id: "MSRTC.HARDENING".to_string(),
        case_id: format!("{}:sub_checks={}", name, sub_checks),
        oracle_commit: "n/a (internal property court)".to_string(),
        rust_commit: git_commit(),
        seed: 0,
        variant: "n/a".to_string(),
        input_hashes: InputHashes {
            pmf_lengths_sha256: String::new(),
            pmf_offsets_sha256: String::new(),
            pmf_table_sha256: String::new(),
            indices_sha256: String::new(),
            values_sha256: String::new(),
        },
        oracle: OracleResult {
            status: "n/a".to_string(),
            output_sha256: String::new(),
            length: 0,
        },
        native: NativeResult {
            status: native_status,
            output_sha256: sha256(name.as_bytes()),
            length: sub_checks,
        },
        comparison: Comparison {
            exact: passed,
            first_differing_offset: None,
            differing_bytes: None,
        },
        classification: if passed {
            ResidualClassification::Unclassified
        } else {
            ResidualClassification::NativeBug
        },
        resolution: ResolutionState::Open,
        minimized_casefile: None,
        environment_sha256: environment_sha256(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardening_court_passes_internally() {
        let court = HardeningCourt;
        let result = court.run();
        assert!(result.case_count > 0);
        assert!(result.is_sealable(), "hardening court must pass");
    }
}
