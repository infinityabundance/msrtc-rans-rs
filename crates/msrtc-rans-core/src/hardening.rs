// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # Hardening tests — raw rANS primitives (Phase 8)
//!
//! Deterministic property tests and generation sweeps for the raw rANS
//! engine. No external RNG: everything is driven by a seeded xorshift/LCG
//! so failures are reproducible by construction.
//!
//! Coverage:
//! - scale-bits sweep roundtrips (2..=MAX for both variants)
//! - boundary frequency patterns (freq=1, freq=scale-start, halves)
//! - prepared-symbol vs raw-division byte equivalence across the sweep
//! - transactional decoder behaviour on truncated streams (no state
//!   mutation on failure)
//! - VecSink growth integrity across 1..=2000 writes

use crate::sink::VecSink;
use crate::source::SliceSource;
use crate::{
    Freq, Rans64Decoder, Rans64EncSymbol, Rans64Encoder, RansByteDecoder, RansByteEncSymbol,
    RansByteEncoder,
};

use alloc::vec;
use alloc::vec::Vec;

/// Deterministic xorshift64* generator (seeded; reproducible).
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// Uniform integer in [0, bound).
    fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }
        self.next() % bound
    }
}

/// Generate a deterministic sequence of valid (start, freq) symbols for a
/// given scale. Every symbol satisfies `0 < freq <= scale - start`.
fn gen_symbols(rng: &mut Lcg, scale: Freq, count: usize) -> Vec<(Freq, Freq)> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let start = rng.below(scale as u64) as Freq;
        // Exclude the degenerate full-scale symbol (freq == scale), which
        // overflows the encoder's x_max in both Rust and C++ (shift wrap).
        // Valid symbols satisfy 0 < freq <= scale - start AND freq < scale.
        let mut max_freq = scale - start;
        if max_freq == scale {
            max_freq -= 1;
        }
        let max_freq = max_freq.max(1);
        // Bias toward interesting values: 1, max, half, random.
        let freq = match rng.below(4) {
            0 => 1,
            1 => max_freq,
            2 => (max_freq / 2).max(1),
            _ => 1 + rng.below(max_freq as u64) as Freq,
        };
        let freq = freq.clamp(1, max_freq);
        out.push((start, freq));
    }
    out
}

fn roundtrip_byte(symbols: &[(Freq, Freq)], scale_bits: Freq) {
    let sink = VecSink::<u8>::new(64);
    let mut encoder = RansByteEncoder::new(sink);
    for &(start, freq) in symbols {
        encoder.put_raw(start, freq, scale_bits);
    }
    encoder.flush();
    let encoded = encoder.into_sink().encoded().to_vec();
    assert!(
        !encoded.is_empty(),
        "scale_bits={} symbols={}",
        scale_bits,
        symbols.len()
    );

    let source = SliceSource::new(&encoded[..]);
    let mut decoder = RansByteDecoder::new(source);
    assert!(decoder.init(), "init failed for scale_bits={}", scale_bits);

    // rANS decodes in REVERSE of the encode order.
    for &(start, freq) in symbols.iter().rev() {
        let got = decoder.get(scale_bits);
        assert!(
            got >= start && got < start + freq,
            "value out of symbol range at scale_bits={}: start={} freq={} got={}",
            scale_bits,
            start,
            freq,
            got
        );
        assert!(
            decoder.advance(start, freq, scale_bits),
            "advance failed at scale_bits={}",
            scale_bits
        );
    }
    assert!(
        decoder.check_eof(),
        "not at EOF for scale_bits={}",
        scale_bits
    );
}

fn roundtrip_64(symbols: &[(Freq, Freq)], scale_bits: Freq) {
    let sink = VecSink::<u32>::new(64);
    let mut encoder = Rans64Encoder::new(sink);
    for &(start, freq) in symbols {
        encoder.put_raw(start, freq, scale_bits);
    }
    encoder.flush();
    let units = encoder.into_sink().encoded().to_vec();
    assert!(!units.is_empty());

    let source = SliceSource::new(&units[..]);
    let mut decoder = Rans64Decoder::new(source);
    assert!(decoder.init());

    // rANS decodes in REVERSE of the encode order.
    for &(start, freq) in symbols.iter().rev() {
        let got = decoder.get(scale_bits);
        assert!(
            got >= start && got < start + freq,
            "value out of symbol range at scale_bits={}: start={} freq={} got={}",
            scale_bits,
            start,
            freq,
            got
        );
        assert!(decoder.advance(start, freq, scale_bits));
    }
    assert!(decoder.check_eof());
}

/// Prepared symbols must produce byte-identical output to raw division.
fn prepared_matches_raw_byte(symbols: &[(Freq, Freq)], scale_bits: Freq) {
    let sink = VecSink::<u8>::new(64);
    let mut raw = RansByteEncoder::new(sink);
    for &(start, freq) in symbols {
        raw.put_raw(start, freq, scale_bits);
    }
    raw.flush();
    let raw_out = raw.into_sink().encoded().to_vec();

    let sink2 = VecSink::<u8>::new(64);
    let mut prepared = RansByteEncoder::new(sink2);
    for &(start, freq) in symbols {
        let sym = RansByteEncSymbol::new(start, freq, scale_bits);
        prepared.put(&sym);
    }
    prepared.flush();
    let prep_out = prepared.into_sink().encoded().to_vec();

    assert_eq!(
        raw_out, prep_out,
        "prepared != raw at scale_bits={}",
        scale_bits
    );
}

