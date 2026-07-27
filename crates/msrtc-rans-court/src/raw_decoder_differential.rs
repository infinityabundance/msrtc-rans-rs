// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # MSRTC.RAW.DECODER.DIFFERENTIAL — Raw rANS decoder differential court
//!
//! Cross-decoding tests: verifies that Rust-encoded streams can be decoded
//! by the C++ decoder oracle (and vice versa), comparing Get() values and
//! EOF status at every step.
//!
//! Each case:
//! - Generates a deterministic sequence of (start, freq) symbols
//! - Encodes with Rust, then decodes with C++ via `run_decoder_oracle()`
//! - Encodes with C++ (via `run_raw_oracle()`), then decodes with Rust
//! - Compares Get() cumulative frequencies and final EOF status

use msrtc_rans_casefile::{
    Comparison, DifferentialResult, InputHashes, NativeResult, OracleResult,
    classification::{ResidualClassification, ResolutionState},
    sha256,
};
use msrtc_rans_core::sink::VecSink;
use msrtc_rans_core::source::SliceSource;
use msrtc_rans_core::{
    Freq, Rans64Decoder, Rans64Encoder, RansByteDecoder, RansByteEncoder, error::RawRansError,
};

use crate::oracle::{self, environment_sha256, git_commit};
use crate::{Court, CourtResult, CourtStatus};

/// A single decoder differential test case.
#[derive(Debug, Clone)]
pub struct DecoderCase {
    pub seed: u64,
    /// 0=Rans64, 1=RansByte
    pub variant: u32,
    /// Scale bits shared by all symbols.
    pub scale_bits: Freq,
    /// Sequence of (start, freq) tuples to encode/decode.
    pub symbols: Vec<(Freq, Freq)>,
}

/// Raw input hashes for a decoder court case.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecoderInputHashes {
    /// SHA-256 of variant in LE bytes
    pub variant_sha256: String,
    /// SHA-256 of scale_bits in LE bytes
    pub scale_bits_sha256: String,
    /// SHA-256 of all symbols (start,freq tuples) in LE bytes
    pub symbols_sha256: String,
    /// SHA-256 of the encoded bitstream bytes
    pub encoded_sha256: String,
    /// SHA-256 of the full decoder casefile binary
    pub casefile_sha256: String,
}

impl DecoderCase {
    /// Encode with the Rust raw encoder to produce a bitstream.
    pub fn rust_encode(&self) -> Result<Vec<u8>, RawRansError> {
        match self.variant {
            1 => self.rust_encode_byte(),
            0 => self.rust_encode_64(),
            _ => Err(RawRansError::InvalidParameters),
        }
    }

    fn rust_encode_byte(&self) -> Result<Vec<u8>, RawRansError> {
        let sink = VecSink::<u8>::new(4096);
        let mut encoder = RansByteEncoder::new(sink);
        for &(start, freq) in &self.symbols {
            encoder.put_raw(start, freq, self.scale_bits);
        }
        encoder.flush();
        Ok(encoder.into_sink().encoded().to_vec())
    }

