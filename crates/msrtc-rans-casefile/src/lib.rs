// Copyright (c) 2026 Riaan de Beer
// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # msrtc-rans-casefile
//!
//! Deterministic casefile and residual formats for the msrtc_rans forensic courts.
//!
//! This crate defines the structured data formats used for:
//! - Casefiles: deterministic test inputs and expected outputs
//! - Residuals: structured mismatch records
//! - Receipts: sealed court evidence
//! - Transcripts: human-readable court proceedings

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Residual classification types.
pub mod classification {
    /// Supported residual classifications matching the forensic specification.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub enum ResidualClassification {
        /// Bug in the native Rust implementation
        #[serde(rename = "native_bug")]
        NativeBug,
        /// Bug in the oracle C++ implementation
        #[serde(rename = "oracle_bug")]
        OracleBug,
        /// Oracle depends on undefined behavior or debug assertions only
        #[serde(rename = "oracle_undefined_or_assert_only")]
        OracleUndefinedOrAssertOnly,
        /// Different invalid-input handling policies
        #[serde(rename = "invalid_input_policy")]
        InvalidInputPolicy,
        /// Platform or endianness divergence
        #[serde(rename = "platform_or_endian_divergence")]
        PlatformOrEndianDivergence,
        /// Python API behavioral divergence
        #[serde(rename = "python_api_divergence")]
        PythonApiDivergence,
        /// Performance residual (not a correctness issue)
        #[serde(rename = "performance_residual")]
        PerformanceResidual,
        /// Intentional safety divergence (e.g., Rust rejects undefined behavior)
        #[serde(rename = "intentional_safety_divergence")]
        IntentionalSafetyDivergence,
        /// Environment-related divergence
        #[serde(rename = "environmental")]
        Environmental,
        /// Classification not yet determined
        #[serde(rename = "unclassified")]
        Unclassified,
    }

    /// Resolution states for residuals.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub enum ResolutionState {
        /// Newly discovered, not yet investigated
        #[serde(rename = "open")]
        Open,
        /// Successfully reproduced
        #[serde(rename = "reproduced")]
        Reproduced,
        /// Minimized to smallest reproducer
        #[serde(rename = "minimized")]
        Minimized,
        /// Root cause identified and explained
        #[serde(rename = "explained")]
        Explained,
        /// Corrective action applied
        #[serde(rename = "fixed")]
        Fixed,
        /// Proved and permanently recorded
        #[serde(rename = "sealed")]
        Sealed,
    }
}

/// Court receipt structure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CourtReceipt {
    /// Schema version
    pub schema_version: u32,
    /// Court identifier
    pub court_id: String,
    /// Case count
    #[serde(default)]
    pub case_count: u64,
    /// Pass count
    #[serde(default)]
    pub pass_count: u64,
    /// Residual count
    #[serde(default)]
    pub residual_count: u64,
    /// Skipped count
    #[serde(default)]
    pub skipped_count: u64,
    /// Skip reasons
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_reasons: Option<Vec<String>>,
    /// Timestamp
    pub timestamp: String,
    /// Rust commit
    pub rust_commit: String,
    /// Oracle commit
    pub oracle_commit: String,
    /// Environment fingerprint
    pub environment_sha256: String,
    /// Transcript hash
    pub transcript_hash: String,
    /// Docker provenance
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docker: Option<DockerProvenance>,
}

/// Docker provenance for court receipts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DockerProvenance {
    /// Docker project name
    pub project_name: String,
    /// Run identifier
    pub run_id: String,
    /// Container ID
    pub container_id: String,
    /// Container name
    pub container_name: String,
    /// Image ID
    pub image_id: String,
    /// Image digest
    pub image_digest: String,
    /// Platform
    pub platform: String,
    /// Distribution
    pub distribution: String,
    /// Distribution version
    pub distribution_version: String,
    /// Network mode
    pub network_mode: String,
    /// Whether privileged
    pub privileged: bool,
    /// Storage root
    pub storage_root: String,
}

/// A single differential comparison result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DifferentialResult {
    /// Schema version
    pub schema_version: u32,
    /// Court ID
    pub court_id: String,
    /// Case ID (content hash)
    pub case_id: String,
    /// Oracle commit
    pub oracle_commit: String,
    /// Rust commit
    pub rust_commit: String,
    /// Random seed
    #[serde(default)]
    pub seed: u64,
    /// Variant
    pub variant: String,
    /// Input hashes
    pub input_hashes: InputHashes,
    /// Oracle result
    pub oracle: OracleResult,
    /// Native implementation result
    pub native: NativeResult,
    /// Comparison
    pub comparison: Comparison,
    /// Classification (typed enum, not free text)
    pub classification: classification::ResidualClassification,
    /// Resolution state (typed enum, not free text)
    pub resolution: classification::ResolutionState,
    /// Path to minimized casefile
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimized_casefile: Option<String>,
    /// Environment hash
    pub environment_sha256: String,
}

/// Input hashes for a casefile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InputHashes {
    /// PMF lengths hash
    pub pmf_lengths_sha256: String,
    /// PMF offsets hash
    pub pmf_offsets_sha256: String,
    /// PMF table hash
    pub pmf_table_sha256: String,
    /// Indices hash
    pub indices_sha256: String,
    /// Values hash
    pub values_sha256: String,
}

/// Oracle test result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OracleResult {
    /// Status
    pub status: String,
    /// Output hash
    pub output_sha256: String,
    /// Output length
    pub length: u64,
}

/// Native implementation test result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NativeResult {
    /// Status
    pub status: String,
    /// Output hash
    pub output_sha256: String,
    /// Output length
    pub length: u64,
}

/// Comparison between oracle and native results.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Comparison {
    /// Whether outputs are exactly equal
    pub exact: bool,
    /// First differing byte offset (None if exact match)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_differing_offset: Option<u64>,
    /// Number of differing bytes (None if exact match)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub differing_bytes: Option<u64>,
}

/// Generate SHA-256 hash for a byte slice.
pub fn sha256(data: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    hex::encode(&hash)
}

/// Hex encoding for hash display.
mod hex {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";

    pub fn encode(data: &[u8]) -> String {
        let mut result = vec![0u8; data.len() * 2];
        for (i, &byte) in data.iter().enumerate() {
            result[i * 2] = HEX_CHARS[(byte >> 4) as usize];
            result[i * 2 + 1] = HEX_CHARS[(byte & 0x0F) as usize];
        }
        String::from_utf8(result).expect("hex encoding should produce valid UTF-8")
    }
}
