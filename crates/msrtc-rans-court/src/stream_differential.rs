// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # MSRTC.STREAM.DIFFERENTIAL — Multipart persistent stream differential court
//!
//! Compares the Rust `RansEncoderStream` / `RansDecoderStream` (persistent
//! raw rANS state across `push()` calls) against Microsoft's
//! `RansEncoderStream` / `RansDecoderStream`.
//!
//! ## Status: ACTIVE
//!
//! Each case pushes multiple batches into a single persistent encoder
//! stream and flushes once. Sub-cases per input:
//!
//! 1. **Wire parity** — Rust flush bytes vs Microsoft stream bytes
//!    (`stream_oracle_cli encode`). This proves the multipart layout is
//!    byte-identical, not merely self-consistent.
//! 2. **Microsoft stream → Rust decode** — Microsoft's stream is decoded
//!    by the Rust `RansDecoderStream` (last-pushed batch first).
//! 3. **Rust stream → Microsoft decode** — Microsoft's `RansDecoderStream`
//!    decodes the Rust stream (`stream_oracle_cli decode`) and reports
//!    the reconstructed values.
//!
//! The decode order is the reverse of the push order (LIFO), matching the
//! backward-writing rANS stream layout.

use msrtc_rans::entropy::{EntropyDecoder, EntropyEncoder};
use msrtc_rans::stream::{RansDecoderStream, RansEncoderStream};
use msrtc_rans_casefile::{
    Comparison, DifferentialResult, InputHashes, NativeResult, OracleResult,
    classification::{ResidualClassification, ResolutionState},
    sha256,
};
use msrtc_rans_core::variant::{Rans64, RansByte};

use crate::oracle::{self, compare_bytes, environment_sha256, git_commit, try_write_residual};
use crate::{Court, CourtResult, CourtStatus};

/// A single batch (one PMF + one message) in a multipart stream.
#[derive(Debug, Clone)]
pub struct StreamBatch {
    pub pmf_lengths: Vec<i32>,
    pub pmf_offsets: Vec<i32>,
    pub pmf_table: Vec<i32>,
    pub indices: Vec<i32>,
    pub values: Vec<i32>,
}

/// A multipart stream test case.
#[derive(Debug, Clone)]
pub struct StreamCase {
    pub seed: u64,
    /// 0=Rans64, 1=RansByte
    pub variant: u32,
    pub symbol_bits: u32,
    pub bypass_bits: u32,
    /// Batches in PUSH order.
    pub batches: Vec<StreamBatch>,
}

fn write_i32_array(buf: &mut Vec<u8>, arr: &[i32]) {
    buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
    for v in arr {
        buf.extend_from_slice(&v.to_le_bytes());
    }
}

impl StreamCase {
    /// Serialize to the stream oracle CLI binary format.
    ///
    /// Layout: variant, symbol_bits, bypass_bits, batch_count, then per
    /// batch (push order) the five length-prefixed i32 arrays.
    pub fn to_binary(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.variant.to_le_bytes());
        buf.extend_from_slice(&self.symbol_bits.to_le_bytes());
        buf.extend_from_slice(&self.bypass_bits.to_le_bytes());
        buf.extend_from_slice(&(self.batches.len() as u32).to_le_bytes());
        for batch in &self.batches {
            write_i32_array(&mut buf, &batch.pmf_lengths);
            write_i32_array(&mut buf, &batch.pmf_offsets);
            write_i32_array(&mut buf, &batch.pmf_table);
            write_i32_array(&mut buf, &batch.indices);
            write_i32_array(&mut buf, &batch.values);
        }
        buf
    }

    /// Hash of the canonical binary input.
    pub fn input_hash(&self) -> String {
        sha256(&self.to_binary())
    }

    /// Input hashes for the DifferentialResult.
    ///
    /// For multipart cases each field is the hash of the concatenated
    /// length-prefixed arrays across all batches (canonical, order-aware).
    pub fn input_hashes(&self) -> InputHashes {
        let mut lengths = Vec::new();
        let mut offsets = Vec::new();
        let mut table = Vec::new();
        let mut indices = Vec::new();
        let mut values = Vec::new();
        for batch in &self.batches {
            append_i32(&mut lengths, &batch.pmf_lengths);
            append_i32(&mut offsets, &batch.pmf_offsets);
            append_i32(&mut table, &batch.pmf_table);
            append_i32(&mut indices, &batch.indices);
            append_i32(&mut values, &batch.values);
        }
        InputHashes {
            pmf_lengths_sha256: sha256(&lengths),
            pmf_offsets_sha256: sha256(&offsets),
            pmf_table_sha256: sha256(&table),
            indices_sha256: sha256(&indices),
            values_sha256: sha256(&values),
        }
    }
}