fn prepared_matches_raw_64(symbols: &[(Freq, Freq)], scale_bits: Freq) {
    let sink = VecSink::<u32>::new(64);
    let mut raw = Rans64Encoder::new(sink);
    for &(start, freq) in symbols {
        raw.put_raw(start, freq, scale_bits);
    }
    raw.flush();
    let raw_out = raw.into_sink().encoded().to_vec();

    let sink2 = VecSink::<u32>::new(64);
    let mut prepared = Rans64Encoder::new(sink2);
    for &(start, freq) in symbols {
        let sym = Rans64EncSymbol::new(start, freq, scale_bits);
        prepared.put(&sym);
    }
    prepared.flush();
    let prep_out = prepared.into_sink().encoded().to_vec();

    assert_eq!(
        raw_out, prep_out,
        "prepared != raw at scale_bits={}",
        scale_bits
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RansByte: full operational scale-bits sweep.
    ///
    /// Domain: scale_bits <= 23. Above 23, `x_max = freq << (31 - scale_bits)`
    /// drops below 2^8 and the encoder's state can drain below LowerBound
    /// (2^23), producing a stream the decoder must reject. This is inherent
    /// to the rANS scheme and identical in the C++ oracle (StateBits=31).
    #[test]
    fn test_ransbyte_scale_bits_sweep_roundtrip() {
        let mut rng = Lcg::new(0x5EED_2026);
        for scale_bits in 2..=23u32 {
            let scale = 1u32 << scale_bits;
            let count = (8 + (scale_bits as usize) * 4).min(512);
            let symbols = gen_symbols(&mut rng, scale, count);
            roundtrip_byte(&symbols, scale_bits);
        }
    }

    /// Rans64: full operational scale-bits sweep (2..=31; 32 is a documented
    /// residual — `1u32 << 32` overflows).
    #[test]
    fn test_rans64_scale_bits_sweep_roundtrip() {
        let mut rng = Lcg::new(0x64_2026);
        for scale_bits in 2..=31u32 {
            let scale = 1u32 << scale_bits;
            let count = (8 + (scale_bits as usize) * 4).min(512);
            let symbols = gen_symbols(&mut rng, scale, count);
            roundtrip_64(&symbols, scale_bits);
        }
    }

    /// Boundary frequency patterns across the operational scale widths.
    #[test]
    fn test_boundary_freq_patterns() {
        let mut rng = Lcg::new(0xB0B);
        for scale_bits in [2u32, 3, 4, 8, 16, 23, 24, 31] {
            let scale = 1u32 << scale_bits;
            // RansByte is only operational up to scale_bits=23; above that,
            // only Rans64 (u64 state) is exercised.
            let byte_ok = scale_bits <= 23;
            let patterns: Vec<(Freq, Freq)> = vec![
                (0, 1),                       // freq=1 at start=0
                (0, scale - 1),               // near-full at 0
                (scale - 1, 1),               // start at top
                (scale / 2, scale / 2),       // halves
                (scale / 2, (scale / 2) - 1), // just under half
                (1, scale - 1),               // near-full from start=1
                (scale - 2, 2),               // two at top
                (0, scale - 2),               // two short of full
                (0, scale / 3),               // thirds
                (scale - scale / 3, scale / 3),
            ];
            let mut symbols = patterns.clone();
            // add random ones for length
            for _ in 0..32 {
                symbols.extend(gen_symbols(&mut rng, scale, 1));
            }
            if byte_ok {
                roundtrip_byte(&symbols, scale_bits);
            }
            roundtrip_64(&symbols, scale_bits);
        }
    }

    /// Prepared symbols == raw division across the operational sweep.
    #[test]
    fn test_prepared_matches_raw_sweep() {
        let mut rng = Lcg::new(0x7A7A);
        for scale_bits in [2u32, 3, 4, 8, 12, 16, 20, 23, 24, 31] {
            let scale = 1u32 << scale_bits;
            let symbols = gen_symbols(&mut rng, scale, 64);
            if scale_bits <= 23 {
                prepared_matches_raw_byte(&symbols, scale_bits);
            }
            prepared_matches_raw_64(&symbols, scale_bits);
        }
    }

    /// Truncating a valid stream at every offset must either succeed or
    /// fail cleanly; a failed `advance` must not mutate decoder state.
    #[test]
    fn test_truncated_stream_transactional() {
        let mut rng = Lcg::new(0xDEAD);
        let scale_bits = 16u32;
        let scale = 1u32 << scale_bits;
        let symbols = gen_symbols(&mut rng, scale, 64);

        // Full encode
        let sink = VecSink::<u8>::new(64);
        let mut encoder = RansByteEncoder::new(sink);
        for &(start, freq) in &symbols {
            encoder.put_raw(start, freq, scale_bits);
        }
        encoder.flush();
        let full = encoder.into_sink().encoded().to_vec();

        // Every truncation point: no panic, and state is preserved on failure.
        for cut in 0..full.len() {
            let data = &full[..cut];
            let source = SliceSource::new(data);
            let mut decoder = RansByteDecoder::new(source);
            if !decoder.init() {
                continue;
            }
            let mut ok = true;
            for &(start, freq) in symbols.iter().rev() {
                let _ = decoder.get(scale_bits);
                let before = decoder.state();
                if !decoder.advance(start, freq, scale_bits) {
                    assert_eq!(
                        decoder.state(),
                        before,
                        "advance failed but mutated state at cut={}",
                        cut
                    );
                    ok = false;
                    break;
                }
            }
            if ok {
                // Only the full stream must decode everything.
                assert_eq!(cut, full.len(), "short stream decoded fully at cut={}", cut);
            }
        }
    }

    /// VecSink growth integrity: 1..=2000 writes preserve every byte.
    #[test]
    fn test_vecsink_growth_sweep() {
        for n in 1..=2000usize {
            let sink = VecSink::<u8>::new(8);
            let mut encoder = RansByteEncoder::new(sink);
            for i in 0..n {
                // Valid single-symbol scale-8 raw puts
                let start = (i % 200) as Freq;
                let freq = 1u32; // freq=1 is always valid
                encoder.put_raw(start, freq, 8);
            }
            encoder.flush();
            let encoded = encoder.into_sink().encoded().to_vec();

            let source = SliceSource::new(&encoded[..]);
            let mut decoder = RansByteDecoder::new(source);
            assert!(decoder.init(), "init failed for n={}", n);
            // rANS decodes in REVERSE of the encode order.
            for i in (0..n).rev() {
                let start = (i % 200) as Freq;
                let got = decoder.get(8);
                assert_eq!(got, start, "freq=1 value must equal start: n={} i={}", n, i);
                assert!(
                    decoder.advance(start, 1, 8),
                    "advance failed n={} i={}",
                    n,
                    i
                );
            }
            assert!(decoder.check_eof(), "eof failed n={}", n);
        }
    }

    /// Raw decoder corrupt-stream robustness: bit-flipped and byte-shuffled
    /// streams must never panic — every `advance` either succeeds or fails
    /// transactionally (state preserved on failure).
    #[test]
    fn test_raw_decoder_no_panic_on_corruption() {
        let mut rng = Lcg::new(0xC0FFEE);
        let scale_bits = 16u32;
        let scale = 1u32 << scale_bits;
        let symbols = gen_symbols(&mut rng, scale, 32);

        let sink = VecSink::<u8>::new(64);
        let mut encoder = RansByteEncoder::new(sink);
        for &(start, freq) in &symbols {
            encoder.put_raw(start, freq, scale_bits);
        }
        encoder.flush();
        let full = encoder.into_sink().encoded().to_vec();

        // Corrupt every byte in turn; decoder must never panic.
        for flip in 0..full.len() {
            let mut data = full.clone();
            data[flip] ^= 0x5A;
            let source = SliceSource::new(&data[..]);
            let mut decoder = RansByteDecoder::new(source);
            if !decoder.init() {
                continue;
            }
            for &(start, freq) in symbols.iter().rev() {
                let _ = decoder.get(scale_bits);
                let before = decoder.state();
                if !decoder.advance(start, freq, scale_bits) {
                    assert_eq!(decoder.state(), before, "state mutated on failure");
                    break;
                }
            }
        }
    }

    /// try_advance / try_get invalid-parameter branches (regression).
    #[test]
    fn test_try_advance_invalid_parameters() {
        use crate::error::RawRansError;

        // Build a valid stream so init() succeeds.
        let sink = VecSink::<u8>::new(64);
        let mut encoder = RansByteEncoder::new(sink);
        encoder.put_raw(0, 128, 8);
        encoder.flush();
        let encoded = encoder.into_sink().encoded().to_vec();

        let source = SliceSource::new(&encoded[..]);
        let mut decoder = RansByteDecoder::new(source);
        assert!(decoder.init());

        // start >= scale
        assert!(matches!(
            decoder.try_advance(256, 1, 8),
            Err(RawRansError::InvalidParameters)
        ));
        // freq == 0
        assert!(matches!(
            decoder.try_advance(0, 0, 8),
            Err(RawRansError::InvalidParameters)
        ));
        // freq > scale - start
        assert!(matches!(
            decoder.try_advance(255, 2, 8),
            Err(RawRansError::InvalidParameters)
        ));
        // scale_bits out of range
        assert!(matches!(
            decoder.try_advance(0, 1, 32),
            Err(RawRansError::InvalidScaleBits { .. })
        ));
        assert!(matches!(
            decoder.try_advance(0, 1, 1),
            Err(RawRansError::InvalidScaleBits { .. })
        ));
    }
}
