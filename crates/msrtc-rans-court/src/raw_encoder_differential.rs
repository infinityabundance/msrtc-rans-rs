// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # MSRTC.RAW.ENCODER.DIFFERENTIAL — Raw rANS encoder differential court
//!
//! Exercises Microsoft's raw `RansEncoder::Put`, `RansEncoder::Put(symbol)`,
//! and `RansEncoder::Flush` templates directly (no PMF, no bypass, no
//! distribution descriptors).
//!
//! Each case is a deterministic sequence of (start, freq, scale_bits) tuples.
//! The court encodes the same sequence with both C++ and Rust, compares the
//! output bitstream byte-for-byte, and writes a structured residual for any
//! mismatch.

use msrtc_rans_casefile::{
    Comparison, DifferentialResult, InputHashes, NativeResult, OracleResult,
    classification::{ResidualClassification, ResolutionState},
    sha256,
};
use msrtc_rans_core::sink::VecSink;
use msrtc_rans_core::{
    Freq, Rans64EncSymbol, Rans64Encoder, RansByteEncSymbol, RansByteEncoder, error::RawRansError,
};

use crate::oracle::{self, compare_bytes, environment_sha256, git_commit, try_write_residual};
use crate::{Court, CourtResult, CourtStatus};

/// A single raw primitive test case.
#[derive(Debug, Clone)]
pub struct RawCase {
    pub seed: u64,
    /// 0=Rans64, 1=RansByte
    pub variant: u32,
    /// 0=raw Put, 1=prepared Put(symbol)
    pub mode: u32,
    /// Scale bits shared by all symbols.
    pub scale_bits: Freq,
    /// Sequence of (start, freq) tuples.
    pub symbols: Vec<(Freq, Freq)>,
}

/// Raw input hashes for a raw encoder court case.
/// Uses per-field SHA-256 of canonical little-endian bytes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RawInputHashes {
    /// SHA-256 of variant in LE bytes
    pub variant_sha256: String,
    /// SHA-256 of mode in LE bytes
    pub mode_sha256: String,
    /// SHA-256 of scale_bits in LE bytes
    pub scale_bits_sha256: String,
    /// SHA-256 of all symbols (start,freq tuples) in LE bytes
    pub symbols_sha256: String,
    /// SHA-256 of the full casefile binary in LE bytes
    pub casefile_sha256: String,
}

impl RawCase {
    /// Serialize to the raw oracle CLI binary format.
    pub fn to_binary(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.variant.to_le_bytes());
        buf.extend_from_slice(&self.mode.to_le_bytes());
        buf.extend_from_slice(&self.scale_bits.to_le_bytes());
        buf.extend_from_slice(&(self.symbols.len() as u32).to_le_bytes());
        for &(start, freq) in &self.symbols {
            buf.extend_from_slice(&start.to_le_bytes());
            buf.extend_from_slice(&freq.to_le_bytes());
        }
        buf
    }

    /// Compute raw input hashes for the DifferentialResult.
    pub fn raw_input_hashes(&self) -> RawInputHashes {
        let variant_le: Vec<u8> = self.variant.to_le_bytes().to_vec();
        let mode_le: Vec<u8> = self.mode.to_le_bytes().to_vec();
        let scale_bits_le: Vec<u8> = self.scale_bits.to_le_bytes().to_vec();

        let mut symbols_le = Vec::new();
        for &(start, freq) in &self.symbols {
            symbols_le.extend_from_slice(&start.to_le_bytes());
            symbols_le.extend_from_slice(&freq.to_le_bytes());
        }

        let mut casefile_le = Vec::new();
        casefile_le.extend_from_slice(&variant_le);
        casefile_le.extend_from_slice(&mode_le);
        casefile_le.extend_from_slice(&scale_bits_le);
        casefile_le.extend_from_slice(&(self.symbols.len() as u32).to_le_bytes());
        casefile_le.extend_from_slice(&symbols_le);

        RawInputHashes {
            variant_sha256: sha256(&variant_le),
            mode_sha256: sha256(&mode_le),
            scale_bits_sha256: sha256(&scale_bits_le),
            symbols_sha256: sha256(&symbols_le),
            casefile_sha256: sha256(&casefile_le),
        }
    }

    /// Encode with the Rust raw encoder.
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

        match self.mode {
            0 => {
                for &(start, freq) in &self.symbols {
                    encoder.put_raw(start, freq, self.scale_bits);
                }
            }
            1 => {
                for &(start, freq) in &self.symbols {
                    let sym = RansByteEncSymbol::try_new(start, freq, self.scale_bits)?;
                    encoder.put(&sym);
                }
            }
            _ => return Err(RawRansError::InvalidParameters),
        }
        encoder.flush();
        Ok(encoder.into_sink().encoded().to_vec())
    }

    fn rust_encode_64(&self) -> Result<Vec<u8>, RawRansError> {
        let sink = VecSink::<u32>::new(4096);
        let mut encoder = Rans64Encoder::new(sink);

        match self.mode {
            0 => {
                for &(start, freq) in &self.symbols {
                    encoder.put_raw(start, freq, self.scale_bits);
                }
            }
            1 => {
                for &(start, freq) in &self.symbols {
                    let sym = Rans64EncSymbol::try_new(start, freq, self.scale_bits)?;
                    encoder.put(&sym);
                }
            }
            _ => return Err(RawRansError::InvalidParameters),
        }
        encoder.flush();
        let result = encoder.into_sink().encoded().to_vec();
        let mut bytes = Vec::with_capacity(result.len() * 4);
        for &u in &result {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        Ok(bytes)
    }
}

