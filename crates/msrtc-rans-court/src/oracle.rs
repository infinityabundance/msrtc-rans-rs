// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! Shared oracle transport and forensic utilities for differential courts.
//!
//! Provides:
//! - `run_oracle()` — executes the oracle CLI in Docker and parses its response
//! - `compute_input_hashes()` — SHA-256 of canonical little-endian array bytes
//! - `environment_sha256()` — fingerprint of the execution environment
//! - `git_commit()` — full SHA at build time with dirty status
//! - `write_residual()` — persists a DifferentialResult to `courts/residuals/`

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use msrtc_rans_casefile::{
    Comparison, DifferentialResult,
    classification::ResidualClassification,
    sha256,
};

/// Oracle commit from upstream.lock.
pub const ORACLE_COMMIT: &str = "0500356a8d6146dd8dc8911022cbeca19675614f";

/// Docker image for the oracle.
pub const ORACLE_IMAGE: &str = "msrtc-rans-rs-oracle:debian12";

/// Schema version for all results.
pub const SCHEMA_VERSION: u32 = 1;

/// Root path for residual storage (relative to workspace root).
pub const RESIDUALS_DIR: &str = "../courts/residuals";

// ---------------------------------------------------------------------------
// Oracle execution
// ---------------------------------------------------------------------------

/// Result of running the oracle CLI.
#[derive(Debug, Clone)]
pub struct OracleResponse {
    pub status: String,
    pub hex: String,
    pub sha256: String,
    pub length: usize,
    pub raw_output: Vec<u8>,
}

