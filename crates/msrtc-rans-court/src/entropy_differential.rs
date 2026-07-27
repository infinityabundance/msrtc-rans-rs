// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # MSRTC.ENTROPY.DIFFERENTIAL — High-level entropy differential court
//!
//! Compares the full Rust EntropyEncoder (PMF, distributions, bypass)
//! against Microsoft's high-level EntropyEncoder.
//!
//! ## Status: PARTIAL
//!
//! This court is `partial` because Rust does not yet implement:
//! - bypass value encoding/decoding
//! - CDF table construction and binary-search lookup
//! - value reconstruction from decoded symbols
//!
//! Until those are implemented, any case with out-of-range values will
//! produce a mismatch (Rust encodes the sentinel without the bypass payload).
//!
//! The court is retained as the integration target for Phase 3.

use msrtc_rans_casefile::{
    Comparison, DifferentialResult, InputHashes, NativeResult, OracleResult,
    classification::{ResidualClassification, ResolutionState},
    sha256,
};

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

/// MSRTC.ENTROPY.DIFFERENTIAL — partial until Phase 3 bypass/CDF implementation.
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

            // ---- Rust encode (partial — no bypass) ----
            let (native_status, native_output) = rust_encode_partial(case);
            let native_len = native_output.len() as u64;
            let native_sha = sha256(&native_output);

            // ---- Oracle encode ----
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
                        // Known gap: Rust doesn't implement bypass yet
                        ResidualClassification::NativeBug
                    }
                }
                Err(ref e) => oracle::classify_error(e),
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
                input_hashes,
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

/// Partial Rust entropy encode — only handles in-range symbols.
/// Out-of-range values produce the sentinel symbol but no bypass payload.
/// This is intentionally incomplete (bypass not yet implemented).
fn rust_encode_partial(case: &EntropyCase) -> (String, Vec<u8>) {
    match case.variant {
        0 => rust_encode_partial_64(case),
        1 => rust_encode_partial_byte(case),
        _ => (
            format!("native_error: unknown variant {}", case.variant),
            Vec::new(),
        ),
    }
}

/// RansByte branch of `rust_encode_partial`.
fn rust_encode_partial_byte(case: &EntropyCase) -> (String, Vec<u8>) {
    use msrtc_rans_core::RansByteEncSymbol;
    use msrtc_rans_core::RansByteEncoder;
    use msrtc_rans_core::sink::VecSink;

    let sink = VecSink::<u8>::new(4096);
    let mut encoder = RansByteEncoder::new(sink);

    for i in (0..case.indices.len()).rev() {
        let mut idx = case.indices[i];
        if idx < 0 {
            continue;
        }
        let num_dist = case.pmf_lengths.len();
        idx = idx.min(num_dist as i32 - 1);
        let dist_idx = idx as usize;
        let length = case.pmf_lengths[dist_idx] as usize;
        let offset = case.pmf_offsets[dist_idx];

        let value = case.values[i] + offset;
        let sym_idx = if value < 0 || value as usize >= length - 1 {
            // Bypass sentinel — no bypass payload (known gap)
            length - 1
        } else {
            value as usize
        };

        let sym_start: usize = case.pmf_lengths[..dist_idx]
            .iter()
            .map(|&l| l as usize)
            .sum();
        let mut start: u32 = 0;
        for j in sym_start..(sym_start + sym_idx) {
            start += case.pmf_table[j] as u32;
        }
        let freq = case.pmf_table[sym_start + sym_idx] as u32;

        let sym = match RansByteEncSymbol::try_new(start, freq, case.symbol_bits as u32) {
            Ok(s) => s,
            Err(e) => return (format!("native_error: {:?}", e), Vec::new()),
        };
        encoder.put(&sym);
    }

    encoder.flush();
    ("ok".to_string(), encoder.into_sink().encoded().to_vec())
}

/// Rans64 branch of `rust_encode_partial`.
/// Uses Rans64Encoder with VecSink<u32>, then converts u32 units to little-endian bytes.
fn rust_encode_partial_64(case: &EntropyCase) -> (String, Vec<u8>) {
    use msrtc_rans_core::Rans64EncSymbol;
    use msrtc_rans_core::Rans64Encoder;
    use msrtc_rans_core::sink::VecSink;

    let sink = VecSink::<u32>::new(4096);
    let mut encoder = Rans64Encoder::new(sink);

    for i in (0..case.indices.len()).rev() {
        let mut idx = case.indices[i];
        if idx < 0 {
            continue;
        }
        let num_dist = case.pmf_lengths.len();
        idx = idx.min(num_dist as i32 - 1);
        let dist_idx = idx as usize;
        let length = case.pmf_lengths[dist_idx] as usize;
        let offset = case.pmf_offsets[dist_idx];

        let value = case.values[i] + offset;
        let sym_idx = if value < 0 || value as usize >= length - 1 {
            // Bypass sentinel — no bypass payload (known gap)
            length - 1
        } else {
            value as usize
        };

        let sym_start: usize = case.pmf_lengths[..dist_idx]
            .iter()
            .map(|&l| l as usize)
            .sum();
        let mut start: u32 = 0;
        for j in sym_start..(sym_start + sym_idx) {
            start += case.pmf_table[j] as u32;
        }
        let freq = case.pmf_table[sym_start + sym_idx] as u32;

        let sym = match Rans64EncSymbol::try_new(start, freq, case.symbol_bits as u32) {
            Ok(s) => s,
            Err(e) => return (format!("native_error: {:?}", e), Vec::new()),
        };
        encoder.put(&sym);
    }

    encoder.flush();
    let units = encoder.into_sink().encoded().to_vec();
    let mut bytes = Vec::with_capacity(units.len() * 4);
    for &u in &units {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    ("ok".to_string(), bytes)
}

fn generate_entropy_cases() -> Vec<EntropyCase> {
    vec![
        // Reference case from test_msrtc_rans.py (RansByte)
        // NOTE: values=[-2, 1, 0, 1] contains -2 which is OUT OF RANGE
        // after applying offset 1. This will diverge until bypass is implemented.
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
        assert!(result.case_count > 0);
    }

    #[test]
    #[ignore = "requires full entropy implementation (bypass, CDF)"]
    fn test_entropy_full_differential() {
        let court = EntropyDifferentialCourt;
        let result = court.run();
        assert_eq!(result.status, CourtStatus::Passed);
        assert!(result.is_sealable());
    }
}
