// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # Seal — Run-scoped receipt, transcript, and manifest generation
//!
//! Produces a sealed record of a court's run for audit and evidence purposes.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use msrtc_rans_casefile::sha256;

use crate::CourtResult;
use crate::oracle::{self, environment_sha256, git_commit};

/// A sealed receipt summarising a single court run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Receipt {
    /// Court identifier
    pub court_id: String,
    /// Run identifier: `<timestamp>_<short_commit>`
    pub run_id: String,
    /// ISO-8601-like timestamp of the run
    pub timestamp: String,
    /// Number of cases
    pub case_count: u64,
    /// Number of passing cases
    pub pass_count: u64,
    /// Number of residual (failing) cases
    pub residual_count: u64,
    /// Number of skipped cases
    pub skipped_count: u64,
    /// Aggregate result
    pub result: String,
    /// Oracle commit (full SHA)
    pub oracle_commit: String,
    /// Rust commit (full SHA, with -dirty suffix if applicable)
    pub rust_commit: String,
    /// Docker image digest
    pub docker_image_digest: String,
    /// Environment fingerprint
    pub environment_sha256: String,
    /// Commands used during the run
    pub commands: Vec<String>,
    /// Per-case summaries
    pub cases: Vec<CaseSummary>,
}

/// Summary of a single case result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CaseSummary {
    /// Case identifier (content hash)
    pub case_id: String,
    /// Status string ("ok", "native_error", "oracle_error")
    pub status: String,
    /// Oracle output SHA-256
    pub oracle_sha256: String,
    /// Native output SHA-256
    pub native_sha256: String,
}

/// Produce the three sealed artefacts for a court's result:
///
/// 1. `courts/receipts/MSRTC_<COURT>_<run_id>.json`
/// 2. `courts/transcripts/MSRTC_<COURT>_<run_id>.txt`
/// 3. `courts/manifests/MSRTC_<COURT>_<run_id>.json`
pub fn seal(result: &CourtResult) -> std::io::Result<Receipt> {
    let rust_commit = git_commit();
    let timestamp = formatted_timestamp();
    let run_id = format!("{}_{}", timestamp, short_commit(&rust_commit));
    let docker_digest = resolve_docker_digest();

    let aggregate = if result.is_sealable() {
        "PASS".to_string()
    } else {
        format!(
            "FAIL: {} pass, {} residual, {} skipped / {} total",
            result.pass_count, result.residual_count, result.skipped_count, result.case_count
        )
    };

    let cases: Vec<CaseSummary> = result
        .results
        .iter()
        .map(|r| {
            let status = if r.comparison.exact {
                r.oracle.status.clone()
            } else {
                format!("{} / {}", r.native.status, r.oracle.status)
            };
            CaseSummary {
                case_id: r.case_id.clone(),
                status,
                oracle_sha256: r.oracle.output_sha256.clone(),
                native_sha256: r.native.output_sha256.clone(),
            }
        })
        .collect();

    let receipt = Receipt {
        court_id: result.court_id.clone(),
        run_id: run_id.clone(),
        timestamp: timestamp.clone(),
        case_count: result.case_count,
        pass_count: result.pass_count,
        residual_count: result.residual_count,
        skipped_count: result.skipped_count,
        result: aggregate,
        oracle_commit: oracle::ORACLE_COMMIT.to_string(),
        rust_commit: rust_commit.clone(),
        docker_image_digest: docker_digest,
        environment_sha256: environment_sha256(),
        commands: vec![
            format!("cargo run --bin {} -- court", result.court_id),
            "docker run -i --rm msrtc-rans-rs-oracle:debian12".to_string(),
        ],
        cases,
    };

    // Build absolute paths from CARGO_MANIFEST_DIR
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../courts")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../courts"));

    let court_slug = result.court_id.replace('.', "_");
    let stem = format!("MSRTC_{}_{}", court_slug, run_id);

    // --- Receipt ---
    let receipt_dir = base.join("receipts");
    std::fs::create_dir_all(&receipt_dir)?;
    let receipt_path = receipt_dir.join(format!("{}.json", stem));
    let receipt_json = serde_json::to_string_pretty(&receipt)?;
    std::fs::write(&receipt_path, &receipt_json)?;

    // --- Transcript ---
    let transcript_dir = base.join("transcripts");
    std::fs::create_dir_all(&transcript_dir)?;
    let transcript_path = transcript_dir.join(format!("{}.txt", stem));
    let transcript = build_transcript(result, &receipt);
    std::fs::write(&transcript_path, &transcript)?;

    // --- Manifest ---
    let manifest_dir = base.join("manifests");
    std::fs::create_dir_all(&manifest_dir)?;
    let manifest_path = manifest_dir.join(format!("{}.json", stem));
    let manifest = build_manifest(result, &receipt);
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, &manifest_json)?;

    Ok(receipt)
}

