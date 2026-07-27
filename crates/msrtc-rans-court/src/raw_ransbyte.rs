// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # MSRTC.RAW.RANSBYTE — Differential courtroom
//!
//! Compares native Rust encoder output against oracle reference bitstreams.

use crate::{Court, CourtResult};

/// The RansByte differential court.
pub struct RansByteCourt;

impl Court for RansByteCourt {
    fn id(&self) -> &str {
        "MSRTC.RAW.RANSBYTE"
    }

    fn run(&self) -> CourtResult {
        CourtResult::scaffold(self.id())
    }
}

/// Oracle reference bitstream for RansByte with standard test inputs.
/// Generated from Microsoft C++ oracle at commit 0500356a.
pub const ORACLE_RANSBYTE_REF: &[u8] = &[
    0x05, 0x00, 0xbd, 0x04, 0x00, 0x01, 0xa1, 0x00, 0x03, 0x00, 0x0b, 0x00,
];

/// Oracle reference bitstream for Rans64.
pub const ORACLE_RANS64_REF: &[u8] = &[
    0x05, 0x00, 0xa1, 0xbd, 0x04, 0x00, 0x00, 0x00, 0x11, 0x0a, 0x00, 0x2f, 0x03, 0x00, 0x03, 0x00,
];

#[cfg(test)]
mod tests {
    use msrtc_rans_core::sink::{Sink, VecSink};
    use msrtc_rans_core::source::SliceSource;
    use msrtc_rans_core::{RansByteDecoder, RansByteEncSymbol, RansByteEncoder};

    // -----------------------------------------------------------------------
    // Self-consistency tests (not parity — no oracle comparison)
    // -----------------------------------------------------------------------

    #[test]
    fn test_ransbyte_raw_roundtrip_self_consistent() {
        let sink = VecSink::<u8>::new(64);
        let mut encoder = RansByteEncoder::new(sink);

        encoder.put_raw(0, 128, 8);
        encoder.flush();
        let encoded = encoder.into_sink().encoded().to_vec();

        let source = SliceSource::new(&encoded[..]);
        let mut decoder = RansByteDecoder::new(source);
        assert!(decoder.init(), "decoder must init successfully");
        assert!(decoder.advance(0, 128, 8), "advance must succeed");
        assert!(decoder.check_eof(), "must be at EOF");
    }

    #[test]
    fn test_ransbyte_prepared_raw_match() {
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

        assert_eq!(out1, out2, "Prepared symbol must match raw division path");
    }

    // -----------------------------------------------------------------------
    // Truncated-stream tests — decoder must not commit partial state
    // -----------------------------------------------------------------------

    #[test]
    fn test_ransbyte_init_rejects_truncated_3_bytes() {
        // RansByte Init needs 4 bytes. 3 bytes should fail.
        let data = [0x00u8, 0x00, 0x00];
        let source = SliceSource::new(&data[..]);
        let mut decoder = RansByteDecoder::new(source);
        assert!(!decoder.init(), "init with 3 bytes must fail");
        // State must remain at LowerBound (not partially updated)
        assert_eq!(
            decoder.state(),
            1u32 << 23,
            "state must remain at LowerBound after failed init"
        );
    }

    #[test]
    fn test_ransbyte_init_rejects_truncated_2_bytes() {
        let data = [0x00u8, 0x00];
        let source = SliceSource::new(&data[..]);
        let mut decoder = RansByteDecoder::new(source);
        assert!(!decoder.init());
        assert_eq!(decoder.state(), 1u32 << 23);
    }

    #[test]
    fn test_ransbyte_init_rejects_truncated_1_byte() {
        let data = [0x00u8];
        let source = SliceSource::new(&data[..]);
        let mut decoder = RansByteDecoder::new(source);
        assert!(!decoder.init());
        assert_eq!(decoder.state(), 1u32 << 23);
    }

    #[test]
    fn test_ransbyte_init_rejects_empty() {
        let data: [u8; 0] = [];
        let source = SliceSource::new(&data[..]);
        let mut decoder = RansByteDecoder::new(source);
        assert!(!decoder.init());
        assert_eq!(decoder.state(), 1u32 << 23);
    }

