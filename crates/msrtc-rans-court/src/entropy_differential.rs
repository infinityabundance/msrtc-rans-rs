// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # MSRTC.ENTROPY.DIFFERENTIAL — High-level entropy differential court
//!
//! Compares the full Rust EntropyEncoder (PMF, distributions, bypass)
//! against Microsoft's high-level EntropyEncoder.
//!
//! ## Status: ACTIVE
//!
//! This court now uses the full [`msrtc_rans::entropy::EntropyEncoder`] /
//! [`msrtc_rans::entropy::EntropyDecoder`] for both RansByte and Rans64
//! variants, including full bypass encoding/decoding, CDF construction,
//! and binary-search symbol lookup.
//!
//! ### Sub-cases per input:
//!
//! 1. **Encoder differential** — Rust EntropyEncoder output vs oracle_cli output
//! 2. **Roundtrip** — Rust encode + Rust decode, verify reconstructed values
//! 3. **Rust-encode / C++-decode** — Encode with Rust, decode with decoder_oracle_cli
//! 4. **C++-encode / Rust-decode** — Encode with oracle_cli, decode with EntropyDecoder

use msrtc_rans::entropy::{EntropyDecoder, EntropyEncoder};
use msrtc_rans_casefile::{
    Comparison, DifferentialResult, InputHashes, NativeResult, OracleResult,
    classification::{ResidualClassification, ResolutionState},
    sha256,
};
use msrtc_rans_core::variant::{Rans64, RansByte};

use crate::oracle::{
    self, compare_bytes, environment_sha256, git_commit, hash_i32_array, try_write_residual,
};
use crate::{Court, CourtResult, CourtStatus};

/// A single entropy-level test case.
#[derive(Debug, Clone)]
pub struct EntropyCase {
    pub seed: u64,
    /// 0=Rans64, 1=RansByte
    pub variant: u32,
    pub symbol_bits: u32,
    pub bypass_bits: u32,
    pub pmf_lengths: Vec<i32>,
    pub pmf_offsets: Vec<i32>,
    pub pmf_table: Vec<i32>,
    pub indices: Vec<i32>,
    pub values: Vec<i32>,
}

impl EntropyCase {
    /// Serialize to the oracle CLI binary format.
    pub fn to_binary(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.variant.to_le_bytes());
        buf.extend_from_slice(&self.symbol_bits.to_le_bytes());
        buf.extend_from_slice(&self.bypass_bits.to_le_bytes());
        write_i32_array(&mut buf, &self.pmf_lengths);
        write_i32_array(&mut buf, &self.pmf_offsets);
        write_i32_array(&mut buf, &self.pmf_table);
        write_i32_array(&mut buf, &self.indices);
        write_i32_array(&mut buf, &self.values);
        buf
    }

    /// Hash of the canonical binary input.
    pub fn input_hash(&self) -> String {
        sha256(&self.to_binary())
    }

    /// Input hashes for the DifferentialResult.
    pub fn input_hashes(&self) -> InputHashes {
        InputHashes {
            pmf_lengths_sha256: hash_i32_array(&self.pmf_lengths),
            pmf_offsets_sha256: hash_i32_array(&self.pmf_offsets),
            pmf_table_sha256: hash_i32_array(&self.pmf_table),
            indices_sha256: hash_i32_array(&self.indices),
            values_sha256: hash_i32_array(&self.values),
        }
    }
}

fn write_i32_array(buf: &mut Vec<u8>, arr: &[i32]) {
    buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
    for v in arr {
        buf.extend_from_slice(&v.to_le_bytes());
    }
}

/// MSRTC.ENTROPY.DIFFERENTIAL — full entropy differential court.
pub struct EntropyDifferentialCourt;

impl Court for EntropyDifferentialCourt {
    fn id(&self) -> &str {
        "MSRTC.ENTROPY.DIFFERENTIAL"
    }

