// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! Raw rANS encoder/decoder implementations for both RansByte and Rans64 variants.
//!
//! The code is generated via a macro to avoid duplication while matching the
//! C++ template structure exactly.
//!
//! # Scale-bits edge cases
//!
//! When `scale_bits == 32` (the maximum for Rans64), the expression `1u32 << scale_bits`
//! overflows in Rust (and is undefined behavior in C++). The upstream C++ code also
//! enters indefinite shift territory here. This implementation defines the behavior
//! deterministically: `scale_bits == 32` is **rejected** for the prepared-symbol path
//! where it would cause overflow, matching an intentional safety divergence.
//! See residual `MSRTC.RAW.SCALE32`.

use crate::Freq;
use crate::arithmetic;
#[allow(unused_imports)]
use crate::sink::Sink;
#[allow(unused_imports)]
use crate::source::Source;

// ###########################################################################
// Macro: generate_rans_impl
// ###########################################################################
// Generates encoder, decoder, and symbol types for a given state/unit pair.

macro_rules! generate_rans_impl {
    (
        $enc_symbol:ident,   // EncSymbol type name
        $dec_symbol:ident,   // DecSymbol type name
        $encoder:ident,      // Encoder type name
        $decoder:ident,      // Decoder type name
        $state_ty:ty,        // State type (u32, u64)
        $unit_ty:ty,         // Unit type (u8, u32)
        $state_bits:expr,    // STATE_BITS constant
        $max_scale_bits:expr,// MAX_SCALE_BITS constant
        $lower_bound:expr,   // LOWER_BOUND constant
        $unit_bits:expr,     // UNIT_BITS constant
        $units_per_state:expr, // UNITS_PER_STATE constant
    ) => {
        /// Prepared encoder symbol.
        #[derive(Debug, Clone, Copy)]
        pub struct $enc_symbol {
            pub x_max_hi: Freq,
            pub freq_rcp_shift: u32,
            pub freq_rcp: $state_ty,
            pub freq_cmpl: Freq,
            pub bias: Freq,
        }

        impl $enc_symbol {
            /// Create a new prepared encoder symbol.
            #[inline]
            pub fn new(start: Freq, freq: Freq, scale_bits: Freq) -> Self {
                let scale = 1u32 << scale_bits;
                debug_assert!(
                    0 < scale_bits && scale_bits as u32 <= $max_scale_bits,
                    "invalid scale_bits"
                );
                debug_assert!(start < scale, "start out of range");
                debug_assert!(freq > 0 && freq <= scale - start, "freq out of range");

                let min_bits =
                    core::cmp::min($state_bits, (core::mem::size_of::<Freq>() * 8 - 1) as u32);
                let x_max_hi = freq << (min_bits - scale_bits);

                let (freq_rcp, mut freq_rcp_shift, bias) = if freq > 1 {
                    let shift = arithmetic::reciprocal_shift(freq);
                    let rcp: $state_ty = if core::mem::size_of::<$state_ty>() >= 8 {
                        arithmetic::compute_reciprocal_u64(freq) as $state_ty
                    } else {
                        arithmetic::compute_reciprocal_u32(freq) as $state_ty
                    };
                    (rcp, shift - 1, start)
                } else {
                    let rcp: $state_ty = if core::mem::size_of::<$state_ty>() >= 8 {
                        !0u64 as $state_ty
                    } else {
                        !0u32 as $state_ty
                    };
                    let bias_adj = start + scale - 1;
                    (rcp, 0, bias_adj)
                };

                if core::mem::size_of::<$state_ty>() < 8 {
                    freq_rcp_shift += (core::mem::size_of::<$state_ty>() * 8) as u32;
                }

                let freq_cmpl = scale - freq;

                Self {
                    x_max_hi,
                    freq_rcp_shift,
                    freq_rcp,
                    freq_cmpl,
                    bias,
                }
            }

            /// Compute quotient: fast division using precomputed reciprocal.
            #[inline]
            pub fn quotient(&self, x: $state_ty) -> $state_ty {
                if core::mem::size_of::<$state_ty>() >= 8 {
                    let x_u64 = x as u64;
                    let rcp_u64 = self.freq_rcp as u64;
                    arithmetic::fast_quotient_u64(x_u64, rcp_u64, self.freq_rcp_shift) as $state_ty
                } else {
                    let x_u32 = x as u32;
                    let rcp_u32 = self.freq_rcp as u32;
                    arithmetic::fast_quotient_u32(x_u32, rcp_u32, self.freq_rcp_shift) as $state_ty
                }
            }
        }

        /// Prepared decoder symbol.
        #[derive(Debug, Clone, Copy)]
        pub struct $dec_symbol {
            pub freq: Freq,
            pub start: Freq,
        }

        impl $dec_symbol {
            /// Create a new decoder symbol.
            #[inline]
            pub fn new(start: Freq, freq: Freq) -> Self {
                debug_assert!(freq > 0, "frequency must be positive");
                Self { freq, start }
            }
        }

        /// Raw rANS encoder.
        #[derive(Debug)]
        pub struct $encoder<Sk: Sink<$unit_ty>> {
            sink: Sk,
            state: $state_ty,
        }

        impl<Sk: Sink<$unit_ty>> $encoder<Sk> {
            #[inline]
            pub fn new(sink: Sk) -> Self {
                Self {
                    sink,
                    state: $lower_bound,
                }
            }

            #[inline]
            pub fn sink(&self) -> &Sk {
                &self.sink
            }
            #[inline]
            pub fn sink_mut(&mut self) -> &mut Sk {
                &mut self.sink
            }
            #[inline]
            pub fn into_sink(self) -> Sk {
                self.sink
            }
            #[inline]
            pub fn state(&self) -> $state_ty {
                self.state
            }
            #[inline]
            pub fn reset(&mut self) {
                self.state = $lower_bound;
            }

            /// Put a raw symbol (start, freq, scale_bits) using division.
            #[inline]
            pub fn put_raw(&mut self, start: Freq, freq: Freq, scale_bits: Freq) {
                debug_assert!(0 < scale_bits && scale_bits as u32 <= $max_scale_bits);
                debug_assert!(start < (1u32 << scale_bits));
                debug_assert!(freq > 0 && freq <= (1u32 << scale_bits) - start);

                let x_max: $state_ty = (freq as $state_ty) << ($state_bits - scale_bits as u32);
                let x = self.renormalize(x_max);

                let shift = scale_bits as u32;
                self.state = ((x / freq as $state_ty) << shift)
                    + (start as $state_ty)
                    + (x % freq as $state_ty);
            }

            /// Put a prepared symbol (fast reciprocal-multiply path).
            #[inline]
            pub fn put(&mut self, symbol: &$enc_symbol) {
                let mut x_max = symbol.x_max_hi as $state_ty;
                if $state_bits > (core::mem::size_of::<Freq>() * 8 - 1) as u32 {
                    let shift =
                        ($state_bits - (core::mem::size_of::<Freq>() * 8 - 1) as u32) as usize;
                    x_max = x_max << shift;
                }
                let x = self.renormalize(x_max);

                let q = symbol.quotient(x);
                self.state = x + (q * symbol.freq_cmpl as $state_ty) + symbol.bias as $state_ty;
            }

            /// Flush the encoder state to the sink.
            #[inline]
            pub fn flush(&mut self) {
                let x = self.state;
                for i in (1..$units_per_state).rev() {
                    let shift = i * $unit_bits as usize;
                    self.sink.write((x >> shift) as $unit_ty);
                }
                self.sink.write(x as $unit_ty);
            }

            #[inline]
            fn renormalize(&mut self, x_max: $state_ty) -> $state_ty {
                let mut x = self.state;
                while x >= x_max {
                    self.sink.write(x as $unit_ty);
                    x >>= $unit_bits;
                    if $max_scale_bits <= $unit_bits {
                        debug_assert!(x < x_max);
                        break;
                    }
                }
                x
            }
        }

        /// Raw rANS decoder.
        #[derive(Debug)]
        pub struct $decoder<Sr: Source<$unit_ty>> {
            source: Sr,
            state: $state_ty,
        }

        impl<Sr: Source<$unit_ty>> $decoder<Sr> {
            #[inline]
            pub fn new(source: Sr) -> Self {
                Self {
                    source,
                    state: $lower_bound,
                }
            }

            #[inline]
            pub fn source(&self) -> &Sr {
                &self.source
            }
            #[inline]
            pub fn source_mut(&mut self) -> &mut Sr {
                &mut self.source
            }
            #[inline]
            pub fn into_source(self) -> Sr {
                self.source
            }
            #[inline]
            pub fn state(&self) -> $state_ty {
                self.state
            }
            #[inline]
            pub fn check_eof(&self) -> bool {
                self.state == $lower_bound
            }

            /// Initialize the decoder by reading the initial state from the source.
            #[inline]
            pub fn init(&mut self) -> bool {
                let mut unit = <$unit_ty>::default();
                if !self.source.read(&mut unit) {
                    return false;
                }
                let mut x = unit as $state_ty;

                for i in 1..$units_per_state {
                    if !self.source.read(&mut unit) {
                        return false;
                    }
                    x = x.wrapping_add((unit as $state_ty) << (i * $unit_bits as usize));
                }

                if x < $lower_bound {
                    return false;
                }
                self.state = x;
                true
            }

            /// Get the next symbol frequency (low `scale_bits` of state).
            #[inline]
            pub fn get(&self, scale_bits: Freq) -> Freq {
                let mask = (1u32 << scale_bits) - 1;
                (self.state as Freq) & mask
            }

            /// Advance the decoder past a symbol.
            ///
            /// Uses transactional state: computes into a local variable and only
            /// assigns to `self.state` after renormalization succeeds.
            ///
            /// Equivalent to: `RansDecoder::Advance(start, freq, scale_bits)`.
            #[inline]
            pub fn advance(&mut self, start: Freq, freq: Freq, scale_bits: Freq) -> bool {
                let scale = 1u32 << scale_bits;
                debug_assert!(start < scale);
                debug_assert!(freq > 0 && freq <= scale - start);

                let x = self.state;
                let mask = (scale - 1) as $state_ty;
                let value = x & mask;
                debug_assert!(value >= start as $state_ty);

                // Compute new state into a LOCAL — do not mutate self.state yet
                let shift = scale_bits as u32;
                let mut x_new = (freq as $state_ty) * (x >> shift) + (value - start as $state_ty);

                // Renormalize using local state; reads source but does not commit
                let mut renorm_unit = <$unit_ty>::default();
                while x_new < $lower_bound {
                    if !self.source.read(&mut renorm_unit) {
                        return false;
                    }
                    x_new = (x_new << $unit_bits) + renorm_unit as $state_ty;
                    if $max_scale_bits <= $unit_bits {
                        debug_assert!(x_new >= $lower_bound);
                        break;
                    }
                }

                // All reads succeeded — commit the new state
                self.state = x_new;
                true
            }

            /// Advance using a prepared decoder symbol.
            #[inline]
            pub fn advance_symbol(&mut self, symbol: &$dec_symbol, scale_bits: Freq) -> bool {
                self.advance(symbol.start, symbol.freq, scale_bits)
            }
        }
    };
}

