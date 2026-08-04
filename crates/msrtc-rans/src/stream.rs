// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # rANS stream types
//!
//! Standalone `RansEncoderStream` and `RansDecoderStream` matching
//! Microsoft's `RansEncoderStream` / `RansDecoderStream` from
//! `EntropyCoder.h`.
//!
//! - Encoder stream keeps a **persistent raw encoder state** across
//!   `push()` calls (matching Microsoft's `RawRansEncoderStream`),
//!   flushing once at the end.
//! - Decoder stream owns the encoded data and advances a persistent
//!   decoder across sequential `decode` calls.
//!
//! Both types are generic over the rANS variant (`RansByte` or `Rans64`),
//! making variant mismatches a compile-time error.

use alloc::vec::Vec;

use crate::entropy::{
    EncoderVariantForS, EntropyDecoder, EntropyEncoder, EntropyError, RawEncoder,
};
use crate::source::SliceSource;
use crate::{Rans64Decoder, RansByteDecoder};

/// rANS variants for runtime dispatch (Python-facing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RansVariant {
    /// RansByte: u32 state, u8 unit (Microsoft value 1)
    RansByte,
    /// Rans64: u64 state, u32 unit (Microsoft value 0)
    Rans64,
}

impl RansVariant {
    /// Microsoft's integer value: Rans64=0, RansByte=1.
    pub const fn as_int(&self) -> i32 {
        match self {
            RansVariant::RansByte => 1,
            RansVariant::Rans64 => 0,
        }
    }

    /// From Microsoft's integer value.
    pub const fn from_int(v: i32) -> Option<Self> {
        match v {
            1 => Some(RansVariant::RansByte),
            0 => Some(RansVariant::Rans64),
            _ => None,
        }
    }
}

/// rANS encoder stream for a specific variant `S`.
///
/// Keeps a persistent raw encoder state across `push()` calls. `flush()`
/// writes the final state and yields the encoded message; the stream can
/// then be reused. `reset()` aborts the current session.
#[derive(Debug)]
pub struct RansEncoderStream<S: EncoderVariantForS> {
    encoder: Option<S::RawEnc>,
    _s: core::marker::PhantomData<S>,
}

impl<S: EncoderVariantForS> Default for RansEncoderStream<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: EncoderVariantForS> RansEncoderStream<S> {
    /// Create a new encoder stream.
    pub fn new() -> Self {
        Self {
            encoder: None,
            _s: core::marker::PhantomData,
        }
    }

    /// Whether the stream has an active encoder session.
    pub fn is_initialized(&self) -> bool {
        self.encoder.is_some()
    }

    /// Push an entropy-encoded batch onto the stream.
    ///
    /// Encodes `values` using the given `encoder`'s PMF, continuing the
    /// persistent raw encoder state. Matches `EntropyEncoder::push` in
    /// Python and `IEntropyEncoderImpl::Encode(stream, indices, values)`.
    pub fn push(
        &mut self,
        encoder: &EntropyEncoder<S>,
        indices: &[i32],
        values: &[i32],
    ) -> Result<(), EntropyError> {
        let raw = self.encoder_mut();
        encoder.encode_batch(indices, values, raw)
    }

    /// Flush the current session and return the encoded message.
    ///
    /// After flush, the stream is reset for reuse. Returns an error if no
    /// data has been pushed (matching the Python `flush` which raises on
    /// empty output).
    pub fn flush(&mut self) -> Result<Vec<u8>, EntropyError> {
        let mut raw = self.encoder.take().ok_or(EntropyError::InvalidState)?;
        raw.flush();
        let units = raw.into_units();
        Ok(S::units_to_bytes(units))
    }

    /// Abort the current session, discarding all pushed data.
    pub fn reset(&mut self) {
        self.encoder = None;
    }

    fn encoder_mut(&mut self) -> &mut S::RawEnc {
        if self.encoder.is_none() {
            self.encoder = Some(S::make_encoder());
        }
        self.encoder.as_mut().expect("just set")
    }
}

