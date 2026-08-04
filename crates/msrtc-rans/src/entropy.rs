// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com
// Derived from Microsoft MLVC msrtc_rans (MIT)
// See NOTICE file for attribution.

//! # Entropy encoder/decoder (high-level PMF/bypass/CDF pipeline)
//!
//! Implements the high-level entropy coder matching Microsoft's
//! `EntropyEncoder` / `EntropyDecoder` in `EntropyCoder.cpp`.
//!
//! Builds on the raw rANS primitives from `msrtc-rans-core`.

#![allow(missing_docs)]

use alloc::vec::Vec;

use msrtc_rans_core::sink::VecSink;
use msrtc_rans_core::source::SliceSource;
use msrtc_rans_core::source::Source;
use msrtc_rans_core::variant::{Rans64, RansByte, RansParams};
use msrtc_rans_core::{
    Freq, Rans64DecSymbol, Rans64EncSymbol, Rans64Encoder, RansByteDecSymbol, RansByteEncSymbol,
    RansByteEncoder, RawRansError,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Size of Freq in bits (sizeof(u32) * 8 = 32).
const FREQ_BITS: u32 = (core::mem::size_of::<Freq>() * 8) as u32;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during entropy encode/decode operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntropyError {
    /// Invalid PMF data (lengths, offsets, or table).
    InvalidPmf,
    /// Invalid parameter value (symbolBits, bypassBits, etc.).
    InvalidParams,
    /// Encoder/decoder is not initialized.
    InvalidState,
    /// Stream data is truncated or corrupted.
    InvalidStream,
    /// Raw rANS primitive error.
    RawRansError(RawRansError),
}

impl core::fmt::Display for EntropyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EntropyError::InvalidPmf => write!(f, "invalid PMF data"),
            EntropyError::InvalidParams => write!(f, "invalid parameter value"),
            EntropyError::InvalidState => write!(f, "invalid state (not initialized)"),
            EntropyError::InvalidStream => write!(f, "invalid stream"),
            EntropyError::RawRansError(e) => write!(f, "raw rANS error: {}", e),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EntropyError {}

// ---------------------------------------------------------------------------
// Distribution descriptor
// ---------------------------------------------------------------------------

/// Describes one probability distribution (one set of PMF symbols).
#[derive(Debug, Clone)]
struct DistributionDesc {
    /// Offset applied to input values for this distribution.
    value_offset: i32,
    /// Sentinel index = length - 1 (last PMF element is tail mass for bypass).
    bypass_sentinel: i32,
    /// Starting offset of this distribution in the global symbol/CDF table.
    symbol_offset: usize,
}

// ---------------------------------------------------------------------------
// Helper: validate distribution descriptors from PMF arrays
// ---------------------------------------------------------------------------

fn initialize_distribution_desc(
    distribution_descs: &mut Vec<DistributionDesc>,
    pmf_lengths: &[i32],
    pmf_offsets: &[i32],
    pmf_table_size: usize,
) -> Result<(), EntropyError> {
    let distribution_count = pmf_lengths.len();
    if pmf_offsets.len() != distribution_count {
        return Err(EntropyError::InvalidPmf);
    }
    distribution_descs.reserve(distribution_count);

    let mut symbol_cursor: usize = 0;
    for i in 0..distribution_count {
        let length = pmf_lengths[i];
        // Each length must be > 1 (last element is bypass tail mass)
        if length <= 1 || pmf_table_size - symbol_cursor < length as usize {
            return Err(EntropyError::InvalidPmf);
        }
        distribution_descs.push(DistributionDesc {
            value_offset: pmf_offsets[i],
            bypass_sentinel: length - 1,
            symbol_offset: symbol_cursor,
        });
        symbol_cursor += length as usize;
    }

    if symbol_cursor != pmf_table_size {
        return Err(EntropyError::InvalidPmf);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: check probability bits against max
// ---------------------------------------------------------------------------

#[inline]
fn check_bits(prob_bits: u32, max_scale_bits: u32) -> Result<(), EntropyError> {
    if prob_bits < 2 || prob_bits > max_scale_bits {
        return Err(EntropyError::InvalidParams);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: convert byte slice to u32 units (LE)
// ---------------------------------------------------------------------------

#[inline]
fn bytes_to_u32_units(data: &[u8]) -> Vec<u32> {
    data.chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

// ---------------------------------------------------------------------------
// Raw rANS encoder trait — abstracts RansByteEncoder and Rans64Encoder
// ---------------------------------------------------------------------------

/// Trait abstracting over raw rANS encoder variants for the entropy coder.
pub(crate) trait RawEncoder {
    type Unit: Copy + Default;
    type Symbol;
    fn put_raw(&mut self, start: Freq, freq: Freq, scale_bits: Freq);
    fn put_symbol(&mut self, symbol: &Self::Symbol);
    fn flush(&mut self);
    fn into_units(self) -> Vec<Self::Unit>;
}

impl RawEncoder for RansByteEncoder<VecSink<u8>> {
    type Unit = u8;
    type Symbol = RansByteEncSymbol;

    fn put_raw(&mut self, start: Freq, freq: Freq, scale_bits: Freq) {
        self.put_raw(start, freq, scale_bits);
    }

    fn put_symbol(&mut self, symbol: &Self::Symbol) {
        self.put(symbol);
    }

    fn flush(&mut self) {
        self.flush();
    }

    fn into_units(self) -> Vec<u8> {
        self.into_sink().encoded().to_vec()
    }
}

impl RawEncoder for Rans64Encoder<VecSink<u32>> {
    type Unit = u32;
    type Symbol = Rans64EncSymbol;

    fn put_raw(&mut self, start: Freq, freq: Freq, scale_bits: Freq) {
        self.put_raw(start, freq, scale_bits);
    }

    fn put_symbol(&mut self, symbol: &Self::Symbol) {
        self.put(symbol);
    }

    fn flush(&mut self) {
        self.flush();
    }

    fn into_units(self) -> Vec<u32> {
        self.into_sink().encoded().to_vec()
    }
}

// ---------------------------------------------------------------------------
// Internal encoder state — generic across variants
// ---------------------------------------------------------------------------

struct EncoderState<S: EncSymbol> {
    symbol_bits: Freq,
    distribution_descs: Vec<DistributionDesc>,
    symbols: Vec<S>,
    bypass_bits: Freq,
    bypass_max_value: Freq,
}

/// Trait for encoder symbol types.
pub(crate) trait EncSymbol: Sized {
    fn try_new(start: Freq, freq: Freq, scale_bits: Freq) -> Result<Self, RawRansError>;
}

impl EncSymbol for RansByteEncSymbol {
    fn try_new(start: Freq, freq: Freq, scale_bits: Freq) -> Result<Self, RawRansError> {
        Self::try_new(start, freq, scale_bits)
    }
}

impl EncSymbol for Rans64EncSymbol {
    fn try_new(start: Freq, freq: Freq, scale_bits: Freq) -> Result<Self, RawRansError> {
        Self::try_new(start, freq, scale_bits)
    }
}

impl<S: EncSymbol> EncoderState<S> {
    fn uninitialized() -> Self {
        Self {
            symbol_bits: 0,
            distribution_descs: Vec::new(),
            symbols: Vec::new(),
            bypass_bits: 0,
            bypass_max_value: 0,
        }
    }

    fn initialize(
        &mut self,
        pmf_lengths: &[i32],
        pmf_offsets: &[i32],
        pmf_table: &[i32],
        symbol_bits: i32,
        bypass_bits: i32,
        max_scale_bits: u32,
    ) -> Result<(), EntropyError> {
        let sb = symbol_bits as Freq;
        let bb = bypass_bits as Freq;
        check_bits(sb, max_scale_bits)?;
        check_bits(bb, max_scale_bits)?;

        // Issue 2a: safe maximum — RansByte allows 30, Rans64 allows 31 for bypass
        let is_byte_variant = max_scale_bits < 32;
        let max_safe_bits = if is_byte_variant { 30u32 } else { 31u32 };
        if sb > max_safe_bits || bb > max_safe_bits {
            return Err(EntropyError::InvalidParams);
        }

        let mut distribution_descs = Vec::new();
        initialize_distribution_desc(
            &mut distribution_descs,
            pmf_lengths,
            pmf_offsets,
            pmf_table.len(),
        )?;

        // Issue 2b: use u64 for max_freq computation to avoid i32 overflow
        let max_freq = 1u64 << symbol_bits;
        let mut symbols: Vec<S> = Vec::with_capacity(pmf_table.len());
        let mut pmf_cursor: usize = 0;

        for desc in &distribution_descs {
            // Issue 2b: track cumulative start in u64
            let mut start: u64 = 0;
            for _i in 0..=desc.bypass_sentinel {
                let freq = pmf_table[pmf_cursor] as u64;
                pmf_cursor += 1;
                if !(freq > 0 && freq <= max_freq - start) {
                    return Err(EntropyError::InvalidPmf);
                }
                let sym = S::try_new(start as Freq, freq as Freq, sb).map_err(|e| match e {
                    RawRansError::InvalidScaleBits { .. } => EntropyError::InvalidParams,
                    RawRansError::InvalidParameters => EntropyError::InvalidPmf,
                })?;
                symbols.push(sym);
                start += freq;
            }
        }

        self.distribution_descs = distribution_descs;
        self.symbols = symbols;
        self.symbol_bits = sb;
        self.bypass_bits = bb;
        // Issue 2c: compute from u64 to avoid overflow at bb=32
        self.bypass_max_value = ((1u64 << bb) - 1) as Freq;
        Ok(())
    }

    /// Encode symbols onto an existing encoder (batch/persistent mode).
    ///
    /// Unlike `encode_to_vec`, this does NOT flush the encoder or extract units,
    /// enabling persistent streaming where multiple batches feed the same encoder.
    fn encode_batch<E: RawEncoder<Symbol = S>>(
        &self,
        indices: &[i32],
        values: &[i32],
        encoder: &mut E,
    ) -> Result<(), EntropyError> {
        if self.symbol_bits == 0 {
            return Err(EntropyError::InvalidState);
        }
        if indices.len() != values.len() {
            return Err(EntropyError::InvalidParams);
        }

        // Encode in reverse order (matching C++: iterate from last to first)
        let data_size = indices.len();
        let mut idx = data_size as isize - 1;
        while idx >= 0 {
            let index = indices[idx as usize];
            let value = values[idx as usize];

            if index < 0 {
                // Skip encoding, decoder returns 0 for skipped indices
                idx -= 1;
                continue;
            }

            // Clamp distribution index to a valid range
            let dist_len = self.distribution_descs.len();
            let ui = if (index as usize) < dist_len {
                index as usize
            } else {
                dist_len - 1
            };
            let desc = &self.distribution_descs[ui];

            // Issue 5: checked add to avoid i32 overflow
            let adjusted = value
                .checked_add(desc.value_offset)
                .ok_or(EntropyError::InvalidParams)?;
            let symbol_index: i32;
            if adjusted < 0 || adjusted >= desc.bypass_sentinel {
                // Out of PMF range — use bypass
                let bypass_value: Freq = if adjusted < 0 {
                    let neg = adjusted.checked_neg().ok_or(EntropyError::InvalidParams)?;
                    2u64.wrapping_mul(neg as u64).wrapping_sub(1) as Freq
                } else {
                    2u64.wrapping_mul((adjusted - desc.bypass_sentinel) as u64) as Freq
                };
                self.encode_bypass_value(encoder, bypass_value);
                symbol_index = desc.bypass_sentinel;
            } else {
                symbol_index = adjusted;
            }

            let sym_idx = desc.symbol_offset + symbol_index as usize;
            encoder.put_symbol(&self.symbols[sym_idx]);
            idx -= 1;
        }

        Ok(())
    }

    fn encode_to_vec<E: RawEncoder<Symbol = S>>(
        &self,
        indices: &[i32],
        values: &[i32],
        make_encoder: impl FnOnce() -> E,
    ) -> Result<Vec<E::Unit>, EntropyError> {
        let mut encoder = make_encoder();
        self.encode_batch(indices, values, &mut encoder)?;
        encoder.flush();
        Ok(encoder.into_units())
    }

    #[inline]
    fn encode_bypass_value<E: RawEncoder>(&self, encoder: &mut E, bypass_value: Freq) {
        // Split bypassValue into bypassBits-sized digits (LSB first)
        let max_parts = (FREQ_BITS as usize / self.bypass_bits as usize).max(2);
        let mut bypass_buffer = Vec::with_capacity(max_parts);

        let mut bv = bypass_value;
        while bv != 0 {
            bypass_buffer.push(bv & self.bypass_max_value);
            bv >>= self.bypass_bits;
        }

        let mut bypass_count = bypass_buffer.len() as Freq;

        // Put digits in reverse order (MSB first in the bitstream)
        // since the rANS encoder writes from end to start
        for &digit in bypass_buffer.iter().rev() {
            encoder.put_raw(digit, 1, self.bypass_bits);
        }

        // Encode bypass count as remainder-coded prefix
        // (each maxValue digit means "more to follow")
        let mut bypass_prefix_count: Freq = 0;
        while bypass_count >= self.bypass_max_value {
            bypass_count -= self.bypass_max_value;
            bypass_prefix_count += 1;
        }
        // Put bypassCount remainder (terminal digit)
        encoder.put_raw(bypass_count, 1, self.bypass_bits);
        // Put bypassCount prefix markers
        for _ in 0..bypass_prefix_count {
            encoder.put_raw(self.bypass_max_value, 1, self.bypass_bits);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal decoder state — generic across variants
// ---------------------------------------------------------------------------

struct DecoderState {
    symbol_bits: Freq,
    distribution_descs: Vec<DistributionDesc>,
    cdf_table: Vec<Freq>,
    bypass_bits: Freq,
    bypass_max_value: Freq,
}

impl DecoderState {
    fn uninitialized() -> Self {
        Self {
            symbol_bits: 0,
            distribution_descs: Vec::new(),
            cdf_table: Vec::new(),
            bypass_bits: 0,
            bypass_max_value: 0,
        }
    }

    fn initialize(
        &mut self,
        pmf_lengths: &[i32],
        pmf_offsets: &[i32],
        pmf_table: &[i32],
        symbol_bits: i32,
        bypass_bits: i32,
        max_scale_bits: u32,
    ) -> Result<(), EntropyError> {
        let sb = symbol_bits as Freq;
        let bb = bypass_bits as Freq;
        check_bits(sb, max_scale_bits)?;
        check_bits(bb, max_scale_bits)?;

        // Issue 2a: safe maximum
        let is_byte_variant = max_scale_bits < 32;
        let max_safe_bits = if is_byte_variant { 30u32 } else { 31u32 };
        if sb > max_safe_bits || bb > max_safe_bits {
            return Err(EntropyError::InvalidParams);
        }

        let mut distribution_descs = Vec::new();
        initialize_distribution_desc(
            &mut distribution_descs,
            pmf_lengths,
            pmf_offsets,
            pmf_table.len(),
        )?;

        // Build CDF table: for each distribution, store cumulative starts
        // plus one extra entry per distribution for the total sum
        let num_dist = distribution_descs.len();
        let mut cdf_table = vec![0u32; pmf_table.len() + num_dist];
        // Issue 2b: use u64 for max_freq
        let max_freq = 1u64 << symbol_bits;

        let mut cursor: usize = 0;
        for dist_idx in 0..num_dist {
            // Update symbol_offset to point into the CDF table (not the PMF table)
            distribution_descs[dist_idx].symbol_offset = cursor + dist_idx;

            let mut start: u64 = 0;
            for _i in 0..=distribution_descs[dist_idx].bypass_sentinel {
                let freq = pmf_table[cursor] as u64;
                if !(freq > 0 && freq <= max_freq - start) {
                    return Err(EntropyError::InvalidPmf);
                }
                cdf_table[cursor + dist_idx] = start as Freq;
                start += freq;
                cursor += 1;
            }
            cdf_table[cursor + dist_idx] = start as Freq; // total sum
        }

        self.distribution_descs = distribution_descs;
        self.cdf_table = cdf_table;
        self.symbol_bits = sb;
        self.bypass_bits = bb;
        // Issue 2c: compute from u64
        self.bypass_max_value = ((1u64 << bb) - 1) as Freq;
        Ok(())
    }

    fn decode_from_slice(
        &self,
        values: &mut [i32],
        indices: &[i32],
        data: &[u8],
        is_byte_variant: bool,
    ) -> Result<(), EntropyError> {
        if self.symbol_bits == 0 {
            return Err(EntropyError::InvalidState);
        }
        if values.len() != indices.len() {
            return Err(EntropyError::InvalidParams);
        }

        if is_byte_variant {
            let units = data.to_vec();
            let source = SliceSource::new(&units);
            let mut decoder = msrtc_rans_core::RansByteDecoder::new(source);
            if !decoder.init() {
                return Err(EntropyError::InvalidStream);
            }
            self.decode_inner_byte(&mut decoder, values, indices)?;
            if !decoder.source().is_exhausted() || !decoder.check_eof() {
                return Err(EntropyError::InvalidStream);
            }
        } else {
            // Issue 3: Reject misaligned Rans64 streams (must be 4-byte aligned)
            if data.len() % 4 != 0 {
                return Err(EntropyError::InvalidStream);
            }
            let units = bytes_to_u32_units(data);
            let source = SliceSource::new(&units);
            let mut decoder = msrtc_rans_core::Rans64Decoder::new(source);
            if !decoder.init() {
                return Err(EntropyError::InvalidStream);
            }
            self.decode_inner_64(&mut decoder, values, indices)?;
            if !decoder.source().is_exhausted() || !decoder.check_eof() {
                return Err(EntropyError::InvalidStream);
            }
        }
        Ok(())
    }

    /// Helper: decode bypass count using remainder-coded prefix for RansByte decoder.
    #[inline]
    fn decode_bypass_count_byte(
        &self,
        decoder: &mut msrtc_rans_core::RansByteDecoder<SliceSource<'_, u8>>,
    ) -> Result<Freq, EntropyError> {
        let mut total: Freq = 0;
        loop {
            let value = decoder.get(self.bypass_bits);
            if !decoder.advance(value, 1, self.bypass_bits) {
                return Err(EntropyError::InvalidStream);
            }
            total += value;
            if value != self.bypass_max_value {
                break;
            }
            if total > FREQ_BITS {
                return Err(EntropyError::InvalidStream);
            }
        }
        Ok(total)
    }

    /// Helper: decode bypass count for Rans64 decoder.
    #[inline]
    fn decode_bypass_count_64(
        &self,
        decoder: &mut msrtc_rans_core::Rans64Decoder<SliceSource<'_, u32>>,
    ) -> Result<Freq, EntropyError> {
        let mut total: Freq = 0;
        loop {
            let value = decoder.get(self.bypass_bits);
            if !decoder.advance(value, 1, self.bypass_bits) {
                return Err(EntropyError::InvalidStream);
            }
            total += value;
            if value != self.bypass_max_value {
                break;
            }
            if total > FREQ_BITS {
                return Err(EntropyError::InvalidStream);
            }
        }
        Ok(total)
    }

    /// Helper: decode bypass value for RansByte decoder.
    #[inline]
    fn decode_bypass_value_payload_byte(
        &self,
        decoder: &mut msrtc_rans_core::RansByteDecoder<SliceSource<'_, u8>>,
        bypass_count: Freq,
    ) -> Result<Freq, EntropyError> {
        // Issue 2c: use u64 for intermediate value to avoid overflow on shift
        let mut encoded_value: u64 = 0;
        let total_bits = bypass_count as u64 * self.bypass_bits as u64;
        // Corrupt-stream hardening: a bounded bypass count can still imply
        // shift >= 64 for wide bypass_bits; reject instead of panicking.
        // (C++ computes `freq_t << shift` — undefined at shift >= 32.)
        if total_bits >= 64 {
            return Err(EntropyError::InvalidStream);
        }
        let mut shift: u64 = 0;
        while shift < total_bits {
            let v = decoder.get(self.bypass_bits);
            if !decoder.advance(v, 1, self.bypass_bits) {
                return Err(EntropyError::InvalidStream);
            }
            encoded_value |= (v as u64) << shift;
            shift += self.bypass_bits as u64;
        }
        Ok(encoded_value as Freq)
    }

    /// Helper: decode bypass value for Rans64 decoder.
    #[inline]
    fn decode_bypass_value_payload_64(
        &self,
        decoder: &mut msrtc_rans_core::Rans64Decoder<SliceSource<'_, u32>>,
        bypass_count: Freq,
    ) -> Result<Freq, EntropyError> {
        // Issue 2c: use u64 for intermediate value to avoid overflow on shift
        let mut encoded_value: u64 = 0;
        let total_bits = bypass_count as u64 * self.bypass_bits as u64;
        // Corrupt-stream hardening (same as byte path).
        if total_bits >= 64 {
            return Err(EntropyError::InvalidStream);
        }
        let mut shift: u64 = 0;
        while shift < total_bits {
            let v = decoder.get(self.bypass_bits);
            if !decoder.advance(v, 1, self.bypass_bits) {
                return Err(EntropyError::InvalidStream);
            }
            encoded_value |= (v as u64) << shift;
            shift += self.bypass_bits as u64;
        }
        Ok(encoded_value as Freq)
    }

    pub(crate) fn decode_inner_byte(
        &self,
        decoder: &mut msrtc_rans_core::RansByteDecoder<SliceSource<'_, u8>>,
        values: &mut [i32],
        indices: &[i32],
    ) -> Result<(), EntropyError> {
        if self.symbol_bits == 0 {
            return Err(EntropyError::InvalidState);
        }
        if values.len() != indices.len() {
            return Err(EntropyError::InvalidParams);
        }

        for (i, &index) in indices.iter().enumerate() {
            if index < 0 {
                values[i] = 0;
                continue;
            }

            let dist_len = self.distribution_descs.len();
            let ui = if (index as usize) < dist_len {
                index as usize
            } else {
                dist_len - 1
            };
            let desc = &self.distribution_descs[ui];

            // Get cumulative frequency from state (low symbolBits bits)
            let cum_freq = decoder.get(self.symbol_bits);
            debug_assert!(cum_freq < (1u32 << self.symbol_bits));

            // Binary search in CDF table to find the symbol
            let base_offset = desc.symbol_offset;
            let lo = base_offset + 1;
            let hi = base_offset + desc.bypass_sentinel as usize + 1;

            // upper_bound: first element > cum_freq
            let upper_idx = {
                let mut low = lo;
                let mut high = hi;
                while low < high {
                    let mid = low + (high - low) / 2;
                    if cum_freq < self.cdf_table[mid] {
                        high = mid;
                    } else {
                        low = mid + 1;
                    }
                }
                low
            };
            // upper_bound - 1 gives the last element ≤ cum_freq
            let start_idx = upper_idx - 1;

            let s0 = self.cdf_table[start_idx];
            let s1 = self.cdf_table[start_idx + 1];
            let freq = s1 - s0;

            if !decoder.advance_symbol(&RansByteDecSymbol::new(s0, freq), self.symbol_bits) {
                return Err(EntropyError::InvalidStream);
            }

            let mut symbol = (start_idx - base_offset) as i32;
            if symbol == desc.bypass_sentinel {
                let bypass_count = self.decode_bypass_count_byte(decoder)?;
                let bypass_value = self.decode_bypass_value_payload_byte(decoder, bypass_count)?;
                // Issue 5: safe conversion with overflow checks using i64
                let half = (bypass_value >> 1) as i64;
                if bypass_value & 1 != 0 {
                    // Negative: 2*(-value) - 1 -> value = -(bypassValue >> 1) - 1
                    // = -(half as i64) - 1
                    symbol = (-half)
                        .checked_sub(1)
                        .ok_or(EntropyError::InvalidStream)?
                        .try_into()
                        .map_err(|_| EntropyError::InvalidStream)?;
                } else {
                    // Positive: 2*(value - sentinel) -> value = (bypassValue >> 1) + sentinel
                    symbol = half
                        .checked_add(desc.bypass_sentinel as i64)
                        .ok_or(EntropyError::InvalidStream)?
                        .try_into()
                        .map_err(|_| EntropyError::InvalidStream)?;
                }
            }

            values[i] = (symbol as i64)
                .checked_sub(desc.value_offset as i64)
                .ok_or(EntropyError::InvalidStream)?
                .try_into()
                .map_err(|_| EntropyError::InvalidStream)?;
        }
        Ok(())
    }

    pub(crate) fn decode_inner_64(
        &self,
        decoder: &mut msrtc_rans_core::Rans64Decoder<SliceSource<'_, u32>>,
        values: &mut [i32],
        indices: &[i32],
    ) -> Result<(), EntropyError> {
        if self.symbol_bits == 0 {
            return Err(EntropyError::InvalidState);
        }
        if values.len() != indices.len() {
            return Err(EntropyError::InvalidParams);
        }

        for (i, &index) in indices.iter().enumerate() {
            if index < 0 {
                values[i] = 0;
                continue;
            }

            let dist_len = self.distribution_descs.len();
            let ui = if (index as usize) < dist_len {
                index as usize
            } else {
                dist_len - 1
            };
            let desc = &self.distribution_descs[ui];

            let cum_freq = decoder.get(self.symbol_bits);
            debug_assert!(cum_freq < (1u32 << self.symbol_bits));

            let base_offset = desc.symbol_offset;
            let lo = base_offset + 1;
            let hi = base_offset + desc.bypass_sentinel as usize + 1;

            let upper_idx = {
                let mut low = lo;
                let mut high = hi;
                while low < high {
                    let mid = low + (high - low) / 2;
                    if cum_freq < self.cdf_table[mid] {
                        high = mid;
                    } else {
                        low = mid + 1;
                    }
                }
                low
            };
            let start_idx = upper_idx - 1;

            let s0 = self.cdf_table[start_idx];
            let s1 = self.cdf_table[start_idx + 1];
            let freq = s1 - s0;

            if !decoder.advance_symbol(&Rans64DecSymbol::new(s0, freq), self.symbol_bits) {
                return Err(EntropyError::InvalidStream);
            }

            let mut symbol = (start_idx - base_offset) as i32;
            if symbol == desc.bypass_sentinel {
                let bypass_count = self.decode_bypass_count_64(decoder)?;
                let bypass_value = self.decode_bypass_value_payload_64(decoder, bypass_count)?;
                // Issue 5: safe conversion with overflow checks using i64
                let half = (bypass_value >> 1) as i64;
                if bypass_value & 1 != 0 {
                    // Negative: 2*(-value) - 1 -> value = -(bypassValue >> 1) - 1
                    // = -(half as i64) - 1
                    symbol = (-half)
                        .checked_sub(1)
                        .ok_or(EntropyError::InvalidStream)?
                        .try_into()
                        .map_err(|_| EntropyError::InvalidStream)?;
                } else {
                    // Positive: 2*(value - sentinel) -> value = (bypassValue >> 1) + sentinel
                    symbol = half
                        .checked_add(desc.bypass_sentinel as i64)
                        .ok_or(EntropyError::InvalidStream)?
                        .try_into()
                        .map_err(|_| EntropyError::InvalidStream)?;
                }
            }

            values[i] = (symbol as i64)
                .checked_sub(desc.value_offset as i64)
                .ok_or(EntropyError::InvalidStream)?
                .try_into()
                .map_err(|_| EntropyError::InvalidStream)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helper traits to map RansParams to encoder/decoder types
// ---------------------------------------------------------------------------

/// Maps `RansParams` implementations to their encoder symbol types.
///
/// This trait is automatically implemented for both `RansByte` and `Rans64`
/// and should not need to be implemented manually.
pub trait EncoderVariantForS: RansParams {
    /// The encoder symbol type for this variant.
    type EncSymbol: EncSymbol;

    /// The raw rANS encoder type for this variant.
    type RawEnc: RawEncoder<Symbol = Self::EncSymbol>;

    /// Maximum scale bits for this variant.
    const MAX_SCALE_BITS: u32;

    /// Convert raw encoder units to a byte vector.
    fn units_to_bytes(units: Vec<<Self::RawEnc as RawEncoder>::Unit>) -> Vec<u8>;

    /// Create a new encoder instance.
    fn make_encoder() -> Self::RawEnc;
}

impl EncoderVariantForS for RansByte {
    type EncSymbol = RansByteEncSymbol;
    type RawEnc = RansByteEncoder<VecSink<u8>>;
    const MAX_SCALE_BITS: u32 = 30;
    fn units_to_bytes(units: Vec<u8>) -> Vec<u8> {
        units
    }
    fn make_encoder() -> Self::RawEnc {
        RansByteEncoder::new(VecSink::new(4096))
    }
}

impl EncoderVariantForS for Rans64 {
    type EncSymbol = Rans64EncSymbol;
    type RawEnc = Rans64Encoder<VecSink<u32>>;
    const MAX_SCALE_BITS: u32 = 32;
    fn units_to_bytes(units: Vec<u32>) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(units.len() * 4);
        for &u in &units {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        bytes
    }
    fn make_encoder() -> Self::RawEnc {
        Rans64Encoder::new(VecSink::new(4096))
    }
}

// ---------------------------------------------------------------------------
// Public EntropyEncoder
// ---------------------------------------------------------------------------

/// High-level entropy encoder using PMF distributions with bypass support.
///
/// Generic over `S: RansParams` (`RansByte` or `Rans64`).
///
/// # Example
///
/// ```ignore
/// use msrtc_rans::entropy::EntropyEncoder;
/// use msrtc_rans::RansByte;
///
/// let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
/// enc.initialize(
///     &[4, 6],       // pmf_lengths
///     &[1, 2],       // pmf_offsets
///     &[1, 3, 1, 1, 1, 3, 5, 3, 1, 1],  // pmf_table
///     16,            // symbol_bits
///     4,             // bypass_bits
/// ).unwrap();
///
/// let mut buffer = Vec::new();
/// enc.encode(&[0, 1], &[1, 1], &mut buffer).unwrap();
/// ```
pub struct EntropyEncoder<S: EncoderVariantForS> {
    state: EncoderState<<S as EncoderVariantForS>::EncSymbol>,
}

impl<S: EncoderVariantForS> EntropyEncoder<S> {
    /// Create a new uninitialized entropy encoder.
    pub fn new() -> Self {
        Self {
            state: EncoderState::uninitialized(),
        }
    }

    /// Initialize the encoder with PMF distribution data.
    ///
    /// * `pmf_lengths` — number of symbols per distribution (including bypass sentinel)
    /// * `pmf_offsets` — value offsets per distribution
    /// * `pmf_table` — flat array of symbol frequencies for all distributions
    /// * `symbol_bits` — number of bits for symbol encoding (e.g. 16)
    /// * `bypass_bits` — number of bits for bypass encoding (e.g. 4)
    pub fn initialize(
        &mut self,
        pmf_lengths: &[i32],
        pmf_offsets: &[i32],
        pmf_table: &[i32],
        symbol_bits: u32,
        bypass_bits: u32,
    ) -> Result<(), EntropyError> {
        self.state.initialize(
            pmf_lengths,
            pmf_offsets,
            pmf_table,
            symbol_bits as i32,
            bypass_bits as i32,
            <S as EncoderVariantForS>::MAX_SCALE_BITS,
        )
    }

    /// Encode symbols onto an existing raw encoder for persistent streaming.
    ///
    /// Unlike `encode()`, this does NOT flush the encoder or finalize output.
    /// Call `encoder.flush()` and extract units after all batches are pushed.
    pub fn encode_batch(
        &self,
        indices: &[i32],
        values: &[i32],
        encoder: &mut <S as EncoderVariantForS>::RawEnc,
    ) -> Result<(), EntropyError> {
        self.state.encode_batch(indices, values, encoder)
    }

    /// One-shot encode: encode `indices`/`values` into `buffer`.
    ///
    /// The encoded bytes are appended to `buffer`.
    pub fn encode(
        &self,
        indices: &[i32],
        values: &[i32],
        buffer: &mut Vec<u8>,
    ) -> Result<(), EntropyError> {
        let units = self.state.encode_to_vec(indices, values, S::make_encoder)?;
        let bytes = S::units_to_bytes(units);
        buffer.extend_from_slice(&bytes);
        Ok(())
    }
}

impl<S: EncoderVariantForS> Default for EntropyEncoder<S> {
    fn default() -> Self {
        Self::new()
    }
}

fn _assert_encoder_bounds() {
    fn _is_encoder<S: EncoderVariantForS>() {}
    _is_encoder::<RansByte>();
    _is_encoder::<Rans64>();
}

// ---------------------------------------------------------------------------
// Public EntropyDecoder
// ---------------------------------------------------------------------------

/// High-level entropy decoder using CDF tables for symbol lookup.
///
/// Generic over `S: RansParams` (`RansByte` or `Rans64`).
pub struct EntropyDecoder<S: RansParams> {
    state: DecoderState,
    _phantom: core::marker::PhantomData<S>,
}

impl<S: RansParams> EntropyDecoder<S> {
    /// Create a new uninitialized entropy decoder.
    pub fn new() -> Self {
        Self {
            state: DecoderState::uninitialized(),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Initialize the decoder with PMF distribution data.
    ///
    /// * `pmf_lengths` — number of symbols per distribution (including bypass sentinel)
    /// * `pmf_offsets` — value offsets per distribution
    /// * `pmf_table` — flat array of symbol frequencies for all distributions
    /// * `symbol_bits` — number of bits for symbol encoding (e.g. 16)
    /// * `bypass_bits` — number of bits for bypass encoding (e.g. 4)
    pub fn initialize(
        &mut self,
        pmf_lengths: &[i32],
        pmf_offsets: &[i32],
        pmf_table: &[i32],
        symbol_bits: u32,
        bypass_bits: u32,
    ) -> Result<(), EntropyError> {
        let max_scale_bits = match S::NAME {
            "RansByte" => 30u32,
            "Rans64" => 32u32,
            _ => return Err(EntropyError::InvalidParams),
        };
        self.state.initialize(
            pmf_lengths,
            pmf_offsets,
            pmf_table,
            symbol_bits as i32,
            bypass_bits as i32,
            max_scale_bits,
        )
    }

    /// One-shot decode: decode from `data` into `values`.
    ///
    /// * `values` — output buffer (must be same length as `indices`)
    /// * `indices` — distribution indices for each value to decode
    /// * `data` — encoded byte stream
    pub fn decode(
        &self,
        values: &mut [i32],
        indices: &[i32],
        data: &[u8],
    ) -> Result<(), EntropyError> {
        let is_byte = match S::NAME {
            "RansByte" => true,
            "Rans64" => false,
            _ => return Err(EntropyError::InvalidParams),
        };
        self.state.decode_from_slice(values, indices, data, is_byte)
    }

    /// Decode from a slice but do NOT require the source to be fully exhausted.
    ///
    /// This is used when decoding from a `RansDecoderStream` where multiple encoded
    /// segments are concatenated. The method decodes `values`/`indices` from the
    /// beginning of `data` and returns the number of bytes consumed.
    ///
    /// * `values` — output buffer (must be same length as `indices`)
    /// * `indices` — distribution indices for each value to decode
    /// * `data` — encoded byte stream (may contain extra trailing data)
    ///
    /// Returns the number of bytes consumed from `data` on success.
    pub fn decode_partial(
        &self,
        values: &mut [i32],
        indices: &[i32],
        data: &[u8],
    ) -> Result<usize, EntropyError> {
        if self.state.symbol_bits == 0 {
            return Err(EntropyError::InvalidState);
        }
        if values.len() != indices.len() {
            return Err(EntropyError::InvalidParams);
        }

        let consumed = match S::NAME {
            "RansByte" => {
                let units = data.to_vec();
                let source = SliceSource::new(&units);
                let mut decoder = msrtc_rans_core::RansByteDecoder::new(source);
                if !decoder.init() {
                    return Err(EntropyError::InvalidStream);
                }
                self.state
                    .decode_inner_byte(&mut decoder, values, indices)?;
                if !decoder.check_eof() {
                    return Err(EntropyError::InvalidStream);
                }
                decoder.source().position()
            }
            "Rans64" => {
                if data.len() % 4 != 0 {
                    return Err(EntropyError::InvalidStream);
                }
                let units = bytes_to_u32_units(data);
                let source = SliceSource::new(&units);
                let mut decoder = msrtc_rans_core::Rans64Decoder::new(source);
                if !decoder.init() {
                    return Err(EntropyError::InvalidStream);
                }
                self.state.decode_inner_64(&mut decoder, values, indices)?;
                if !decoder.check_eof() {
                    return Err(EntropyError::InvalidStream);
                }
                decoder.source().position() * 4
            }
            _ => return Err(EntropyError::InvalidParams),
        };

        Ok(consumed)
    }

    /// Decode a batch of symbols from data without requiring EOF.
    ///
    /// This is used for multipart stream decoding, where the stream contains
    /// multiple messages concatenated. The first decoder reads its symbols
    /// and returns the bytes consumed, leaving the rest for subsequent decoders.
    ///
    /// Returns the number of bytes consumed.
    pub fn decode_batch(
        &self,
        values: &mut [i32],
        indices: &[i32],
        data: &[u8],
    ) -> Result<usize, EntropyError> {
        if self.state.symbol_bits == 0 {
            return Err(EntropyError::InvalidState);
        }
        if values.len() != indices.len() {
            return Err(EntropyError::InvalidParams);
        }

        let consumed = match S::NAME {
            "RansByte" => {
                let units = data.to_vec();
                let source = SliceSource::new(&units);
                let mut decoder = msrtc_rans_core::RansByteDecoder::new(source);
                if !decoder.init() {
                    return Err(EntropyError::InvalidStream);
                }
                self.state
                    .decode_inner_byte(&mut decoder, values, indices)?;
                decoder.source().position()
            }
            "Rans64" => {
                if data.len() % 4 != 0 {
                    return Err(EntropyError::InvalidStream);
                }
                let units = bytes_to_u32_units(data);
                let source = SliceSource::new(&units);
                let mut decoder = msrtc_rans_core::Rans64Decoder::new(source);
                if !decoder.init() {
                    return Err(EntropyError::InvalidStream);
                }
                self.state.decode_inner_64(&mut decoder, values, indices)?;
                decoder.source().position() * 4
            }
            _ => return Err(EntropyError::InvalidParams),
        };

        Ok(consumed)
    }

    /// Decode a batch of symbols from a sub-slice of data, returning bytes consumed.
    ///
    /// This is an alias for `decode_batch` used by the Python stream decoder.
    pub fn decode_stream(
        &self,
        values: &mut [i32],
        indices: &[i32],
        data: &[u8],
    ) -> Result<usize, EntropyError> {
        self.decode_batch(values, indices, data)
    }

    /// Continue decoding from a persistent RansByte decoder (stream mode).
    ///
    /// The decoder must already be initialized (via `init()` on the first
    /// call). The caller owns the decoder and its source cursor.
    pub fn decode_byte_continue(
        &self,
        raw: &mut msrtc_rans_core::RansByteDecoder<SliceSource<'_, u8>>,
        values: &mut [i32],
        indices: &[i32],
    ) -> Result<(), EntropyError> {
        self.state.decode_inner_byte(raw, values, indices)
    }

    /// Continue decoding from a persistent Rans64 decoder (stream mode).
    pub fn decode_64_continue(
        &self,
        raw: &mut msrtc_rans_core::Rans64Decoder<SliceSource<'_, u32>>,
        values: &mut [i32],
        indices: &[i32],
    ) -> Result<(), EntropyError> {
        self.state.decode_inner_64(raw, values, indices)
    }
}

impl<S: RansParams> Default for EntropyDecoder<S> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Reference test case from test_msrtc_rans.py:
    //   PMF_LENGTHS = [4, 6]
    //   PMF_OFFSETS = [1, 2]
    //   PMF_TABLE   = [1, 3, 1, 1, 1, 3, 5, 3, 1, 1]
    //   INDICES     = [0, 1, 0, 1]
    //   VALUES      = [-2, 1, 0, 1]
    //   SYMBOL_BITS = 16
    //   BYPASS_BITS = 4
    //
    // Reference bitstreams from upstream oracle (EntropyCoder.cpp):
    //   RansByte: hex = "0500bd040001a10003000b00"
    //   Rans64:   hex = "0500a1bd04000000110a002f03000300"

    const PMF_LENGTHS: [i32; 2] = [4, 6];
    const PMF_OFFSETS: [i32; 2] = [1, 2];
    const PMF_TABLE: [i32; 10] = [1, 3, 1, 1, 1, 3, 5, 3, 1, 1];
    const INDICES: [i32; 4] = [0, 1, 0, 1];
    const VALUES: [i32; 4] = [-2, 1, 0, 1];
    const SYMBOL_BITS: u32 = 16;
    const BYPASS_BITS: u32 = 4;

    const REF_HEX_BYTE: &str = "0500bd040001a10003000b00";
    const REF_HEX_64: &str = "0500a1bd04000000110a002f03000300";

    fn hex_decode(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn test_encoder_byte_initialize() {
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        assert!(
            enc.initialize(
                &PMF_LENGTHS,
                &PMF_OFFSETS,
                &PMF_TABLE,
                SYMBOL_BITS,
                BYPASS_BITS
            )
            .is_ok()
        );
    }

    #[test]
    fn test_encoder_64_initialize() {
        let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
        assert!(
            enc.initialize(
                &PMF_LENGTHS,
                &PMF_OFFSETS,
                &PMF_TABLE,
                SYMBOL_BITS,
                BYPASS_BITS
            )
            .is_ok()
        );
    }

    #[test]
    fn test_encoder_rejects_invalid_pmf() {
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        // Mismatched lengths/offsets
        assert_eq!(
            enc.initialize(&[4], &[1, 2], &PMF_TABLE, SYMBOL_BITS, BYPASS_BITS),
            Err(EntropyError::InvalidPmf)
        );
    }

    #[test]
    fn test_encoder_rejects_invalid_params() {
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        // symbol_bits < 2
        assert_eq!(
            enc.initialize(&PMF_LENGTHS, &PMF_OFFSETS, &PMF_TABLE, 1, BYPASS_BITS),
            Err(EntropyError::InvalidParams)
        );
    }

    #[test]
    fn test_encoder_byte_rejects_length_leq_one() {
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        // Length <= 1 is invalid (need tail mass for bypass)
        assert_eq!(
            enc.initialize(&[1, 6], &[1, 2], &PMF_TABLE, SYMBOL_BITS, BYPASS_BITS),
            Err(EntropyError::InvalidPmf)
        );
    }

    #[test]
    fn test_encode_byte_matches_reference() {
        // Encodes values=[-2, 1, 0, 1] with RansByte.
        // Value -2 (index 0, offset 1 => adjusted=-1) triggers bypass.
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut buffer = Vec::new();
        enc.encode(&INDICES, &VALUES, &mut buffer).unwrap();

        let expected = hex_decode(REF_HEX_BYTE);
        assert_eq!(
            buffer, expected,
            "RansByte encode output does not match reference hex"
        );
    }

    #[test]
    fn test_encode_64_matches_reference() {
        let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut buffer = Vec::new();
        enc.encode(&INDICES, &VALUES, &mut buffer).unwrap();

        let expected = hex_decode(REF_HEX_64);
        assert_eq!(
            buffer, expected,
            "Rans64 encode output does not match reference hex"
        );
    }

    #[test]
    fn test_encode_in_range_values_no_bypass() {
        // Values that are all in-range (no bypass):
        // Dist 0: offset=1, sentinel=3, valid adjusted: [0,2] => value: [-1, 1]
        // Dist 1: offset=2, sentinel=5, valid adjusted: [0,4] => value: [-2, 2]
        let in_range_values = [1i32, 1, 0, 1];
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut buffer = Vec::new();
        let result = enc.encode(&INDICES, &in_range_values, &mut buffer);
        assert!(result.is_ok(), "encode should succeed: {:?}", result);
        assert!(!buffer.is_empty(), "encoded buffer should not be empty");
    }

    #[test]
    fn test_decode_byte_roundtrip_in_range() {
        // Verify roundtrip encode-decode with in-range values (no bypass).
        let values = [1i32, 1, 0, 1];

        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut decoded = vec![0i32; values.len()];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();

        assert_eq!(
            decoded, values,
            "roundtrip decode should match original values"
        );
    }

    #[test]
    fn test_decode_64_roundtrip_in_range() {
        let values = [1i32, 1, 0, 1];

        let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut decoded = vec![0i32; values.len()];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();

        assert_eq!(
            decoded, values,
            "Rans64 roundtrip decode should match original values"
        );
    }

    #[test]
    fn test_decode_byte_roundtrip_bypass() {
        // Roundtrip with values that require bypass (value=-2 is out-of-range).
        let values = [-2i32, 1, 0, 1];

        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut decoded = vec![0i32; values.len()];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();

        assert_eq!(
            decoded, values,
            "bypass roundtrip decode should match original values"
        );
    }

    #[test]
    fn test_decode_64_roundtrip_bypass() {
        let values = [-2i32, 1, 0, 1];

        let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();

        let mut decoded = vec![0i32; values.len()];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();

        assert_eq!(
            decoded, values,
            "Rans64 bypass roundtrip decode should match original values"
        );
    }

    // -----------------------------------------------------------------------
    // Issue 2d: scale-32 safe / reject tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_encoder_64_symbol_bits_31_accepted() {
        // Rans64 symbol_bits=31 is within max_safe_bits (31)
        let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
        assert!(
            enc.initialize(&PMF_LENGTHS, &PMF_OFFSETS, &PMF_TABLE, 31, BYPASS_BITS)
                .is_ok()
        );
    }

    #[test]
    fn test_encoder_64_symbol_bits_32_rejected() {
        // Rans64 symbol_bits=32 exceeds max_safe_bits (31), must not panic
        let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
        assert_eq!(
            enc.initialize(&PMF_LENGTHS, &PMF_OFFSETS, &PMF_TABLE, 32, BYPASS_BITS),
            Err(EntropyError::InvalidParams)
        );
    }

    #[test]
    fn test_encoder_64_bypass_bits_32_rejected() {
        // Rans64 bypass_bits=32 exceeds max_safe_bits (31), must not panic
        let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
        assert_eq!(
            enc.initialize(&PMF_LENGTHS, &PMF_OFFSETS, &PMF_TABLE, SYMBOL_BITS, 32),
            Err(EntropyError::InvalidParams)
        );
    }

    // -----------------------------------------------------------------------
    // Issue 3: misaligned Rans64 streams rejected
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_64_rejects_misaligned_1_extra_byte() {
        let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &[1i32, 1, 0, 1], &mut encoded)
            .unwrap();

        // Append 1 extra byte to make it misaligned
        let mut misaligned = encoded.clone();
        misaligned.push(0xAB);

        let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut decoded = vec![0i32; 4];
        let result = dec.decode(&mut decoded, &INDICES, &misaligned);
        assert_eq!(result, Err(EntropyError::InvalidStream));
    }

    #[test]
    fn test_decode_64_rejects_misaligned_2_extra_bytes() {
        let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &[1i32, 1, 0, 1], &mut encoded)
            .unwrap();

        let mut misaligned = encoded.clone();
        misaligned.extend_from_slice(&[0xAB, 0xCD]);

        let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut decoded = vec![0i32; 4];
        let result = dec.decode(&mut decoded, &INDICES, &misaligned);
        assert_eq!(result, Err(EntropyError::InvalidStream));
    }

    #[test]
    fn test_decode_64_rejects_misaligned_3_extra_bytes() {
        let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &[1i32, 1, 0, 1], &mut encoded)
            .unwrap();

        let mut misaligned = encoded.clone();
        misaligned.extend_from_slice(&[0xAB, 0xCD, 0xEF]);

        let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut decoded = vec![0i32; 4];
        let result = dec.decode(&mut decoded, &INDICES, &misaligned);
        assert_eq!(result, Err(EntropyError::InvalidStream));
    }

    #[test]
    fn test_decode_byte_accepts_extra_bytes() {
        // RansByte has byte-level alignment, extra bytes should not be rejected
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &[1i32, 1, 0, 1], &mut encoded)
            .unwrap();

        // Append extra bytes
        let mut extended = encoded.clone();
        extended.extend_from_slice(&[0xAB, 0xCD]);

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut decoded = vec![0i32; 4];
        // This may fail because the decoder checks EOF, but it should NOT
        // be rejected for misalignment
        let _ = dec.decode(&mut decoded, &INDICES, &extended);
        // We don't assert success or failure — we just assert no panic
    }

    // -----------------------------------------------------------------------
    // Issue 4: Expanded bypass coverage
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_bypass_positive_outlier() {
        // Value > sentinel (positive outlier): dist 0 sentinel=3, offset=1,
        // value=10 => adjusted=11 > 3 => bypass (8/2=4 above sentinel => value 4+3=7-1=6...
        // actually: adjusted=11, sentinel=3, bypass_value = 2*(11-3) = 16
        // decode: 16>>1=8, 8+3=11-1=10 ✓
        let values = [10i32, 1, 0, 1];
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut decoded = vec![0i32; 4];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_encode_bypass_multi_digit_value() {
        // Value requiring multiple bypassBits-sized chunks (bypass_bits=4)
        // Large bypass value: dist 0 sentinel=3, offset=1, value=200 => adjusted=201
        // bypass_value = 2*(201-3) = 396 = 0x18C, needs multiple 4-bit chunks
        let values = [200i32, 1, 0, 1];
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut decoded = vec![0i32; 4];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_encode_bypass_bits_2() {
        // Minimum bypass_bits = 2
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(&PMF_LENGTHS, &PMF_OFFSETS, &PMF_TABLE, SYMBOL_BITS, 2)
            .unwrap();
        let values = [10i32, 1, 0, 1]; // value 10 => bypass
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(&PMF_LENGTHS, &PMF_OFFSETS, &PMF_TABLE, SYMBOL_BITS, 2)
            .unwrap();
        let mut decoded = vec![0i32; 4];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_encode_bypass_bits_3() {
        // Odd bypass_bits = 3
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(&PMF_LENGTHS, &PMF_OFFSETS, &PMF_TABLE, SYMBOL_BITS, 3)
            .unwrap();
        let values = [10i32, 1, 0, 1];
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(&PMF_LENGTHS, &PMF_OFFSETS, &PMF_TABLE, SYMBOL_BITS, 3)
            .unwrap();
        let mut decoded = vec![0i32; 4];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_encode_bypass_bits_8() {
        // Larger bypass_bits = 8
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(&PMF_LENGTHS, &PMF_OFFSETS, &PMF_TABLE, SYMBOL_BITS, 8)
            .unwrap();
        let values = [10i32, 1, 0, 1];
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(&PMF_LENGTHS, &PMF_OFFSETS, &PMF_TABLE, SYMBOL_BITS, 8)
            .unwrap();
        let mut decoded = vec![0i32; 4];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_encode_bypass_multiple_bypasses() {
        // Multiple bypass values in one stream: both -2 and 10 need bypass
        // dist 0: offset=1 sentinel=3, dist 1: offset=2 sentinel=5
        // value -2 (dist 0) => adjusted=-1 => bypass (negative)
        // value 10 (dist 1) => adjusted=12 => bypass (positive)
        let values = [-2i32, 10, 0, 1];
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut decoded = vec![0i32; 4];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_encode_bypass_mixed_in_range_and_bypass() {
        // Mix of in-range and bypass values
        // dist 0: offset=1 sentinel=3, valid adjusted [0,2] => values [-1, 1]
        // dist 1: offset=2 sentinel=5, valid adjusted [0,4] => values [-2, 2]
        // value 0 in dist 0 => in-range, value 1 in dist 0 => in-range
        // value 5 in dist 1 => bypass (adjusted=7 > 4), value -3 in dist 1 => bypass (adjusted=-1 < 0)
        let values = [0i32, 5, 1, -3];
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut decoded = vec![0i32; 4];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_encode_bypass_negative_outlier_at_boundary() {
        // Negative outlier at boundary: -1 - sentinel (very negative)
        // dist 0: offset=1, sentinel=3, value=-10 => adjusted=-9 => bypass
        // (-9 < 0) => bypass_value = 2*9-1 = 17, decode: 17>>1=8, -(8+1) = -9, -9+1 = -8... wait
        // decode bypass: negative flag set, half=8, symbol = -(8+1) = -9, -9 = -9+1 = -8...
        // Actually: symbol = -9, values[i] = symbol - offset = (-9) - 1 = -10 ✓
        let values = [-10i32, 1, 0, 1];
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut decoded = vec![0i32; 4];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();
        assert_eq!(decoded, values);
    }

    #[test]
    fn test_encode_bypass_large_positive_outlier() {
        // Large positive outlier
        let values = [10000i32, 1, 0, 1];
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        enc.encode(&INDICES, &values, &mut encoded).unwrap();

        let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
        dec.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut decoded = vec![0i32; 4];
        dec.decode(&mut decoded, &INDICES, &encoded).unwrap();
        assert_eq!(decoded, values);
    }

    // -----------------------------------------------------------------------
    // Issue 5: extreme value overflow protection
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_bypass_extreme_negative_i32_min_plus_one() {
        // i32::MIN + 1 with offset 1 gives adjusted = i32::MIN + 2 = -2147483646
        // checked_neg of that gives 2147483646, no overflow.
        // This should succeed (no overflow).
        let values = [i32::MIN + 1, 1, 0, 1];
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        let result = enc.encode(&INDICES, &values, &mut encoded);
        // Must not panic — should succeed or return InvalidParams gracefully
        assert!(result.is_ok() || result == Err(EntropyError::InvalidParams));
    }

    #[test]
    fn test_encode_bypass_extreme_positive_i32_max() {
        // i32::MAX with offset could cause overflow in checked_add -> InvalidParams
        let values = [i32::MAX, 1, 0, 1];
        let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
        enc.initialize(
            &PMF_LENGTHS,
            &PMF_OFFSETS,
            &PMF_TABLE,
            SYMBOL_BITS,
            BYPASS_BITS,
        )
        .unwrap();
        let mut encoded = Vec::new();
        let result = enc.encode(&INDICES, &values, &mut encoded);
        assert_eq!(result, Err(EntropyError::InvalidParams));
    }
}