/// MSRTC.RAW.ENCODER.DIFFERENTIAL court.
pub struct RawEncoderDifferentialCourt;

impl Court for RawEncoderDifferentialCourt {
    fn id(&self) -> &str {
        "MSRTC.RAW.ENCODER.DIFFERENTIAL"
    }

    fn run(&self) -> CourtResult {
        let cases = generate_raw_cases();
        let mut results = Vec::with_capacity(cases.len());

        for case in &cases {
            let case_binary = case.to_binary();
            let all_input_sha = sha256(&case_binary);

            // ---- Rust encode ----
            let (native_status, native_output) = match case.rust_encode() {
                Ok(out) => ("ok".to_string(), out),
                Err(e) => (format!("native_error: {:?}", e), Vec::new()),
            };

            // ---- Oracle encode ----
            let oracle_result = oracle::run_raw_oracle(&case_binary);
            let (oracle_status, oracle_output, oracle_err) = match &oracle_result {
                Ok(resp) => ("ok".to_string(), resp.raw_output.clone(), None),
                Err(e) => ("oracle_error".to_string(), Vec::new(), Some(e.clone())),
            };

            // ---- Compare ----
            let exact = native_status == oracle_status && native_output == oracle_output;
            let comparison = compare_bytes(&native_output, &oracle_output);

            let classification = if exact {
                ResidualClassification::Unclassified
            } else if oracle_err.is_some() {
                oracle::classify_error(oracle_err.as_ref().unwrap())
            } else if native_status != "ok" {
                ResidualClassification::NativeBug
            } else {
                ResidualClassification::Unclassified
            };

            let result = DifferentialResult {
                schema_version: oracle::SCHEMA_VERSION,
                court_id: self.id().to_string(),
                case_id: format!("sha256:{}", all_input_sha),
                oracle_commit: oracle::ORACLE_COMMIT.to_string(),
                rust_commit: git_commit(),
                seed: case.seed,
                variant: if case.variant == 1 {
                    "RansByte".into()
                } else {
                    "Rans64".into()
                },
                input_hashes: {
                    let rh = case.raw_input_hashes();
                    InputHashes {
                        pmf_lengths_sha256: rh.variant_sha256,
                        pmf_offsets_sha256: rh.mode_sha256,
                        pmf_table_sha256: rh.scale_bits_sha256,
                        indices_sha256: rh.symbols_sha256,
                        values_sha256: rh.casefile_sha256,
                    }
                },
                oracle: OracleResult {
                    status: oracle_status,
                    output_sha256: sha256(&oracle_output),
                    length: oracle_output.len() as u64,
                },
                native: NativeResult {
                    status: native_status,
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
            };

            // Persist residuals
            if !exact {
                if let Err(_e) = try_write_residual(&result) {
                    let failure_result = DifferentialResult {
                        case_id: format!("residual_persistence_failure:{}", result.case_id),
                        classification: ResidualClassification::Environmental,
                        comparison: Comparison {
                            exact: false,
                            first_differing_offset: None,
                            differing_bytes: None,
                        },
                        ..result.clone()
                    };
                    // Overwrite result with the failure so the court count is accurate
                    results.push(failure_result);
                    continue;
                }
            }

            results.push(result);
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

/// Generate deterministic raw primitive test cases.
///
/// 8 hand-picked boundary cases + a seeded LCG sweep over variants, modes,
/// and scale widths → ~112 cases.
fn generate_raw_cases() -> Vec<RawCase> {
    let mut cases = Vec::new();

    // RansByte: single symbol, raw mode
    cases.push(RawCase {
        seed: 0,
        variant: 1,
        mode: 0,
        scale_bits: 8,
        symbols: vec![(0, 128)],
    });

    // RansByte: single symbol, prepared mode
    cases.push(RawCase {
        seed: 1,
        variant: 1,
        mode: 1,
        scale_bits: 8,
        symbols: vec![(0, 128)],
    });

    // RansByte: multiple symbols, raw mode
    cases.push(RawCase {
        seed: 2,
        variant: 1,
        mode: 0,
        scale_bits: 8,
        symbols: vec![(0, 128), (64, 64), (128, 32)],
    });

    // RansByte: multiple symbols, prepared mode
    cases.push(RawCase {
        seed: 3,
        variant: 1,
        mode: 1,
        scale_bits: 8,
        symbols: vec![(0, 128), (64, 64), (128, 32)],
    });

    // RansByte: freq=1 (special case)
    cases.push(RawCase {
        seed: 4,
        variant: 1,
        mode: 1,
        scale_bits: 8,
        symbols: vec![(0, 1)],
    });

    // Rans64: multiple symbols, raw mode
    cases.push(RawCase {
        seed: 5,
        variant: 0,
        mode: 0,
        scale_bits: 16,
        symbols: vec![(0, 32768), (16384, 16384)],
    });

    // Rans64: multiple symbols, prepared mode
    cases.push(RawCase {
        seed: 6,
        variant: 0,
        mode: 1,
        scale_bits: 16,
        symbols: vec![(0, 32768), (16384, 16384)],
    });

    // Rans64: freq=1 (special case)
    cases.push(RawCase {
        seed: 7,
        variant: 0,
        mode: 1,
        scale_bits: 16,
        symbols: vec![(0, 1)],
    });

    // ---- seeded LCG sweep (adds ~104 cases) ----
    let mut rng = crate::corpus::Lcg::new(0xE7_1C0DE);
    let mut extra = 0usize;
    let byte_bits = [8u32, 10, 12, 16, 20, 22, 23];
    let s64_bits = [8u32, 12, 16, 20, 24, 28, 31];
    let mut seq = 0u64;

    for &variant in &[1u32, 0u32] {
        for &mode in &[0u32, 1u32] {
            let bits = if variant == 1 { byte_bits } else { s64_bits };
            for &sb in &bits {
                let scale = 1u32 << sb;
                for run in 0..3 {
                    let count = 1 + (rng.below(24) as usize);
                    let symbols = crate::corpus::gen_symbols(&mut rng, scale, count);
                    // Boundary-flavored sequences: freq=1 / near-full / half
                    let symbols = if run == 0 {
                        vec![
                            (0, 1),
                            (0, scale - 1),
                            (scale - 1, 1),
                            (scale / 2, scale / 2),
                        ]
                    } else if run == 1 {
                        let mut s = vec![(0, scale.max(2) - 1), (scale / 3, scale / 3)];
                        s.extend(symbols.iter().take(4).copied());
                        s
                    } else {
                        symbols
                    };
                    cases.push(RawCase {
                        seed: 1000 + seq,
                        variant,
                        mode,
                        scale_bits: sb,
                        symbols,
                    });
                    extra += 1;
                    seq += 1;
                }
            }
        }
    }

    // Boundary-focused Rans64 cases at the operational top (scale_bits=31)
    for &mode in &[0u32, 1u32] {
        for i in 0..4 {
            let scale = 1u32 << 31;
            let symbols = match i {
                0 => vec![(0, 1), (scale - 1, 1), (scale / 2, scale / 2)],
                1 => vec![(0, scale - 1), (1, scale - 2), (2, scale - 3)],
                2 => vec![(scale - 2, 2), (scale / 3, scale / 3)],
                _ => crate::corpus::gen_symbols(&mut rng, scale, 8),
            };
            cases.push(RawCase {
                seed: 2000 + seq,
                variant: 0,
                mode,
                scale_bits: 31,
                symbols,
            });
            extra += 1;
            seq += 1;
        }
    }

    let _ = extra;
    cases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_case_serialization() {
        let c = RawCase {
            seed: 0,
            variant: 1,
            mode: 0,
            scale_bits: 8,
            symbols: vec![(0, 128), (64, 64)],
        };
        let bin = c.to_binary();
        // variant(4) + mode(4) + scale_bits(4) + count(4) + 2*(start(4)+freq(4))
        assert_eq!(bin.len(), 4 + 4 + 4 + 4 + 2 * 8);
        assert_eq!(&bin[..4], &[1u8, 0, 0, 0]); // RansByte
        assert_eq!(&bin[4..8], &[0u8, 0, 0, 0]); // mode 0
        assert_eq!(&bin[8..12], &[8u8, 0, 0, 0]); // scale_bits 8
        assert_eq!(&bin[12..16], &[2u8, 0, 0, 0]); // 2 symbols
    }

    #[test]
    fn test_raw_rust_encodes_byte_single() {
        let c = RawCase {
            seed: 0,
            variant: 1,
            mode: 0,
            scale_bits: 8,
            symbols: vec![(0, 128)],
        };
        let out = c.rust_encode().expect("encode");
        assert!(!out.is_empty());
    }

    #[test]
    fn test_raw_rust_prepared_matches_raw_byte() {
        let c_raw = RawCase {
            seed: 0,
            variant: 1,
            mode: 0,
            scale_bits: 8,
            symbols: vec![(0, 128), (64, 64)],
        };
        let c_prep = RawCase {
            seed: 0,
            variant: 1,
            mode: 1,
            scale_bits: 8,
            symbols: vec![(0, 128), (64, 64)],
        };
        let out_raw = c_raw.rust_encode().expect("raw");
        let out_prep = c_prep.rust_encode().expect("prepared");
        assert_eq!(out_raw, out_prep, "prepared must match raw path");
    }

    #[test]
    fn test_raw_rust_prepared_matches_raw_64() {
        let c_raw = RawCase {
            seed: 0,
            variant: 0,
            mode: 0,
            scale_bits: 16,
            symbols: vec![(0, 32768), (16384, 16384)],
        };
        let c_prep = RawCase {
            seed: 0,
            variant: 0,
            mode: 1,
            scale_bits: 16,
            symbols: vec![(0, 32768), (16384, 16384)],
        };
        let out_raw = c_raw.rust_encode().expect("raw");
        let out_prep = c_prep.rust_encode().expect("prepared");
        assert_eq!(out_raw, out_prep, "prepared must match raw path (Rans64)");
    }

    #[test]
    fn test_raw_court_generates_cases() {
        let court = RawEncoderDifferentialCourt;
        let result = court.run();
        assert!(result.case_count > 0, "court must generate cases");
        // This test passes even if Docker is unavailable (cases are still generated)
    }

    #[test]
    #[ignore = "requires Docker oracle image: msrtc-rans-rs-oracle:debian12"]
    fn test_raw_court_full_differential() {
        let court = RawEncoderDifferentialCourt;
        let result = court.run();
        assert_eq!(
            result.status,
            CourtStatus::Passed,
            "raw encoder differential court must pass: {} passed, {} residuals",
            result.pass_count,
            result.residual_count
        );
        assert!(result.is_sealable(), "passing court must be sealable");
    }
}