/// Run the oracle CLI inside Docker with the given binary casefile data.
/// Returns the parsed metadata and raw bitstream.
pub fn run_oracle(binary: &[u8]) -> Result<OracleResponse, String> {
    let mut child = Command::new("docker")
        .args([
            "run",
            "-i",
            "--rm",
            ORACLE_IMAGE,
            "/workspace/bin/oracle_cli",
            "/dev/stdin",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("docker_spawn: {}", e))?;

    // Write casefile to stdin and close the pipe
    if let Some(ref mut stdin) = child.stdin {
        stdin
            .write_all(binary)
            .map_err(|e| format!("stdin_write: {}", e))?;
    }
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|e| format!("docker_wait: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "oracle_exit_{}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    let bitstream = output.stdout;
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    let last_line = stderr_str.lines().last().unwrap_or("");

    // Parse the JSON metadata strictly
    let parsed: serde_json::Value = serde_json::from_str(last_line)
        .map_err(|e| format!("oracle_json_parse: {} — line: {}", e, last_line))?;

    let status = parsed["status"].as_str().unwrap_or("unknown").to_string();
    if status != "ok" {
        let msg = parsed["message"].as_str().unwrap_or("unknown error");
        return Err(format!("oracle_error: {}", msg));
    }

    let hex = parsed["hex"].as_str().unwrap_or("").to_string();
    let sha = parsed["sha256"].as_str().unwrap_or("").to_string();
    let length = parsed["length"].as_u64().unwrap_or(0) as usize;

    // Validate: reported length must match stdout size
    if length != bitstream.len() {
        return Err(format!(
            "oracle_length_mismatch: reported {} but stdout has {} bytes",
            length,
            bitstream.len()
        ));
    }

    // Validate: reported SHA-256 must match stdout
    let actual_sha = sha256(&bitstream);
    if sha != actual_sha {
        return Err(format!(
            "oracle_sha256_mismatch: reported {} but computed {}",
            sha, actual_sha
        ));
    }

    Ok(OracleResponse {
        status,
        hex,
        sha256: sha,
        length,
        raw_output: bitstream,
    })
}

/// Run the raw oracle CLI inside Docker with the given binary input.
pub fn run_raw_oracle(binary: &[u8]) -> Result<OracleResponse, String> {
    let mut child = Command::new("docker")
        .args([
            "run",
            "-i",
            "--rm",
            ORACLE_IMAGE,
            "/workspace/bin/raw_oracle_cli",
            "/dev/stdin",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("docker_spawn: {}", e))?;

    if let Some(ref mut stdin) = child.stdin {
        stdin
            .write_all(binary)
            .map_err(|e| format!("stdin_write: {}", e))?;
    }
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|e| format!("docker_wait: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "raw_oracle_exit_{}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    let bitstream = output.stdout;
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    let last_line = stderr_str.lines().last().unwrap_or("");

    let parsed: serde_json::Value = serde_json::from_str(last_line)
        .map_err(|e| format!("raw_oracle_json_parse: {} — line: {}", e, last_line))?;

    let status = parsed["status"].as_str().unwrap_or("unknown").to_string();
    if status != "ok" {
        let msg = parsed["message"].as_str().unwrap_or("unknown error");
        return Err(format!("raw_oracle_error: {}", msg));
    }

    let hex = parsed["hex"].as_str().unwrap_or("").to_string();
    let sha = parsed["sha256"].as_str().unwrap_or("").to_string();
    let length = parsed["length"].as_u64().unwrap_or(0) as usize;

    if length != bitstream.len() {
        return Err(format!(
            "raw_oracle_length_mismatch: reported {} but stdout has {}",
            length,
            bitstream.len()
        ));
    }

    Ok(OracleResponse {
        status,
        hex,
        sha256: sha,
        length,
        raw_output: bitstream,
    })
}

// ---------------------------------------------------------------------------
// Input hashing
// ---------------------------------------------------------------------------

/// Compute SHA-256 of canonical little-endian bytes for an i32 array.
pub fn hash_i32_array(arr: &[i32]) -> String {
    let le_bytes: Vec<u8> = arr.iter().flat_map(|v| v.to_le_bytes()).collect();
    sha256(&le_bytes)
}

/// Compute SHA-256 of canonical little-endian bytes for a u32 array.
pub fn hash_u32_array(arr: &[u32]) -> String {
    let le_bytes: Vec<u8> = arr.iter().flat_map(|v| v.to_le_bytes()).collect();
    sha256(&le_bytes)
}

// ---------------------------------------------------------------------------
// Environment fingerprint
// ---------------------------------------------------------------------------

/// Compute a SHA-256 environment fingerprint from available provenance data.
pub fn environment_sha256() -> String {
    let mut input = Vec::new();

    writeln!(input, "oracle_image={}", ORACLE_IMAGE).ok();
    writeln!(input, "oracle_commit={}", ORACLE_COMMIT).ok();
    writeln!(input, "rust_version={}", env!("CARGO_PKG_RUST_VERSION")).ok();
    writeln!(input, "git_hash={}", git_commit()).ok();
    writeln!(
        input,
        "target={}-{}",
        std::env::consts::ARCH,
        std::env::consts::OS
    )
    .ok();

    msrtc_rans_casefile::sha256(&input)
}

// ---------------------------------------------------------------------------
// Git commit
// ---------------------------------------------------------------------------

/// Full Git commit SHA and dirty status, captured at build time.
pub fn git_commit() -> String {
    let hash = env!("GIT_HASH");
    let dirty = env!("GIT_DIRTY");
    if dirty == "true" {
        format!("{}-dirty", hash)
    } else {
        hash.to_string()
    }
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

/// Compare two byte slices and produce a structured Comparison.
pub fn compare_bytes(native: &[u8], oracle: &[u8]) -> Comparison {
    if native == oracle {
        return Comparison {
            exact: true,
            first_differing_offset: None,
            differing_bytes: None,
        };
    }

    let min_len = native.len().min(oracle.len());
    let first_diff = (0..min_len)
        .find(|&i| native[i] != oracle[i])
        .map(|i| i as u64);

    // Count differing overlapping positions
    let overlapping_diffs = (0..min_len).filter(|&i| native[i] != oracle[i]).count() as u64;

    // Absolute length difference
    let len_diff = if native.len() != oracle.len() {
        native.len().max(oracle.len()) as u64 - native.len().min(oracle.len()) as u64
    } else {
        0
    };

    Comparison {
        exact: false,
        first_differing_offset: Some(first_diff.unwrap_or(min_len as u64)),
        differing_bytes: Some(overlapping_diffs + len_diff),
    }
}

// ---------------------------------------------------------------------------
// Residual persistence
// ---------------------------------------------------------------------------

/// Write a DifferentialResult to the residuals directory as a JSON file.
/// The filename is derived from the court_id and case_id.
pub fn write_residual(result: &DifferentialResult) -> std::io::Result<()> {
    let dir = Path::new(RESIDUALS_DIR);
    std::fs::create_dir_all(dir)?;

    // Sanitize case_id for filename
    let case_id_sanitized: String = result
        .case_id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    let class_str = match &result.classification {
        ResidualClassification::NativeBug => "native_bug",
        ResidualClassification::OracleBug => "oracle_bug",
        ResidualClassification::OracleUndefinedOrAssertOnly => "oracle_ub",
        ResidualClassification::InvalidInputPolicy => "invalid_input",
        ResidualClassification::PlatformOrEndianDivergence => "endian",
        ResidualClassification::PythonApiDivergence => "python_api",
        ResidualClassification::PerformanceResidual => "perf",
        ResidualClassification::IntentionalSafetyDivergence => "safety",
        ResidualClassification::Environmental => "environmental",
        ResidualClassification::Unclassified => "unclassified",
    };

    let filename = format!(
        "{}_{}_{}.json",
        result.court_id.replace('.', "_"),
        case_id_sanitized.chars().take(40).collect::<String>(),
        class_str,
    );

    let path = dir.join(&filename);
    let json = serde_json::to_string_pretty(result)?;
    std::fs::write(&path, json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Classification helper
// ---------------------------------------------------------------------------

/// Determine the appropriate residual classification for a failed comparison.
pub fn classify_mismatch(
    native_status: &str,
    oracle_status: Result<&OracleResponse, &str>,
) -> ResidualClassification {
    match oracle_status {
        Err(e) if e.contains("docker") => ResidualClassification::Environmental,
        Err(_) => ResidualClassification::OracleBug,
        Ok(_) if native_status != "ok" => ResidualClassification::NativeBug,
        Ok(_) => ResidualClassification::Unclassified,
    }
}

/// Determine the default classification when oracle result is a string error.
pub fn classify_error(error_str: &str) -> ResidualClassification {
    if error_str.contains("docker") || error_str.contains("spawn") {
        ResidualClassification::Environmental
    } else if error_str.contains("oracle_exit") || error_str.contains("oracle_json") {
        ResidualClassification::OracleBug
    } else {
        ResidualClassification::Unclassified
    }
}