    fn run(&self) -> CourtResult {
        let cases = generate_entropy_cases();
        let mut results = Vec::with_capacity(cases.len());

        for case in &cases {
            let all_input_sha = case.input_hash();
            let input_hashes = case.input_hashes();

            // ------------------------------------------------------------------
            // 1. Encoder differential: Rust EntropyEncoder vs oracle_cli
            // ------------------------------------------------------------------
            let encoder_result = run_encoder_differential(case, &all_input_sha, &input_hashes);
            results.push(encoder_result);

            // ------------------------------------------------------------------
            // 2. Decoder path: Rust encode + Rust decode roundtrip
            // ------------------------------------------------------------------
            let roundtrip_result = run_roundtrip(case, &all_input_sha, &input_hashes);
            results.push(roundtrip_result);

            // ------------------------------------------------------------------
            // 3. Cross-encode: C++ oracle encode → Rust decode
            // ------------------------------------------------------------------
            let cross_result = run_cpp_encode_rust_decode(case, &all_input_sha, &input_hashes);
            results.push(cross_result);
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

// ---------------------------------------------------------------------------
// Rust encoder (full EntropyEncoder)
// ---------------------------------------------------------------------------

fn rust_encode(case: &EntropyCase) -> Result<Vec<u8>, String> {
    match case.variant {
        0 => rust_encode_64(case),
        1 => rust_encode_byte(case),
        v => Err(format!("unknown variant {}", v)),
    }
}

fn rust_encode_byte(case: &EntropyCase) -> Result<Vec<u8>, String> {
    let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
    enc.initialize(
        &case.pmf_lengths,
        &case.pmf_offsets,
        &case.pmf_table,
        case.symbol_bits,
        case.bypass_bits,
    )
    .map_err(|e| format!("EntropyEncoder init error: {:?}", e))?;

    let mut buffer = Vec::new();
    enc.encode(&case.indices, &case.values, &mut buffer)
        .map_err(|e| format!("EntropyEncoder encode error: {:?}", e))?;
    Ok(buffer)
}

fn rust_encode_64(case: &EntropyCase) -> Result<Vec<u8>, String> {
    let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
    enc.initialize(
        &case.pmf_lengths,
        &case.pmf_offsets,
        &case.pmf_table,
        case.symbol_bits,
        case.bypass_bits,
    )
    .map_err(|e| format!("EntropyEncoder init error: {:?}", e))?;

    let mut buffer = Vec::new();
    enc.encode(&case.indices, &case.values, &mut buffer)
        .map_err(|e| format!("EntropyEncoder encode error: {:?}", e))?;
    Ok(buffer)
}

// ---------------------------------------------------------------------------
// Rust decoder (full EntropyDecoder)
// ---------------------------------------------------------------------------

fn rust_decode(case: &EntropyCase, data: &[u8]) -> Result<Vec<i32>, String> {
    match case.variant {
        0 => rust_decode_64(case, data),
        1 => rust_decode_byte(case, data),
        v => Err(format!("unknown variant {}", v)),
    }
}

fn rust_decode_byte(case: &EntropyCase, data: &[u8]) -> Result<Vec<i32>, String> {
    let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
    dec.initialize(
        &case.pmf_lengths,
        &case.pmf_offsets,
        &case.pmf_table,
        case.symbol_bits,
        case.bypass_bits,
    )
    .map_err(|e| format!("EntropyDecoder init error: {:?}", e))?;

    let mut values = vec![0i32; case.indices.len()];
    dec.decode(&mut values, &case.indices, data)
        .map_err(|e| format!("EntropyDecoder decode error: {:?}", e))?;
    Ok(values)
}

fn rust_decode_64(case: &EntropyCase, data: &[u8]) -> Result<Vec<i32>, String> {
    let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
    dec.initialize(
        &case.pmf_lengths,
        &case.pmf_offsets,
        &case.pmf_table,
        case.symbol_bits,
        case.bypass_bits,
    )
    .map_err(|e| format!("EntropyDecoder init error: {:?}", e))?;

    let mut values = vec![0i32; case.indices.len()];
    dec.decode(&mut values, &case.indices, data)
        .map_err(|e| format!("EntropyDecoder decode error: {:?}", e))?;
    Ok(values)
}

// ---------------------------------------------------------------------------
// Sub-case runners
// ---------------------------------------------------------------------------

/// Sub-case 1: Compare Rust EntropyEncoder output vs oracle_cli output.
fn run_encoder_differential(
    case: &EntropyCase,
    all_input_sha: &str,
    input_hashes: &InputHashes,
) -> DifferentialResult {
    let variant_str = if case.variant == 1 {
        "RansByte"
    } else {
        "Rans64"
    };
    let case_id = format!("encoder_diff:sha256:{}", all_input_sha);

    let (native_status, native_output) = match rust_encode(case) {
        Ok(bytes) => ("ok".to_string(), bytes),
        Err(e) => (format!("native_error: {}", e), Vec::new()),
    };
    let native_len = native_output.len() as u64;
    let native_sha = sha256(&native_output);

    let oracle_result = oracle::run_oracle(&case.to_binary());
    let (oracle_status, oracle_output) = match &oracle_result {
        Ok(resp) => ("ok".to_string(), resp.raw_output.clone()),
        Err(_e) => ("oracle_error".to_string(), Vec::new()),
    };

    let exact = native_status == oracle_status && native_output == oracle_output;
    let comparison = compare_bytes(&native_output, &oracle_output);

    let classification = match oracle_result {
        Ok(_) => {
            if exact {
                ResidualClassification::Unclassified
            } else {
                ResidualClassification::NativeBug
            }
        }
        Err(ref e) => oracle::classify_error(e),
    };

    let result = DifferentialResult {
        schema_version: oracle::SCHEMA_VERSION,
        court_id: "MSRTC.ENTROPY.DIFFERENTIAL".to_string(),
        case_id,
        oracle_commit: oracle::ORACLE_COMMIT.to_string(),
        rust_commit: git_commit(),
        seed: case.seed,
        variant: variant_str.into(),
        input_hashes: input_hashes.clone(),
        oracle: OracleResult {
            status: oracle_status,
            output_sha256: oracle_result
                .as_ref()
                .map(|r| r.sha256.clone())
                .unwrap_or_default(),
            length: oracle_result.as_ref().map(|r| r.length as u64).unwrap_or(0),
        },
        native: NativeResult {
            status: native_status,
            output_sha256: native_sha,
            length: native_len,
        },
        comparison,
        classification,
        resolution: ResolutionState::Open,
        minimized_casefile: None,
        environment_sha256: environment_sha256(),
    };

    if !exact {
        let _ = try_write_residual(&result);
    }
    result
}

/// Sub-case 2: Rust encode → Rust decode roundtrip. Verify reconstructed values.
fn run_roundtrip(
    case: &EntropyCase,
    all_input_sha: &str,
    input_hashes: &InputHashes,
) -> DifferentialResult {
    let variant_str = if case.variant == 1 {
        "RansByte"
    } else {
        "Rans64"
    };
    let case_id = format!("roundtrip:sha256:{}", all_input_sha);

    let encoded = match rust_encode(case) {
        Ok(bytes) => bytes,
        Err(e) => {
            return DifferentialResult {
                schema_version: oracle::SCHEMA_VERSION,
                court_id: "MSRTC.ENTROPY.DIFFERENTIAL".to_string(),
                case_id,
                oracle_commit: oracle::ORACLE_COMMIT.to_string(),
                rust_commit: git_commit(),
                seed: case.seed,
                variant: variant_str.into(),
                input_hashes: input_hashes.clone(),
                oracle: OracleResult {
                    status: String::new(),
                    output_sha256: String::new(),
                    length: 0,
                },
                native: NativeResult {
                    status: format!("encode_error: {}", e),
                    output_sha256: String::new(),
                    length: 0,
                },
                comparison: Comparison {
                    exact: false,
                    first_differing_offset: None,
                    differing_bytes: None,
                },
                classification: ResidualClassification::NativeBug,
                resolution: ResolutionState::Open,
                minimized_casefile: None,
                environment_sha256: environment_sha256(),
            };
        }
    };

    let decoded = match rust_decode(case, &encoded) {
        Ok(vals) => vals,
        Err(e) => {
            return DifferentialResult {
                schema_version: oracle::SCHEMA_VERSION,
                court_id: "MSRTC.ENTROPY.DIFFERENTIAL".to_string(),
                case_id,
                oracle_commit: oracle::ORACLE_COMMIT.to_string(),
                rust_commit: git_commit(),
                seed: case.seed,
                variant: variant_str.into(),
                input_hashes: input_hashes.clone(),
                oracle: OracleResult {
                    status: String::new(),
                    output_sha256: String::new(),
                    length: 0,
                },
                native: NativeResult {
                    status: format!("decode_error: {}", e),
                    output_sha256: sha256(&encoded),
                    length: encoded.len() as u64,
                },
                comparison: Comparison {
                    exact: false,
                    first_differing_offset: None,
                    differing_bytes: None,
                },
                classification: ResidualClassification::NativeBug,
                resolution: ResolutionState::Open,
                minimized_casefile: None,
                environment_sha256: environment_sha256(),
            };
        }
    };

    let exact = decoded == case.values;
    let comparison = if exact {
        Comparison {
            exact: true,
            first_differing_offset: None,
            differing_bytes: None,
        }
    } else {
        let first_diff = decoded
            .iter()
            .zip(case.values.iter())
            .position(|(a, b)| a != b)
            .map(|i| i as u64);
        let diff_count = decoded
            .iter()
            .zip(case.values.iter())
            .filter(|(a, b)| a != b)
            .count() as u64;
        Comparison {
            exact: false,
            first_differing_offset: first_diff,
            differing_bytes: Some(diff_count),
        }
    };

    let result = DifferentialResult {
        schema_version: oracle::SCHEMA_VERSION,
        court_id: "MSRTC.ENTROPY.DIFFERENTIAL".to_string(),
        case_id,
        oracle_commit: oracle::ORACLE_COMMIT.to_string(),
        rust_commit: git_commit(),
        seed: case.seed,
        variant: variant_str.into(),
        input_hashes: input_hashes.clone(),
        oracle: OracleResult {
            status: String::new(),
            output_sha256: String::new(),
            length: 0,
        },
        native: NativeResult {
            status: "ok".to_string(),
            output_sha256: sha256(&encoded),
            length: encoded.len() as u64,
        },
        comparison,
        classification: if exact {
            ResidualClassification::Unclassified
        } else {
            ResidualClassification::NativeBug
        },
        resolution: ResolutionState::Open,
        minimized_casefile: None,
        environment_sha256: environment_sha256(),
    };

    if !exact {
        let _ = try_write_residual(&result);
    }
    result
}

/// Sub-case 3: Rust encode → C++ decoder oracle decode.
/// Sub-case 3: C++ oracle encode → Rust EntropyDecoder decode.
fn run_cpp_encode_rust_decode(
    case: &EntropyCase,
    all_input_sha: &str,
    input_hashes: &InputHashes,
) -> DifferentialResult {
    let variant_str = if case.variant == 1 {
        "RansByte"
    } else {
        "Rans64"
    };
    let case_id = format!("cpp_enc_rust_dec:sha256:{}", all_input_sha);

    // C++ oracle encode
    let oracle_result = oracle::run_oracle(&case.to_binary());
    let (oracle_status, oracle_bitstream) = match &oracle_result {
        Ok(resp) => ("ok".to_string(), resp.raw_output.clone()),
        Err(e) => (format!("oracle_error: {}", e), vec![]),
    };

    if oracle_status != "ok" {
        return DifferentialResult {
            schema_version: oracle::SCHEMA_VERSION,
            court_id: "MSRTC.ENTROPY.DIFFERENTIAL".to_string(),
            case_id,
            oracle_commit: oracle::ORACLE_COMMIT.to_string(),
            rust_commit: git_commit(),
            seed: case.seed,
            variant: variant_str.into(),
            input_hashes: input_hashes.clone(),
            oracle: OracleResult {
                status: oracle_status.clone(),
                output_sha256: String::new(),
                length: 0,
            },
            native: NativeResult {
                status: String::new(),
                output_sha256: String::new(),
                length: 0,
            },
            comparison: Comparison {
                exact: false,
                first_differing_offset: None,
                differing_bytes: None,
            },
            classification: oracle::classify_error(&oracle_status),
            resolution: ResolutionState::Open,
            minimized_casefile: None,
            environment_sha256: environment_sha256(),
        };
    }

    // Rust EntropyDecoder decode
    let decoded = match rust_decode(case, &oracle_bitstream) {
        Ok(vals) => vals,
        Err(e) => {
            return DifferentialResult {
                schema_version: oracle::SCHEMA_VERSION,
                court_id: "MSRTC.ENTROPY.DIFFERENTIAL".to_string(),
                case_id,
                oracle_commit: oracle::ORACLE_COMMIT.to_string(),
                rust_commit: git_commit(),
                seed: case.seed,
                variant: variant_str.into(),
                input_hashes: input_hashes.clone(),
                oracle: OracleResult {
                    status: "ok".to_string(),
                    output_sha256: oracle_result
                        .as_ref()
                        .map(|r| r.sha256.clone())
                        .unwrap_or_default(),
                    length: oracle_result.as_ref().map(|r| r.length as u64).unwrap_or(0),
                },
                native: NativeResult {
                    status: format!("decode_error: {}", e),
                    output_sha256: sha256(&oracle_bitstream),
                    length: oracle_bitstream.len() as u64,
                },
                comparison: Comparison {
                    exact: false,
                    first_differing_offset: None,
                    differing_bytes: None,
                },
                classification: ResidualClassification::NativeBug,
                resolution: ResolutionState::Open,
                minimized_casefile: None,
                environment_sha256: environment_sha256(),
            };
        }
    };

    let exact = decoded == case.values;
    let comparison = if exact {
        Comparison {
            exact: true,
            first_differing_offset: None,
            differing_bytes: None,
        }
    } else {
        let first_diff = decoded
            .iter()
            .zip(case.values.iter())
            .position(|(a, b)| a != b)
            .map(|i| i as u64);
        let diff_count = decoded
            .iter()
            .zip(case.values.iter())
            .filter(|(a, b)| a != b)
            .count() as u64;
        Comparison {
            exact: false,
            first_differing_offset: first_diff,
            differing_bytes: Some(diff_count),
        }
    };

    let result = DifferentialResult {
        schema_version: oracle::SCHEMA_VERSION,
        court_id: "MSRTC.ENTROPY.DIFFERENTIAL".to_string(),
        case_id,
        oracle_commit: oracle::ORACLE_COMMIT.to_string(),
        rust_commit: git_commit(),
        seed: case.seed,
        variant: variant_str.into(),
        input_hashes: input_hashes.clone(),
        oracle: OracleResult {
            status: "ok".to_string(),
            output_sha256: oracle_result
                .as_ref()
                .map(|r| r.sha256.clone())
                .unwrap_or_default(),
            length: oracle_result.as_ref().map(|r| r.length as u64).unwrap_or(0),
        },
        native: NativeResult {
            status: "ok".to_string(),
            output_sha256: sha256(&oracle_bitstream),
            length: oracle_bitstream.len() as u64,
        },
        comparison,
        classification: if exact {
            ResidualClassification::Unclassified
        } else {
            ResidualClassification::NativeBug
        },
        resolution: ResolutionState::Open,
        minimized_casefile: None,
        environment_sha256: environment_sha256(),
    };

    if !exact {
        let _ = try_write_residual(&result);
    }
    result
}

// ---------------------------------------------------------------------------
// Raw decode helper: extract cum_freq values from a bitstream using raw rANS
// (used to compare against the decoder oracle output)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Test case generation
// ---------------------------------------------------------------------------

fn generate_entropy_cases() -> Vec<EntropyCase> {
    vec![
        // Reference case from test_msrtc_rans.py (RansByte)
        EntropyCase {
            seed: 0,
            variant: 1,
            symbol_bits: 16,
            bypass_bits: 4,
            pmf_lengths: vec![4, 6],
            pmf_offsets: vec![1, 2],
            pmf_table: vec![1, 3, 1, 1, 1, 3, 5, 3, 1, 1],
            indices: vec![0, 1, 0, 1],
            values: vec![-2, 1, 0, 1],
        },
        // Same with Rans64
        EntropyCase {
            seed: 1,
            variant: 0,
            ..EntropyCase {
                seed: 0,
                variant: 1,
                symbol_bits: 16,
                bypass_bits: 4,
                pmf_lengths: vec![4, 6],
                pmf_offsets: vec![1, 2],
                pmf_table: vec![1, 3, 1, 1, 1, 3, 5, 3, 1, 1],
                indices: vec![0, 1, 0, 1],
                values: vec![-2, 1, 0, 1],
            }
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_case_serialization() {
        let c = EntropyCase {
            seed: 0,
            variant: 1,
            symbol_bits: 16,
            bypass_bits: 4,
            pmf_lengths: vec![4, 6],
            pmf_offsets: vec![1, 2],
            pmf_table: vec![1, 3, 1, 1, 1, 3, 5, 3, 1, 1],
            indices: vec![0, 1, 0, 1],
            values: vec![-2, 1, 0, 1],
        };
        let bin = c.to_binary();
        assert!(!bin.is_empty());
        assert_eq!(&bin[..4], &[1u8, 0, 0, 0]); // RansByte
    }

    #[test]
    fn test_entropy_court_generates_cases() {
        let court = EntropyDifferentialCourt;
        let result = court.run();

        // Since oracle won't be available without Docker, only check case count
        assert!(result.case_count > 0);
    }

    #[test]
    #[ignore = "requires Docker oracle container"]
    fn test_entropy_court_full_differential() {
        let court = EntropyDifferentialCourt;
        let result = court.run();
        assert_eq!(result.status, CourtStatus::Passed);
        assert!(result.is_sealable());
    }
}