    #[test]
    fn test_ransbyte_init_rejects_below_lower_bound() {
        // Initial state below LowerBound (1<<23 = 8388608) should be rejected.
        // Construct a 4-byte sequence that reconstructs to a state < LowerBound.
        let data = [0x00u8, 0x00, 0x00, 0x00]; // state = 0 < LowerBound
        let source = SliceSource::new(&data[..]);
        let mut decoder = RansByteDecoder::new(source);
        assert!(!decoder.init(), "state below LowerBound must be rejected");
    }

    #[test]
    fn test_ransbyte_advance_transactional_on_truncated_renormalization() {
        // Encode with a frequency that forces renormalization (state >= x_max).
        // Then truncate the stream and verify advance fails without committing state.
        let sink = VecSink::<u8>::new(64);
        let mut encoder = RansByteEncoder::new(sink);

        // Use freq=1 (minimum) to force the encoder to renormalize aggressively,
        // which means the decoder will need to read more units on advance.
        encoder.put_raw(0, 1, 8); // freq=1 means scale=256, max output expansion
        encoder.flush();
        let encoded = encoder.into_sink().encoded().to_vec();
        assert!(encoded.len() > 4, "freq=1 should produce more than 4 bytes");

        // Truncate the stream to exactly the 4-byte initial state (no renormalization data)
        let truncated = &encoded[..4];

        let source = SliceSource::new(truncated);
        let mut decoder = RansByteDecoder::new(source);
        assert!(decoder.init(), "4 bytes should be enough for init");

        let state_before = decoder.state();
        // Advance should fail because there's no renormalization data in the buffer
        assert!(
            !decoder.advance(0, 1, 8),
            "advance with truncated stream must fail"
        );
        // State must NOT have been committed
        assert_eq!(
            decoder.state(),
            state_before,
            "state must not change on failed advance"
        );
    }

    #[test]
    fn test_ransbyte_advance_succeeds_with_full_stream() {
        // Same freq=1 encoding but with the full stream
        let sink = VecSink::<u8>::new(64);
        let mut encoder = RansByteEncoder::new(sink);
        encoder.put_raw(0, 1, 8);
        encoder.flush();
        let encoded = encoder.into_sink().encoded().to_vec();

        let source = SliceSource::new(&encoded[..]);
        let mut decoder = RansByteDecoder::new(source);
        assert!(decoder.init());

        // With the full stream, advance must succeed
        assert!(
            decoder.advance(0, 1, 8),
            "advance with full stream must succeed"
        );
    }

    // -----------------------------------------------------------------------
    // Growth boundary tests for VecSink (regression: was corrupting output)
    // These test the sink directly (encoder produces units sporadically;
    // the correct way to test growth is via the sink interface).

    #[test]
    fn test_encoder_growth_preserves_all_data_65_writes() {
        // 64 initial capacity → 65th triggers growth
        let mut sink = VecSink::<u8>::with_exact_capacity(64);
        for i in 0..65u16 {
            sink.write((i % 256) as u8);
        }
        let encoded = sink.encoded();
        assert_eq!(encoded.len(), 65, "all 65 writes must be present");
        for i in 0..65u16 {
            assert_eq!(
                encoded[(64 - i) as usize],
                (i % 256) as u8,
                "mismatch at {}",
                i
            );
        }
    }

    #[test]
    fn test_encoder_growth_preserves_all_data_321_writes() {
        let mut sink = VecSink::<u8>::with_exact_capacity(320);
        for i in 0..321u16 {
            sink.write((i % 256) as u8);
        }
        let encoded = sink.encoded();
        assert_eq!(encoded.len(), 321, "all 321 writes must be present");
        for i in 0..321u16 {
            assert_eq!(
                encoded[(320 - i) as usize],
                (i % 256) as u8,
                "mismatch at {}",
                i
            );
        }
    }

    #[test]
    fn test_encoder_growth_preserves_all_data_1000_writes() {
        // Force multiple growth cycles
        let mut sink = VecSink::<u8>::with_exact_capacity(64);
        for i in 0..1000u16 {
            sink.write((i % 256) as u8);
        }
        let encoded = sink.encoded();
        assert_eq!(encoded.len(), 1000, "all 1000 writes must be present");
        // Verify every byte
        for i in 0..1000u16 {
            assert_eq!(
                encoded[(999 - i) as usize],
                (i % 256) as u8,
                "mismatch at {}",
                i
            );
        }
    }
}