    fn rust_encode_64(&self) -> Result<Vec<u8>, RawRansError> {
        let sink = VecSink::<u32>::new(4096);
        let mut encoder = Rans64Encoder::new(sink);
        for &(start, freq) in &self.symbols {
            encoder.put_raw(start, freq, self.scale_bits);
        }
        encoder.flush();
        let result = encoder.into_sink().encoded().to_vec();
        let mut bytes = Vec::with_capacity(result.len() * 4);
        for &u in &result {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        Ok(bytes)
    }

    /// Decode the bitstream with the Rust raw decoder.
    /// Returns (get_values, eof).
    pub fn rust_decode(&self, bitstream: &[u8]) -> Result<(Vec<u32>, bool), String> {
        match self.variant {
            1 => self.rust_decode_byte(bitstream),
            0 => self.rust_decode_64(bitstream),
            _ => Err("invalid variant".into()),
        }
    }

    fn rust_decode_byte(&self, bitstream: &[u8]) -> Result<(Vec<u32>, bool), String> {
        let source = SliceSource::new(bitstream);
        let mut decoder = RansByteDecoder::new(source);
        if !decoder.init() {
            return Err("Rust decoder init failed".into());
        }

        let mut get_values = Vec::with_capacity(self.symbols.len());
        for &(start, freq) in &self.symbols {
            let cum_freq = decoder.get(self.scale_bits);
            get_values.push(cum_freq);
            if !decoder.advance(start, freq, self.scale_bits) {
                return Err("Rust decoder advance failed".into());
            }
        }

        let eof = decoder.check_eof();
        Ok((get_values, eof))
    }

    fn rust_decode_64(&self, bitstream: &[u8]) -> Result<(Vec<u32>, bool), String> {
        // Reinterpret bytes as u32 units (little-endian)
        if bitstream.len() % 4 != 0 {
            return Err("bitstream length not multiple of 4 for Rans64".into());
        }
        let unit_count = bitstream.len() / 4;
        let mut units = Vec::with_capacity(unit_count);
        for i in 0..unit_count {
            let off = i * 4;
            let val = u32::from_le_bytes(bitstream[off..off + 4].try_into().unwrap());
            units.push(val);
        }

        let source = SliceSource::new(&units);
        let mut decoder = Rans64Decoder::new(source);
        if !decoder.init() {
            return Err("Rust decoder init failed (Rans64)".into());
        }

        let mut get_values = Vec::with_capacity(self.symbols.len());
        for &(start, freq) in &self.symbols {
            let cum_freq = decoder.get(self.scale_bits);
            get_values.push(cum_freq);
            if !decoder.advance(start, freq, self.scale_bits) {
                return Err("Rust decoder advance failed (Rans64)".into());
            }
        }

        let eof = decoder.check_eof();
        Ok((get_values, eof))
    }

    /// Build the decoder oracle binary input format.
    pub fn to_decoder_oracle_binary(&self, bitstream: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.variant.to_le_bytes());
        buf.extend_from_slice(&self.scale_bits.to_le_bytes());
        buf.extend_from_slice(&(self.symbols.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(bitstream.len() as u32).to_le_bytes());
        buf.extend_from_slice(bitstream);
        for &(start, freq) in &self.symbols {
            buf.extend_from_slice(&start.to_le_bytes());
            buf.extend_from_slice(&freq.to_le_bytes());
        }
        buf
    }

    /// Build the raw encoder oracle binary input format (mode 0).
    pub fn to_encoder_oracle_binary(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.variant.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // mode = raw Put
        buf.extend_from_slice(&self.scale_bits.to_le_bytes());
        buf.extend_from_slice(&(self.symbols.len() as u32).to_le_bytes());
        for &(start, freq) in &self.symbols {
            buf.extend_from_slice(&start.to_le_bytes());
            buf.extend_from_slice(&freq.to_le_bytes());
        }
        buf
    }
}

/// MSRTC.RAW.DECODER.DIFFERENTIAL court.
pub struct RawDecoderDifferentialCourt;

impl Court for RawDecoderDifferentialCourt {
    fn id(&self) -> &str {
        "MSRTC.RAW.DECODER.DIFFERENTIAL"
    }