fn append_i32(buf: &mut Vec<u8>, arr: &[i32]) {
    buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
    for v in arr {
        buf.extend_from_slice(&v.to_le_bytes());
    }
}

/// MSRTC.STREAM.DIFFERENTIAL — multipart stream differential court.
pub struct StreamDifferentialCourt;

impl Court for StreamDifferentialCourt {
    fn id(&self) -> &str {
        "MSRTC.STREAM.DIFFERENTIAL"
    }

    fn run(&self) -> CourtResult {
        let cases = generate_stream_cases();
        let mut results = Vec::with_capacity(cases.len() * 3);

        for case in &cases {
            let all_input_sha = case.input_hash();
            let input_hashes = case.input_hashes();

            // 1. Wire parity: Rust stream bytes vs Microsoft stream bytes
            let wire = run_wire_parity(case, &all_input_sha, &input_hashes);
            results.push(wire);

            // 2. Microsoft stream → Rust decoder
            let cpp_to_rust = run_cpp_stream_rust_decode(case, &all_input_sha, &input_hashes);
            results.push(cpp_to_rust);

            // 3. Rust stream → Microsoft decoder
            let rust_to_cpp = run_rust_stream_cpp_decode(case, &all_input_sha, &input_hashes);
            results.push(rust_to_cpp);
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
// Rust stream encode / decode
// ---------------------------------------------------------------------------

/// Encode all batches into a single persistent stream and flush once.
fn rust_stream_encode(case: &StreamCase) -> Result<Vec<u8>, String> {
    match case.variant {
        1 => {
            let mut stream = RansEncoderStream::<RansByte>::new();
            for batch in &case.batches {
                let mut enc: EntropyEncoder<RansByte> = EntropyEncoder::new();
                enc.initialize(
                    &batch.pmf_lengths,
                    &batch.pmf_offsets,
                    &batch.pmf_table,
                    case.symbol_bits,
                    case.bypass_bits,
                )
                .map_err(|e| format!("EntropyEncoder init error: {:?}", e))?;
                stream
                    .push(&enc, &batch.indices, &batch.values)
                    .map_err(|e| format!("stream push error: {:?}", e))?;
            }
            stream
                .flush()
                .map_err(|e| format!("stream flush error: {:?}", e))
        }
        0 => {
            let mut stream = RansEncoderStream::<Rans64>::new();
            for batch in &case.batches {
                let mut enc: EntropyEncoder<Rans64> = EntropyEncoder::new();
                enc.initialize(
                    &batch.pmf_lengths,
                    &batch.pmf_offsets,
                    &batch.pmf_table,
                    case.symbol_bits,
                    case.bypass_bits,
                )
                .map_err(|e| format!("EntropyEncoder init error: {:?}", e))?;
                stream
                    .push(&enc, &batch.indices, &batch.values)
                    .map_err(|e| format!("stream push error: {:?}", e))?;
            }
            stream
                .flush()
                .map_err(|e| format!("stream flush error: {:?}", e))
        }
        v => Err(format!("unknown variant {}", v)),
    }
}

/// Decode a stream with the Rust persistent decoder.
///
/// Returns values per batch in DECODE order (reverse push order) — the
/// order Microsoft's decoder consumes them.
fn rust_stream_decode(case: &StreamCase, data: &[u8]) -> Result<Vec<Vec<i32>>, String> {
    match case.variant {
        1 => {
            let mut stream = RansDecoderStream::<RansByte>::open_on(data);
            let mut out = Vec::with_capacity(case.batches.len());
            for batch in case.batches.iter().rev() {
                let mut dec: EntropyDecoder<RansByte> = EntropyDecoder::new();
                dec.initialize(
                    &batch.pmf_lengths,
                    &batch.pmf_offsets,
                    &batch.pmf_table,
                    case.symbol_bits,
                    case.bypass_bits,
                )
                .map_err(|e| format!("EntropyDecoder init error: {:?}", e))?;
                let mut values = vec![0i32; batch.indices.len()];
                stream
                    .decode(&dec, &mut values, &batch.indices)
                    .map_err(|e| format!("stream decode error: {:?}", e))?;
                out.push(values);
            }
            stream
                .decode_eof()
                .map_err(|e| format!("stream decodeEOF error: {:?}", e))?;
            Ok(out)
        }
        0 => {
            if data.len() % 4 != 0 {
                return Err("Rans64 stream length must be a multiple of 4".into());
            }
            let mut stream = RansDecoderStream::<Rans64>::open_on(data);
            let mut out = Vec::with_capacity(case.batches.len());
            for batch in case.batches.iter().rev() {
                let mut dec: EntropyDecoder<Rans64> = EntropyDecoder::new();
                dec.initialize(
                    &batch.pmf_lengths,
                    &batch.pmf_offsets,
                    &batch.pmf_table,
                    case.symbol_bits,
                    case.bypass_bits,
                )
                .map_err(|e| format!("EntropyDecoder init error: {:?}", e))?;
                let mut values = vec![0i32; batch.indices.len()];
                stream
                    .decode(&dec, &mut values, &batch.indices)
                    .map_err(|e| format!("stream decode error: {:?}", e))?;
                out.push(values);
            }
            stream
                .decode_eof()
                .map_err(|e| format!("stream decodeEOF error: {:?}", e))?;
            Ok(out)
        }
        v => Err(format!("unknown variant {}", v)),
    }
}

/// Expected values in decode order (reverse push order).
fn expected_values(case: &StreamCase) -> Vec<Vec<i32>> {
    case.batches
        .iter()
        .rev()
        .map(|b| b.values.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Sub-case runners
// ---------------------------------------------------------------------------

/// Build a failure DifferentialResult (shared by all sub-case runners).
fn failure_result(
    court_id: &str,
    case_id: String,
    case: &StreamCase,
    input_hashes: &InputHashes,
    oracle_status: String,
    oracle_sha: String,
    oracle_len: u64,
    native_status: String,
    native_sha: String,
    native_len: u64,
    classification: ResidualClassification,
) -> DifferentialResult {
    DifferentialResult {
        schema_version: oracle::SCHEMA_VERSION,
        court_id: court_id.to_string(),
        case_id,
        oracle_commit: oracle::ORACLE_COMMIT.to_string(),
        rust_commit: git_commit(),
        seed: case.seed,
        variant: if case.variant == 1 {
            "RansByte".into()
        } else {
            "Rans64".into()
        },
        input_hashes: input_hashes.clone(),
        oracle: OracleResult {
            status: oracle_status,
            output_sha256: oracle_sha,
            length: oracle_len,
        },
        native: NativeResult {
            status: native_status,
            output_sha256: native_sha,
            length: native_len,
        },
        comparison: Comparison {
            exact: false,
            first_differing_offset: None,
            differing_bytes: None,
        },
        classification,
        resolution: ResolutionState::Open,
        minimized_casefile: None,
        environment_sha256: environment_sha256(),
    }
}

/// Sub-case 1: Rust persistent stream bytes vs Microsoft stream bytes.
fn run_wire_parity(
    case: &StreamCase,
    all_input_sha: &str,
    input_hashes: &InputHashes,
) -> DifferentialResult {
    let case_id = format!("wire:sha256:{}", all_input_sha);

    let (native_status, native_output) = match rust_stream_encode(case) {
        Ok(bytes) => ("ok".to_string(), bytes),
        Err(e) => (format!("native_error: {}", e), Vec::new()),
    };
    let native_len = native_output.len() as u64;
    let native_sha = sha256(&native_output);

    let oracle_result = oracle::run_stream_oracle_encode(&case.to_binary());
    let (oracle_status, oracle_output) = match &oracle_result {
        Ok(resp) => ("ok".to_string(), resp.raw_output.clone()),
        Err(e) => (format!("oracle_error: {}", e), Vec::new()),
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
        court_id: "MSRTC.STREAM.DIFFERENTIAL".to_string(),
        case_id,
        oracle_commit: oracle::ORACLE_COMMIT.to_string(),
        rust_commit: git_commit(),
        seed: case.seed,
        variant: if case.variant == 1 {
            "RansByte".into()
        } else {
            "Rans64".into()
        },
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

/// Sub-case 2: Microsoft stream → Rust persistent decoder.
fn run_cpp_stream_rust_decode(
    case: &StreamCase,
    all_input_sha: &str,
    input_hashes: &InputHashes,
) -> DifferentialResult {
    let case_id = format!("cpp_stream_rust_dec:sha256:{}", all_input_sha);

    let oracle_result = oracle::run_stream_oracle_encode(&case.to_binary());
    let (oracle_status, oracle_bitstream) = match &oracle_result {
        Ok(resp) => ("ok".to_string(), resp.raw_output.clone()),
        Err(e) => (format!("oracle_error: {}", e), vec![]),
    };

    if oracle_status != "ok" {
        return failure_result(
            "MSRTC.STREAM.DIFFERENTIAL",
            case_id,
            case,
            input_hashes,
            oracle_status.clone(),
            String::new(),
            0,
            String::new(),
            String::new(),
            0,
            oracle::classify_error(&oracle_status),
        );
    }

    let decoded = match rust_stream_decode(case, &oracle_bitstream) {
        Ok(vals) => vals,
        Err(e) => {
            return failure_result(
                "MSRTC.STREAM.DIFFERENTIAL",
                case_id,
                case,
                input_hashes,
                "ok".to_string(),
                oracle_result
                    .as_ref()
                    .map(|r| r.sha256.clone())
                    .unwrap_or_default(),
                oracle_result.as_ref().map(|r| r.length as u64).unwrap_or(0),
                format!("decode_error: {}", e),
                sha256(&oracle_bitstream),
                oracle_bitstream.len() as u64,
                ResidualClassification::NativeBug,
            );
        }
    };

    let expected = expected_values(case);
    let exact = decoded == expected;
    let comparison = if exact {
        Comparison {
            exact: true,
            first_differing_offset: None,
            differing_bytes: None,
        }
    } else {
        Comparison {
            exact: false,
            first_differing_offset: Some(0),
            differing_bytes: Some(count_mismatches(&decoded, &expected) as u64),
        }
    };

    let result = DifferentialResult {
        schema_version: oracle::SCHEMA_VERSION,
        court_id: "MSRTC.STREAM.DIFFERENTIAL".to_string(),
        case_id,
        oracle_commit: oracle::ORACLE_COMMIT.to_string(),
        rust_commit: git_commit(),
        seed: case.seed,
        variant: if case.variant == 1 {
            "RansByte".into()
        } else {
            "Rans64".into()
        },
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

/// Sub-case 3: Rust stream → Microsoft persistent decoder.
fn run_rust_stream_cpp_decode(
    case: &StreamCase,
    all_input_sha: &str,
    input_hashes: &InputHashes,
) -> DifferentialResult {
    let case_id = format!("rust_stream_cpp_dec:sha256:{}", all_input_sha);

    let native_bytes = match rust_stream_encode(case) {
        Ok(bytes) => bytes,
        Err(e) => {
            return failure_result(
                "MSRTC.STREAM.DIFFERENTIAL",
                case_id,
                case,
                input_hashes,
                String::new(),
                String::new(),
                0,
                format!("encode_error: {}", e),
                String::new(),
                0,
                ResidualClassification::NativeBug,
            );
        }
    };

    let oracle_result = oracle::run_stream_oracle_decode(&case.to_binary(), &native_bytes);
    let (oracle_status, decoded) = match &oracle_result {
        Ok(resp) => ("ok".to_string(), resp.values.clone()),
        Err(e) => (format!("oracle_error: {}", e), vec![]),
    };

    if oracle_status != "ok" {
        return failure_result(
            "MSRTC.STREAM.DIFFERENTIAL",
            case_id,
            case,
            input_hashes,
            oracle_status.clone(),
            String::new(),
            0,
            "ok".to_string(),
            sha256(&native_bytes),
            native_bytes.len() as u64,
            oracle::classify_error(&oracle_status),
        );
    }

    let expected = expected_values(case);
    let exact = decoded == expected;
    let comparison = if exact {
        Comparison {
            exact: true,
            first_differing_offset: None,
            differing_bytes: None,
        }
    } else {
        Comparison {
            exact: false,
            first_differing_offset: Some(0),
            differing_bytes: Some(count_mismatches(&decoded, &expected) as u64),
        }
    };

    let result = DifferentialResult {
        schema_version: oracle::SCHEMA_VERSION,
        court_id: "MSRTC.STREAM.DIFFERENTIAL".to_string(),
        case_id,
        oracle_commit: oracle::ORACLE_COMMIT.to_string(),
        rust_commit: git_commit(),
        seed: case.seed,
        variant: if case.variant == 1 {
            "RansByte".into()
        } else {
            "Rans64".into()
        },
        input_hashes: input_hashes.clone(),
        oracle: OracleResult {
            status: "ok".to_string(),
            output_sha256: oracle_result
                .as_ref()
                .map(|r| r.values_sha256.clone())
                .unwrap_or_default(),
            length: 0,
        },
        native: NativeResult {
            status: "ok".to_string(),
            output_sha256: sha256(&native_bytes),
            length: native_bytes.len() as u64,
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

fn count_mismatches(decoded: &[Vec<i32>], expected: &[Vec<i32>]) -> usize {
    decoded
        .iter()
        .zip(expected.iter())
        .map(|(a, b)| a.iter().zip(b.iter()).filter(|(x, y)| x != y).count())
        .sum()
}

// ---------------------------------------------------------------------------
// Test case generation
// ---------------------------------------------------------------------------

/// Reference batch from test_msrtc_rans.py (PMF1).
fn batch_pmf1() -> StreamBatch {
    StreamBatch {
        pmf_lengths: vec![4, 6],
        pmf_offsets: vec![1, 2],
        pmf_table: vec![1, 3, 1, 1, 1, 3, 5, 3, 1, 1],
        indices: vec![0, 1, 0, 1],
        values: vec![-2, 1, 0, 1],
    }
}

/// Reference batch from test_msrtc_rans.py (PMF2).
fn batch_pmf2() -> StreamBatch {
    StreamBatch {
        pmf_lengths: vec![5],
        pmf_offsets: vec![1],
        pmf_table: vec![1, 3, 3, 1, 1],
        indices: vec![0, 0, 0],
        values: vec![-2, 1, 2],
    }
}

fn generate_stream_cases() -> Vec<StreamCase> {
    // A deterministic 256-symbol batch (matches test_rans_encoder_stream_0).
    let indices_256: Vec<i32> = (0..256).map(|i| (i % 2) as i32).collect();
    let values_256: Vec<i32> = (0..256).collect();

    vec![
        // Reference multipart fixture from test_encode_decode_multi_part_0
        StreamCase {
            seed: 10,
            variant: 1,
            symbol_bits: 16,
            bypass_bits: 4,
            batches: vec![batch_pmf2(), batch_pmf1()],
        },
        // Same with Rans64
        StreamCase {
            seed: 11,
            variant: 0,
            symbol_bits: 16,
            bypass_bits: 4,
            batches: vec![batch_pmf2(), batch_pmf1()],
        },
        // Three batches, all RansByte, with a large middle batch
        StreamCase {
            seed: 12,
            variant: 1,
            symbol_bits: 16,
            bypass_bits: 4,
            batches: vec![
                batch_pmf2(),
                StreamBatch {
                    pmf_lengths: vec![4, 6],
                    pmf_offsets: vec![1, 2],
                    pmf_table: vec![1, 3, 1, 1, 1, 3, 5, 3, 1, 1],
                    indices: indices_256.clone(),
                    values: values_256.clone(),
                },
                batch_pmf1(),
            ],
        },
        // Three batches with Rans64 (mirrors the RansByte triple)
        StreamCase {
            seed: 13,
            variant: 0,
            symbol_bits: 16,
            bypass_bits: 4,
            batches: vec![
                batch_pmf2(),
                StreamBatch {
                    pmf_lengths: vec![4, 6],
                    pmf_offsets: vec![1, 2],
                    pmf_table: vec![1, 3, 1, 1, 1, 3, 5, 3, 1, 1],
                    indices: indices_256.clone(),
                    values: values_256.clone(),
                },
                batch_pmf1(),
            ],
        },
        // Single-batch stream (degenerate multipart: one push)
        StreamCase {
            seed: 14,
            variant: 1,
            symbol_bits: 16,
            bypass_bits: 4,
            batches: vec![batch_pmf1()],
        },
        // Single-batch Rans64
        StreamCase {
            seed: 15,
            variant: 0,
            symbol_bits: 16,
            bypass_bits: 4,
            batches: vec![batch_pmf1()],
        },
        // Small bypass bits (2) exercises multi-digit bypass counts
        StreamCase {
            seed: 16,
            variant: 1,
            symbol_bits: 16,
            bypass_bits: 2,
            batches: vec![
                StreamBatch {
                    pmf_lengths: vec![5],
                    pmf_offsets: vec![1],
                    pmf_table: vec![1, 3, 3, 1, 1],
                    indices: vec![0, 0, 0],
                    values: vec![-2, 1, 2],
                },
                batch_pmf1(),
            ],
        },
        // Repeated push of the SAME batch (state continuity stress)
        StreamCase {
            seed: 17,
            variant: 1,
            symbol_bits: 16,
            bypass_bits: 4,
            batches: vec![batch_pmf1(), batch_pmf1(), batch_pmf1()],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_case_serialization() {
        let c = StreamCase {
            seed: 0,
            variant: 1,
            symbol_bits: 16,
            bypass_bits: 4,
            batches: vec![batch_pmf2(), batch_pmf1()],
        };
        let bin = c.to_binary();
        assert!(!bin.is_empty());
        assert_eq!(&bin[..4], &[1u8, 0, 0, 0]); // RansByte
        // batch_count == 2 at offset 12
        assert_eq!(&bin[12..16], &[2u8, 0, 0, 0]);
    }

    #[test]
    fn test_stream_input_hashes() {
        let c = StreamCase {
            seed: 0,
            variant: 1,
            symbol_bits: 16,
            bypass_bits: 4,
            batches: vec![batch_pmf2(), batch_pmf1()],
        };
        let h = c.input_hashes();
        assert!(!h.pmf_lengths_sha256.is_empty());
        assert!(!h.indices_sha256.is_empty());
        // Single-batch pmf1 must differ from two-batch hashes
        let c2 = StreamCase {
            seed: 0,
            variant: 1,
            symbol_bits: 16,
            bypass_bits: 4,
            batches: vec![batch_pmf1()],
        };
        let h2 = c2.input_hashes();
        assert_ne!(h.pmf_table_sha256, h2.pmf_table_sha256);
    }

    #[test]
    fn test_rust_stream_roundtrip() {
        // Internal self-consistency: Rust encode → Rust decode must recover
        // all values and reach EOF. No oracle required.
        for case in generate_stream_cases() {
            let bytes = rust_stream_encode(&case).expect("encode");
            assert!(!bytes.is_empty());
            let decoded = rust_stream_decode(&case, &bytes).expect("decode");
            assert_eq!(decoded, expected_values(&case), "seed {}", case.seed);
        }
    }

    #[test]
    fn test_stream_court_generates_cases() {
        let court = StreamDifferentialCourt;
        let result = court.run();

        // Without Docker the oracle errors are environmental residuals; only
        // check that cases were generated.
        assert!(result.case_count > 0);
    }

    #[test]
    #[ignore = "requires Docker oracle container"]
    fn test_stream_court_full_differential() {
        let court = StreamDifferentialCourt;
        let result = court.run();
        assert_eq!(result.status, CourtStatus::Passed);
        assert!(result.is_sealable());
    }
}