/// rANS decoder stream for a specific variant `S`.
///
/// Owns the encoded data and keeps a **persistent decode cursor**
/// (unit position + rANS state) across sequential `decode` calls.
/// This matches Microsoft's `RansDecoderStream`, where the raw decoder
/// is initialized once in `Open()` and reused by every `Decode` call.
#[derive(Debug)]
pub struct RansDecoderStream<S: EncoderVariantForS> {
    data: Vec<u8>,
    /// (unit position, decoder state). `None` before the first decode.
    cursor: Option<(usize, u64)>,
    _s: core::marker::PhantomData<S>,
}

impl<S: EncoderVariantForS> Default for RansDecoderStream<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: EncoderVariantForS> RansDecoderStream<S> {
    /// Create a decoder stream (closed).
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            cursor: None,
            _s: core::marker::PhantomData,
        }
    }

    /// Create a decoder stream opened on the given data.
    pub fn open_on(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            cursor: None,
            _s: core::marker::PhantomData,
        }
    }

    /// Whether the stream is open (has data).
    pub fn is_open(&self) -> bool {
        !self.data.is_empty()
    }

    /// Whether the current state can be at EOF.
    ///
    /// EOF requires the source to be exhausted and the decoder state to be
    /// at `LowerBound` (or the stream to be unopened).
    pub fn check_eof(&self) -> bool {
        let Some((pos, state)) = self.cursor else {
            return !self.is_open();
        };
        let unit_len = match S::NAME {
            "RansByte" => self.data.len(),
            "Rans64" => self.data.len() / 4,
            _ => 0,
        };
        let lower = match S::NAME {
            "RansByte" => 1u64 << 23,
            "Rans64" => 1u64 << 31,
            _ => 0,
        };
        pos == unit_len && state == lower
    }

    /// Open the stream on new data, resetting the cursor.
    pub fn open(&mut self, data: &[u8]) {
        self.data = data.to_vec();
        self.cursor = None;
    }

    /// Close the stream, releasing data.
    pub fn close(&mut self) {
        self.data.clear();
        self.cursor = None;
    }

    /// Check that decoding reached the end of the message and close on success.
    pub fn decode_eof(&mut self) -> Result<(), EntropyError> {
        if !self.check_eof() {
            return Err(EntropyError::InvalidStream);
        }
        self.close();
        Ok(())
    }

    /// Decode a batch of symbols from the stream, advancing the persistent cursor.
    ///
    /// The first call initializes the raw decoder from the stream's initial
    /// state; subsequent calls continue from the saved cursor.
    pub fn decode(
        &mut self,
        decoder: &EntropyDecoder<S>,
        values: &mut [i32],
        indices: &[i32],
    ) -> Result<(), EntropyError> {
        match S::NAME {
            "RansByte" => {
                let units = self.data.clone();
                let mut source = SliceSource::new(&units);
                let mut raw = match self.cursor {
                    Some((pos, state)) => {
                        source.seek(pos);
                        RansByteDecoder::from_state(source, state as u32)
                    }
                    None => {
                        let mut d = RansByteDecoder::new(source);
                        if !d.init() {
                            return Err(EntropyError::InvalidStream);
                        }
                        d
                    }
                };
                decoder.decode_byte_continue(&mut raw, values, indices)?;
                self.cursor = Some((raw.source().position(), raw.state() as u64));
                Ok(())
            }
            "Rans64" => {
                if self.data.len() % 4 != 0 {
                    return Err(EntropyError::InvalidStream);
                }
                let units: Vec<u32> = self
                    .data
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                let mut source = SliceSource::new(&units);
                let mut raw = match self.cursor {
                    Some((pos, state)) => {
                        source.seek(pos);
                        Rans64Decoder::from_state(source, state)
                    }
                    None => {
                        let mut d = Rans64Decoder::new(source);
                        if !d.init() {
                            return Err(EntropyError::InvalidStream);
                        }
                        d
                    }
                };
                decoder.decode_64_continue(&mut raw, values, indices)?;
                self.cursor = Some((raw.source().position(), raw.state()));
                Ok(())
            }
            _ => Err(EntropyError::InvalidParams),
        }
    }

    /// Number of bytes consumed so far (unit position × unit size).
    pub fn bytes_consumed(&self) -> usize {
        match self.cursor {
            Some((pos, _)) => match S::NAME {
                "RansByte" => pos,
                "Rans64" => pos * 4,
                _ => 0,
            },
            None => 0,
        }
    }

    /// The full stream data (borrowed).
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// Convert u32 units to little-endian bytes.
pub fn units_to_le_bytes(units: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(units.len() * 4);
    for &u in units {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entropy::EntropyDecoder;
    use crate::variant::{Rans64, RansByte};

    #[test]
    fn test_variant_values() {
        assert_eq!(RansVariant::RansByte.as_int(), 1);
        assert_eq!(RansVariant::Rans64.as_int(), 0);
        assert_eq!(RansVariant::from_int(1), Some(RansVariant::RansByte));
        assert_eq!(RansVariant::from_int(0), Some(RansVariant::Rans64));
        assert_eq!(RansVariant::from_int(2), None);
    }

    #[test]
    fn test_encoder_stream_multipart() {
        // Matches test_encode_decode_multi_part_0 from upstream
        let pmf_lengths1 = vec![4, 6];
        let pmf_offsets1 = vec![1, 2];
        let pmf_table1 = vec![1, 3, 1, 1, 1, 3, 5, 3, 1, 1];
        let values1 = vec![-2, 1, 0, 1];
        let indices1 = vec![0, 1, 0, 1];

        let pmf_lengths2 = vec![5];
        let pmf_offsets2 = vec![1];
        let pmf_table2 = vec![1, 3, 3, 1, 1];
        let values2 = vec![-2, 1, 2];
        let indices2 = vec![0, 0, 0];

        let mut encoder1 = EntropyEncoder::<RansByte>::new();
        encoder1
            .initialize(&pmf_lengths1, &pmf_offsets1, &pmf_table1, 16, 4)
            .expect("init1");
        let mut encoder2 = EntropyEncoder::<RansByte>::new();
        encoder2
            .initialize(&pmf_lengths2, &pmf_offsets2, &pmf_table2, 16, 4)
            .expect("init2");

        let mut stream = RansEncoderStream::<RansByte>::new();
        stream.push(&encoder2, &indices2, &values2).expect("push2");
        stream.push(&encoder1, &indices1, &values1).expect("push1");
        let data = stream.flush().expect("flush");
        assert!(!data.is_empty());

        // Decode
        let mut decoder1 = EntropyDecoder::<RansByte>::new();
        decoder1
            .initialize(&pmf_lengths1, &pmf_offsets1, &pmf_table1, 16, 4)
            .expect("dec1 init");
        let mut decoder2 = EntropyDecoder::<RansByte>::new();
        decoder2
            .initialize(&pmf_lengths2, &pmf_offsets2, &pmf_table2, 16, 4)
            .expect("dec2 init");

        let mut dstream = RansDecoderStream::<RansByte>::open_on(&data);

        let mut decoded1 = vec![0i32; values1.len()];
        dstream
            .decode(&decoder1, &mut decoded1, &indices1)
            .expect("decode1");
        assert_eq!(decoded1, values1);

        let mut decoded2 = vec![0i32; values2.len()];
        dstream
            .decode(&decoder2, &mut decoded2, &indices2)
            .expect("decode2");
        assert_eq!(decoded2, values2);

        dstream.decode_eof().expect("eof");
    }

    #[test]
    fn test_encoder_stream_multipart_64() {
        // Same multipart test with Rans64
        let pmf_lengths1 = vec![4, 6];
        let pmf_offsets1 = vec![1, 2];
        let pmf_table1 = vec![1, 3, 1, 1, 1, 3, 5, 3, 1, 1];
        let values1 = vec![-2, 1, 0, 1];
        let indices1 = vec![0, 1, 0, 1];

        let pmf_lengths2 = vec![5];
        let pmf_offsets2 = vec![1];
        let pmf_table2 = vec![1, 3, 3, 1, 1];
        let values2 = vec![-2, 1, 2];
        let indices2 = vec![0, 0, 0];

        let mut encoder1 = EntropyEncoder::<Rans64>::new();
        encoder1
            .initialize(&pmf_lengths1, &pmf_offsets1, &pmf_table1, 16, 4)
            .expect("init1");
        let mut encoder2 = EntropyEncoder::<Rans64>::new();
        encoder2
            .initialize(&pmf_lengths2, &pmf_offsets2, &pmf_table2, 16, 4)
            .expect("init2");

        let mut stream = RansEncoderStream::<Rans64>::new();
        stream.push(&encoder2, &indices2, &values2).expect("push2");
        stream.push(&encoder1, &indices1, &values1).expect("push1");
        let data = stream.flush().expect("flush");
        assert!(!data.is_empty());
        assert_eq!(data.len() % 4, 0, "Rans64 stream must be 4-byte aligned");

        let mut decoder1 = EntropyDecoder::<Rans64>::new();
        decoder1
            .initialize(&pmf_lengths1, &pmf_offsets1, &pmf_table1, 16, 4)
            .expect("dec1 init");
        let mut decoder2 = EntropyDecoder::<Rans64>::new();
        decoder2
            .initialize(&pmf_lengths2, &pmf_offsets2, &pmf_table2, 16, 4)
            .expect("dec2 init");

        let mut dstream = RansDecoderStream::<Rans64>::open_on(&data);

        let mut decoded1 = vec![0i32; values1.len()];
        dstream
            .decode(&decoder1, &mut decoded1, &indices1)
            .expect("decode1");
        assert_eq!(decoded1, values1);

        let mut decoded2 = vec![0i32; values2.len()];
        dstream
            .decode(&decoder2, &mut decoded2, &indices2)
            .expect("decode2");
        assert_eq!(decoded2, values2);

        dstream.decode_eof().expect("eof");
    }

    #[test]
    fn test_encoder_stream_reuse_after_flush() {
        let pmf_lengths = vec![4, 6];
        let pmf_offsets = vec![1, 2];
        let pmf_table = vec![1, 3, 1, 1, 1, 3, 5, 3, 1, 1];
        let values = vec![-2, 1, 0, 1];
        let indices = vec![0, 1, 0, 1];

        let mut encoder = EntropyEncoder::<RansByte>::new();
        encoder
            .initialize(&pmf_lengths, &pmf_offsets, &pmf_table, 16, 4)
            .expect("init");

        let mut stream = RansEncoderStream::<RansByte>::new();
        stream.push(&encoder, &indices, &values).expect("push1");
        let data1 = stream.flush().expect("flush1");

        // Reuse the stream for a second message
        stream.push(&encoder, &indices, &values).expect("push2");
        let data2 = stream.flush().expect("flush2");

        let mut decoder = EntropyDecoder::<RansByte>::new();
        decoder
            .initialize(&pmf_lengths, &pmf_offsets, &pmf_table, 16, 4)
            .expect("dec init");

        for data in [&data1, &data2] {
            let mut decoded = vec![0i32; values.len()];
            decoder
                .decode(&mut decoded, &indices, data)
                .expect("decode");
            assert_eq!(decoded, values);
        }
    }

    #[test]
    fn test_encoder_stream_reset_aborts() {
        let pmf_lengths = vec![4, 6];
        let pmf_offsets = vec![1, 2];
        let pmf_table = vec![1, 3, 1, 1, 1, 3, 5, 3, 1, 1];
        let values = vec![-2, 1, 0, 1];
        let indices = vec![0, 1, 0, 1];

        let mut encoder = EntropyEncoder::<RansByte>::new();
        encoder
            .initialize(&pmf_lengths, &pmf_offsets, &pmf_table, 16, 4)
            .expect("init");

        let mut stream = RansEncoderStream::<RansByte>::new();
        stream.push(&encoder, &indices, &values).expect("push");
        stream.reset();
        assert!(!stream.is_initialized(), "reset must clear state");
        // flush after reset → no data → error
        assert!(stream.flush().is_err());
    }

    #[test]
    fn test_decoder_stream_lifecycle() {
        let mut stream = RansDecoderStream::<RansByte>::new();
        assert!(!stream.is_open());
        stream.open(&[1, 2, 3, 4, 5]);
        assert!(stream.is_open());
        assert!(!stream.check_eof());
        stream.close();
        assert!(!stream.is_open());
        assert!(stream.check_eof());
    }

    #[test]
    fn test_units_to_le_bytes() {
        let units = [0x01020304u32, 0x05060708];
        let bytes = <Rans64 as EncoderVariantForS>::units_to_bytes(units.to_vec());
        assert_eq!(bytes, vec![4, 3, 2, 1, 8, 7, 6, 5]);
    }
}