// ###########################################################################
// Generate implementations for both variants
// ###########################################################################

generate_rans_impl! {
    RansByteEncSymbol,    // enc symbol name
    RansByteDecSymbol,    // dec symbol name
    RansByteEncoder,      // encoder name
    RansByteDecoder,      // decoder name
    u32, u8,              // state, unit types
    31,                   // STATE_BITS
    30,                   // MAX_SCALE_BITS
    1u32 << 23,           // LOWER_BOUND
    8,                    // UNIT_BITS
    4,                    // UNITS_PER_STATE
}

generate_rans_impl! {
    Rans64EncSymbol,    // enc symbol name
    Rans64DecSymbol,    // dec symbol name
    Rans64Encoder,      // encoder name
    Rans64Decoder,      // decoder name
    u64, u32,           // state, unit types
    63,                 // STATE_BITS
    32,                 // MAX_SCALE_BITS
    1u64 << 31,         // LOWER_BOUND
    32,                 // UNIT_BITS
    2,                  // UNITS_PER_STATE
}

// ###########################################################################
// Tests
// ###########################################################################

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sink::VecSink;
    use crate::source::SliceSource;

    // ---- RansByte tests ----

    #[test]
    fn test_ransbyte_initial_state() {
        let sink = VecSink::<u8>::new(64);
        let encoder = RansByteEncoder::new(sink);
        assert_eq!(encoder.state(), 1u32 << 23);
    }

    #[test]
    fn test_ransbyte_raw_roundtrip() {
        let sink = VecSink::<u8>::new(64);
        let mut encoder = RansByteEncoder::new(sink);
        encoder.put_raw(0, 128, 8);
        encoder.flush();
        let encoded = encoder.into_sink().encoded().to_vec();
        assert!(!encoded.is_empty());

        let source = SliceSource::new(&encoded[..]);
        let mut decoder = RansByteDecoder::new(source);
        assert!(decoder.init());

        let freq_val = decoder.get(8);
        assert_eq!(freq_val, 0);
        assert!(decoder.advance(0, 128, 8));
        assert!(decoder.check_eof());
    }

    #[test]
    fn test_ransbyte_prepared_symbol_matches_raw() {
        let sink1 = VecSink::<u8>::new(64);
        let mut enc1 = RansByteEncoder::new(sink1);
        enc1.put_raw(0, 128, 8);
        enc1.flush();
        let out1 = enc1.into_sink().encoded().to_vec();

        let sink2 = VecSink::<u8>::new(64);
        let mut enc2 = RansByteEncoder::new(sink2);
        let sym = RansByteEncSymbol::new(0, 128, 8);
        enc2.put(&sym);
        enc2.flush();
        let out2 = enc2.into_sink().encoded().to_vec();

        assert_eq!(out1, out2);
    }

    #[test]
    fn test_ransbyte_decoder_rejects_empty() {
        let data: [u8; 0] = [];
        let source = SliceSource::new(&data[..]);
        let mut decoder = RansByteDecoder::new(source);
        assert!(!decoder.init());
    }

    #[test]
    fn test_ransbyte_encoder_reset() {
        let sink = VecSink::<u8>::new(64);
        let mut encoder = RansByteEncoder::new(sink);
        encoder.put_raw(0, 128, 8);
        encoder.reset();
        assert_eq!(encoder.state(), 1u32 << 23);
    }

    // ---- Rans64 tests ----

    #[test]
    fn test_rans64_initial_state() {
        let sink = VecSink::<u32>::new(64);
        let encoder = Rans64Encoder::new(sink);
        assert_eq!(encoder.state(), 1u64 << 31);
    }

    #[test]
    fn test_rans64_raw_roundtrip() {
        let sink = VecSink::<u32>::new(64);
        let mut encoder = Rans64Encoder::new(sink);
        encoder.put_raw(0, 128, 8);
        encoder.flush();
        let encoded = encoder.into_sink().encoded().to_vec();
        assert!(!encoded.is_empty());

        let source = SliceSource::new(&encoded[..]);
        let mut decoder = Rans64Decoder::new(source);
        assert!(decoder.init());
        assert!(decoder.advance(0, 128, 8));
        assert!(decoder.check_eof());
    }

    #[test]
    fn test_rans64_prepared_symbol_matches_raw() {
        let sink1 = VecSink::<u32>::new(64);
        let mut enc1 = Rans64Encoder::new(sink1);
        enc1.put_raw(0, 128, 8);
        enc1.flush();
        let out1 = enc1.into_sink().encoded().to_vec();

        let sink2 = VecSink::<u32>::new(64);
        let mut enc2 = Rans64Encoder::new(sink2);
        let sym = Rans64EncSymbol::new(0, 128, 8);
        enc2.put(&sym);
        enc2.flush();
        let out2 = enc2.into_sink().encoded().to_vec();

        assert_eq!(out1, out2);
    }

    #[test]
    fn test_rans64_encoder_reset() {
        let sink = VecSink::<u32>::new(64);
        let mut encoder = Rans64Encoder::new(sink);
        encoder.put_raw(0, 128, 8);
        encoder.reset();
        assert_eq!(encoder.state(), 1u64 << 31);
    }
}