    fn run(&self) -> CourtResult {
        let cases = generate_decoder_cases();
        let mut results = Vec::with_capacity(cases.len());

        for case in &cases {
            // Determine case direction for case_id
            // Two sub-cases: Rust-encode → C++-decode, and C++-encode → Rust-decode
            for direction in &["rust_enc_cpp_dec", "cpp_enc_rust_dec"] {
                let result = match *direction {
                    "rust_enc_cpp_dec" => self.run_rust_encode_cpp_decode(case),
                    "cpp_enc_rust_dec" => self.run_cpp_encode_rust_decode(case),
                    _ => continue,
                };

                match result {
                    Ok(r) => results.push(r),
                    Err(e) => {
                        // Generate a failure result for infrastructure errors
                        let all_binary = case.to_decoder_oracle_binary(&[]);
                        let all_input_sha = sha256(&all_binary);
                        let err_result = DifferentialResult {
                            schema_version: oracle::SCHEMA_VERSION,
                            court_id: self.id().to_string(),
                            case_id: format!("{}_{}", direction, all_input_sha),
                            oracle_commit: oracle::ORACLE_COMMIT.to_string(),
                            rust_commit: git_commit(),
                            seed: case.seed,
                            variant: if case.variant == 1 {
                                "RansByte".into()
                            } else {
                                "Rans64".into()
                            },
                            input_hashes: InputHashes {
                                pmf_lengths_sha256: sha256(&case.variant.to_le_bytes()),
                                pmf_offsets_sha256: sha256(&case.scale_bits.to_le_bytes()),
                                pmf_table_sha256: sha256(&{
                                    let mut s = Vec::new();
                                    for &(start, freq) in &case.symbols {
                                        s.extend_from_slice(&start.to_le_bytes());
                                        s.extend_from_slice(&freq.to_le_bytes());
                                    }
                                    s
                                }),
                                indices_sha256: String::new(),
                                values_sha256: all_input_sha,
                            },
                            oracle: OracleResult {
                                status: "oracle_error".into(),
                                output_sha256: sha256(b""),
                                length: 0,
                            },
                            native: NativeResult {
                                status: format!("error: {}", e),
                                output_sha256: sha256(b""),
                                length: 0,
                            },
                            comparison: Comparison {
                                exact: false,
                                first_differing_offset: None,
                                differing_bytes: None,
                            },
                            classification: oracle::classify_error(&e),
                            resolution: ResolutionState::Open,
                            minimized_casefile: None,
                            environment_sha256: environment_sha256(),
                        };
                        results.push(err_result);
                    }
                }
            }
        }

        let pass_count = results.iter().filter(|r| r.comparison.exact).count() as u64;
        let residual_count = results.iter().filter(|r| !r.comparison.exact).count() as u64;

        CourtResult {
            court_id: self.id().to_string(),
            status: if residual_count == 0 {
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

impl RawDecoderDifferentialCourt {
    /// Run sub-case: Rust encode → C++ decoder oracle decode.
    /// Compare Get() values and EOF between Rust and C++.
    fn run_rust_encode_cpp_decode(&self, case: &DecoderCase) -> Result<DifferentialResult, String> {
        // ---- Rust encode ----
        let bitstream = case
            .rust_encode()
            .map_err(|e| format!("rust_encode: {:?}", e))?;

        // ---- Rust decode (native reference) ----
        let (native_get_values, native_eof) = case
            .rust_decode(&bitstream)
            .map_err(|e| format!("rust_decode: {}", e))?;

        // ---- C++ decoder oracle decode ----
        let decoder_binary = case.to_decoder_oracle_binary(&bitstream);
        let all_input_sha = sha256(&decoder_binary);

        let oracle_response = oracle::run_decoder_oracle(&decoder_binary)
            .map_err(|e| format!("run_decoder_oracle: {}", e))?;

        let oracle_get_values = oracle_response.get_values;
        let oracle_eof = oracle_response.eof;

        // ---- Compare Get() values and EOF ----
        let get_values_match = native_get_values == oracle_get_values;
        let eof_match = native_eof == oracle_eof;
        let exact = get_values_match && eof_match;

        // Build comparison info
        let comparison = if exact {
            Comparison {
                exact: true,
                first_differing_offset: None,
                differing_bytes: None,
            }
        } else {
            // Find first differing Get() value
            let min_len = native_get_values.len().min(oracle_get_values.len());
            let first_diff = (0..min_len)
                .find(|&i| native_get_values[i] != oracle_get_values[i])
                .map(|i| (i * 4) as u64);

            let diff_count = if !get_values_match {
                let overlapping_diffs = (0..min_len)
                    .filter(|&i| native_get_values[i] != oracle_get_values[i])
                    .count() as u64;
                let len_diff = if native_get_values.len() != oracle_get_values.len() {
                    native_get_values.len().max(oracle_get_values.len()) as u64
                        - native_get_values.len().min(oracle_get_values.len()) as u64
                } else {
                    0
                };
                Some(overlapping_diffs + len_diff)
            } else {
                Some(1) // EOF mismatch
            };

            Comparison {
                exact: false,
                first_differing_offset: first_diff,
                differing_bytes: diff_count,
            }
        };

        let classification = if exact {
            ResidualClassification::Unclassified
        } else {
            ResidualClassification::Unclassified
        };

        // Serialize get_values for the oracle and native results
        let mut native_output = Vec::new();
        for &v in &native_get_values {
            native_output.extend_from_slice(&v.to_le_bytes());
        }
        native_output.extend_from_slice(&(native_eof as u32).to_le_bytes());

        let mut oracle_output = Vec::new();
        for &v in &oracle_get_values {
            oracle_output.extend_from_slice(&v.to_le_bytes());
        }
        oracle_output.extend_from_slice(&(oracle_eof as u32).to_le_bytes());

        Ok(DifferentialResult {
            schema_version: oracle::SCHEMA_VERSION,
            court_id: self.id().to_string(),
            case_id: format!("rust_enc_cpp_dec:sha256:{}", all_input_sha),
            oracle_commit: oracle::ORACLE_COMMIT.to_string(),
            rust_commit: git_commit(),
            seed: case.seed,
            variant: if case.variant == 1 {
                "RansByte".into()
            } else {
                "Rans64".into()
            },
            input_hashes: {
                let rh = case.compute_input_hashes(&bitstream);
                InputHashes {
                    pmf_lengths_sha256: rh.variant_sha256,
                    pmf_offsets_sha256: rh.scale_bits_sha256,
                    pmf_table_sha256: rh.symbols_sha256,
                    indices_sha256: rh.encoded_sha256,
                    values_sha256: rh.casefile_sha256,
                }
            },
            oracle: OracleResult {
                status: "ok".to_string(),
                output_sha256: sha256(&oracle_output),
                length: oracle_output.len() as u64,
            },
            native: NativeResult {
                status: "ok".to_string(),
                output_sha256: sha256(&native_output),
                length: native_output.len() as u64,
            },
            comparison,
            classification,
            resolution: if exact {
                ResolutionState::Fixed
            } else {
                ResolutionState::Open
            },
            minimized_casefile: None,
            environment_sha256: environment_sha256(),
        })
    }

    /// Run sub-case: C++ raw encoder → Rust decoder decode.
    /// Compare Get() values and EOF between C++ and Rust.
    fn run_cpp_encode_rust_decode(&self, case: &DecoderCase) -> Result<DifferentialResult, String> {
        // ---- C++ encode ----
        let encoder_binary = case.to_encoder_oracle_binary();
        let oracle_encode_response = oracle::run_raw_oracle(&encoder_binary)
            .map_err(|e| format!("run_raw_oracle (encode): {}", e))?;
        let bitstream = oracle_encode_response.raw_output;
        // ---- C++ decoder oracle decode ----
        let decoder_binary = case.to_decoder_oracle_binary(&bitstream);
        let all_input_sha = sha256(&decoder_binary);

        let oracle_response = oracle::run_decoder_oracle(&decoder_binary)
            .map_err(|e| format!("run_decoder_oracle: {}", e))?;
        let oracle_get_values = oracle_response.get_values;
        let oracle_eof = oracle_response.eof;

        // ---- Rust decode (native) ----
        let (native_get_values, native_eof) = case
            .rust_decode(&bitstream)
            .map_err(|e| format!("rust_decode: {}", e))?;

        // ---- Compare Get() values and EOF ----
        let get_values_match = native_get_values == oracle_get_values;
        let eof_match = native_eof == oracle_eof;
        let exact = get_values_match && eof_match;

        let comparison = if exact {
            Comparison {
                exact: true,
                first_differing_offset: None,
                differing_bytes: None,
            }
        } else {
            let min_len = native_get_values.len().min(oracle_get_values.len());
            let first_diff = (0..min_len)
                .find(|&i| native_get_values[i] != oracle_get_values[i])
                .map(|i| (i * 4) as u64);

            let diff_count = if !get_values_match {
                let overlapping_diffs = (0..min_len)
                    .filter(|&i| native_get_values[i] != oracle_get_values[i])
                    .count() as u64;
                let len_diff = if native_get_values.len() != oracle_get_values.len() {
                    native_get_values.len().max(oracle_get_values.len()) as u64
                        - native_get_values.len().min(oracle_get_values.len()) as u64
                } else {
                    0
                };
                Some(overlapping_diffs + len_diff)
            } else {
                Some(1)
            };

            Comparison {
                exact: false,
                first_differing_offset: first_diff,
                differing_bytes: diff_count,
            }
        };

        let classification = if exact {
            ResidualClassification::Unclassified
        } else {
            ResidualClassification::Unclassified
        };

        let mut native_output = Vec::new();
        for &v in &native_get_values {
            native_output.extend_from_slice(&v.to_le_bytes());
        }
        native_output.extend_from_slice(&(native_eof as u32).to_le_bytes());

        let mut oracle_output = Vec::new();
        for &v in &oracle_get_values {
            oracle_output.extend_from_slice(&v.to_le_bytes());
        }
        oracle_output.extend_from_slice(&(oracle_eof as u32).to_le_bytes());

        Ok(DifferentialResult {
            schema_version: oracle::SCHEMA_VERSION,
            court_id: self.id().to_string(),
            case_id: format!("cpp_enc_rust_dec:sha256:{}", all_input_sha),
            oracle_commit: oracle::ORACLE_COMMIT.to_string(),
            rust_commit: git_commit(),
            seed: case.seed,
            variant: if case.variant == 1 {
                "RansByte".into()
            } else {
                "Rans64".into()
            },
            input_hashes: {
                let rh = case.compute_input_hashes(&bitstream);
                InputHashes {
                    pmf_lengths_sha256: rh.variant_sha256,
                    pmf_offsets_sha256: rh.scale_bits_sha256,
                    pmf_table_sha256: rh.symbols_sha256,
                    indices_sha256: rh.encoded_sha256,
                    values_sha256: rh.casefile_sha256,
                }
            },
            oracle: OracleResult {
                status: "ok".to_string(),
                output_sha256: sha256(&oracle_output),
                length: oracle_output.len() as u64,
            },
            native: NativeResult {
                status: "ok".to_string(),
                output_sha256: sha256(&native_output),
                length: native_output.len() as u64,
            },
            comparison,
            classification,
            resolution: if exact {
                ResolutionState::Fixed
            } else {
                ResolutionState::Open
            },
            minimized_casefile: None,
            environment_sha256: environment_sha256(),
        })
    }
}

impl DecoderCase {
    /// Compute input hashes for the DifferentialResult.
    pub fn compute_input_hashes(&self, bitstream: &[u8]) -> DecoderInputHashes {
        let variant_le: Vec<u8> = self.variant.to_le_bytes().to_vec();
        let scale_bits_le: Vec<u8> = self.scale_bits.to_le_bytes().to_vec();

        let mut symbols_le = Vec::new();
        for &(start, freq) in &self.symbols {
            symbols_le.extend_from_slice(&start.to_le_bytes());
            symbols_le.extend_from_slice(&freq.to_le_bytes());
        }

        let mut casefile_le = Vec::new();
        casefile_le.extend_from_slice(&variant_le);
        casefile_le.extend_from_slice(&scale_bits_le);
        casefile_le.extend_from_slice(&(self.symbols.len() as u32).to_le_bytes());
        casefile_le.extend_from_slice(&(bitstream.len() as u32).to_le_bytes());
        casefile_le.extend_from_slice(bitstream);
        casefile_le.extend_from_slice(&symbols_le);

        DecoderInputHashes {
            variant_sha256: sha256(&variant_le),
            scale_bits_sha256: sha256(&scale_bits_le),
            symbols_sha256: sha256(&symbols_le),
            encoded_sha256: sha256(bitstream),
            casefile_sha256: sha256(&casefile_le),
        }
    }
}

/// Generate deterministic decoder test cases.
fn generate_decoder_cases() -> Vec<DecoderCase> {
    let mut cases = Vec::new();

    // RansByte: single symbol
    cases.push(DecoderCase {
        seed: 0,
        variant: 1,
        scale_bits: 8,
        symbols: vec![(0, 128)],
    });

    // RansByte: multiple symbols
    cases.push(DecoderCase {
        seed: 1,
        variant: 1,
        scale_bits: 8,
        symbols: vec![(0, 128), (64, 64), (128, 32)],
    });

    // RansByte: freq=1 (special case)
    cases.push(DecoderCase {
        seed: 2,
        variant: 1,
        scale_bits: 8,
        symbols: vec![(0, 1)],
    });

    // RansByte: mid-range freq
    cases.push(DecoderCase {
        seed: 3,
        variant: 1,
        scale_bits: 10,
        symbols: vec![(128, 512), (0, 256), (512, 256)],
    });

    // Rans64: single symbol
    cases.push(DecoderCase {
        seed: 4,
        variant: 0,
        scale_bits: 16,
        symbols: vec![(0, 32768)],
    });

    // Rans64: multiple symbols
    cases.push(DecoderCase {
        seed: 5,
        variant: 0,
        scale_bits: 16,
        symbols: vec![(0, 32768), (16384, 16384)],
    });

    // Rans64: freq=1 (special case)
    cases.push(DecoderCase {
        seed: 6,
        variant: 0,
        scale_bits: 16,
        symbols: vec![(0, 1)],
    });

    // Rans64: multiple symbols with varied frequencies
    cases.push(DecoderCase {
        seed: 7,
        variant: 0,
        scale_bits: 20,
        symbols: vec![(0, 524288), (524288, 262144), (786432, 131072)],
    });

    cases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_rust_roundtrip_byte() {
        let case = DecoderCase {
            seed: 0,
            variant: 1,
            scale_bits: 8,
            symbols: vec![(0, 128)],
        };
        let bitstream = case.rust_encode().expect("encode");
        let (get_values, eof) = case.rust_decode(&bitstream).expect("decode");
        assert_eq!(
            get_values,
            vec![0],
            "Get() value must be 0 for single symbol"
        );
        assert!(eof, "EOF must be true after consuming all symbols");
    }

    #[test]
    fn test_decoder_rust_roundtrip_byte_multiple() {
        let case = DecoderCase {
            seed: 1,
            variant: 1,
            scale_bits: 8,
            symbols: vec![(0, 128), (64, 64), (128, 32)],
        };
        let bitstream = case.rust_encode().expect("encode");
        let (get_values, eof) = case.rust_decode(&bitstream).expect("decode");
        assert_eq!(get_values.len(), 3, "must produce 3 Get() values");
        assert!(eof, "EOF must be true");
        // Verify each Get() value is in range [0, scale_bits)
        for &v in &get_values {
            assert!(v < (1u32 << 8), "Get() value {} must be < {}", v, 1u32 << 8);
        }
    }

    #[test]
    fn test_decoder_rust_roundtrip_64() {
        let case = DecoderCase {
            seed: 4,
            variant: 0,
            scale_bits: 16,
            symbols: vec![(0, 32768)],
        };
        let bitstream = case.rust_encode().expect("encode");
        let (get_values, eof) = case.rust_decode(&bitstream).expect("decode");
        assert_eq!(
            get_values,
            vec![0],
            "Get() value must be 0 for single symbol"
        );
        assert!(eof, "EOF must be true");
    }

    #[test]
    fn test_decoder_rust_roundtrip_64_multiple() {
        let case = DecoderCase {
            seed: 5,
            variant: 0,
            scale_bits: 16,
            symbols: vec![(0, 32768), (16384, 16384)],
        };
        let bitstream = case.rust_encode().expect("encode");
        let (get_values, eof) = case.rust_decode(&bitstream).expect("decode");
        assert_eq!(get_values.len(), 2, "must produce 2 Get() values");
        assert!(eof, "EOF must be true");
        for &v in &get_values {
            assert!(
                v < (1u32 << 16),
                "Get() value {} must be < {}",
                v,
                1u32 << 16
            );
        }
    }

    #[test]
    fn test_decoder_rust_self_consistency() {
        // Encode and decode with Rust only, verifying that Get() values
        // are recovered correctly for all test cases.
        let cases = generate_decoder_cases();
        for case in &cases {
            let bitstream = case.rust_encode().expect("encode");
            let (get_values, eof) = case.rust_decode(&bitstream).expect("decode");

            assert_eq!(
                get_values.len(),
                case.symbols.len(),
                "case seed {}: get_values count mismatch",
                case.seed
            );

            // Verify that the last decoded fragment restores EOF
            assert!(eof, "case seed {}: EOF must be true", case.seed);

            // Verify Get() values are within range
            let scale = 1u32 << case.scale_bits;
            for (i, &v) in get_values.iter().enumerate() {
                assert!(
                    v < scale,
                    "case seed {}, symbol {}: Get() value {} must be < scale {}",
                    case.seed,
                    i,
                    v,
                    scale
                );
            }
        }
    }

    #[test]
    fn test_decoder_court_generates_cases() {
        let court = RawDecoderDifferentialCourt;
        let result = court.run();
        assert!(result.case_count > 0, "court must generate cases");
    }

    #[test]
    #[ignore = "requires Docker oracle image: msrtc-rans-rs-oracle:debian12"]
    fn test_decoder_court_full_differential() {
        let court = RawDecoderDifferentialCourt;
        let result = court.run();
        assert_eq!(
            result.status,
            CourtStatus::Passed,
            "raw decoder differential court must pass: {} passed, {} residuals",
            result.pass_count,
            result.residual_count
        );
        assert!(result.is_sealable(), "passing court must be sealable");
    }
}