fn formatted_timestamp() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Produce YYYYMMDDTHHMMSS in UTC
    let days_since_epoch = secs / 86400;
    let time_secs = secs % 86400;

    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Simple leap-day-aware date from days since epoch
    let (year, month, day) = days_to_date(days_since_epoch as i64);

    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}",
        year, month, day, hours, minutes, seconds
    )
}

fn days_to_date(mut days: i64) -> (i64, i64, i64) {
    // Based on the algorithm from Howard Hinnant
    days += 719468; // shift epoch from 1970-01-01 to 0000-03-01
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = days - era * 146097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month phase [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as i64, d as i64)
}

fn short_commit(commit: &str) -> &str {
    // Strip -dirty suffix, take first 12 chars
    let clean = commit.strip_suffix("-dirty").unwrap_or(commit);
    &clean[..clean.len().min(12)]
}

fn resolve_docker_digest() -> String {
    // Try to get the actual digest from Docker
    let output = std::process::Command::new("docker")
        .args([
            "image",
            "inspect",
            oracle::ORACLE_IMAGE,
            "--format",
            "{{.Id}}",
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => format!("sha256:{}", sha256(oracle::ORACLE_IMAGE.as_bytes())),
    }
}

fn build_transcript(result: &CourtResult, receipt: &Receipt) -> String {
    let mut buf = Vec::new();

    writeln!(buf, "MSRTC Court Transcript").ok();
    writeln!(buf, "=====================").ok();
    writeln!(buf).ok();
    writeln!(buf, "Court:       {}", result.court_id).ok();
    writeln!(buf, "Run ID:      {}", receipt.run_id).ok();
    writeln!(buf, "Timestamp:   {}", receipt.timestamp).ok();
    writeln!(buf, "Rust Commit: {}", receipt.rust_commit).ok();
    writeln!(buf, "Oracle Cmt:  {}", receipt.oracle_commit).ok();
    writeln!(buf, "Environment: {}", receipt.environment_sha256).ok();
    writeln!(buf).ok();
    writeln!(
        buf,
        "Cases: {} total, {} pass, {} residual, {} skipped",
        result.case_count, result.pass_count, result.residual_count, result.skipped_count
    )
    .ok();
    writeln!(buf, "Result: {}", receipt.result).ok();
    writeln!(buf).ok();

    for (i, r) in result.results.iter().enumerate() {
        let mark = if r.comparison.exact { "PASS" } else { "FAIL" };
        writeln!(buf, "--- Case {} ---", i + 1).ok();
        writeln!(buf, "ID:     {}", r.case_id).ok();
        writeln!(buf, "Status: {}", mark).ok();
        writeln!(
            buf,
            "Oracle: {} (sha256: {})",
            r.oracle.status, r.oracle.output_sha256
        )
        .ok();
        writeln!(
            buf,
            "Native: {} (sha256: {})",
            r.native.status, r.native.output_sha256
        )
        .ok();
        writeln!(buf, "Seed:   {}", r.seed).ok();
        writeln!(buf, "Variant: {}", r.variant).ok();
        if !r.comparison.exact {
            if let Some(offset) = r.comparison.first_differing_offset {
                writeln!(buf, "First diff at byte: {}", offset).ok();
            }
            if let Some(count) = r.comparison.differing_bytes {
                writeln!(buf, "Differing bytes: {}", count).ok();
            }
        }
        writeln!(buf).ok();
    }

    String::from_utf8(buf).unwrap_or_else(|_| "transcript encoding error".to_string())
}

fn build_manifest(result: &CourtResult, receipt: &Receipt) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "court_id": result.court_id,
        "run_id": receipt.run_id,
        "timestamp": receipt.timestamp,
        "case_count": result.case_count,
        "pass_count": result.pass_count,
        "residual_count": result.residual_count,
        "skipped_count": result.skipped_count,
        "result": receipt.result,
        "rust_commit": receipt.rust_commit,
        "oracle_commit": receipt.oracle_commit,
        "environment_sha256": receipt.environment_sha256,
        "docker_image_digest": receipt.docker_image_digest,
    })
}
