// Copyright (c) 2026 Riaan de Beer
// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

#![allow(missing_docs)]

//! # msrtc-rans-court
//!
//! Differential forensic courts for msrtc_rans parity verification.
//!
//! This crate implements the forensic courts that compare the native Rust
//! implementation against the pinned Microsoft C++ oracle. Each court
//! produces machine-readable receipts, human-readable transcripts, and
//! structured residuals for every discovered difference.

#![forbid(unsafe_code)]

use msrtc_rans_casefile::DifferentialResult;

/// Status of a court.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CourtStatus {
    /// Court is scaffolding — not yet implemented.
    Scaffold,
    /// Court ran and all cases passed.
    Passed,
    /// Court ran and some cases failed or produced residuals.
    Failed,
    /// Court was skipped.
    Skipped,
}

impl CourtStatus {
    /// Returns true if the court is in a sealable state (ran and passed).
    pub fn is_sealable(&self) -> bool {
        matches!(self, CourtStatus::Passed)
    }
}

/// Trait implemented by every forensic court.
pub trait Court {
    /// The unique court identifier (e.g., "MSRTC.RAW.RANSBYTE").
    fn id(&self) -> &str;

    /// Run the court and produce results.
    fn run(&self) -> CourtResult;
}

/// Results from running a court.
#[derive(Debug, Clone)]
pub struct CourtResult {
    /// Court identifier
    pub court_id: String,
    /// Court status
    pub status: CourtStatus,
    /// Number of cases
    pub case_count: u64,
    /// Number of passing cases
    pub pass_count: u64,
    /// Number of residual (failing) cases
    pub residual_count: u64,
    /// Number of skipped cases
    pub skipped_count: u64,
    /// Individual differential results
    pub results: Vec<DifferentialResult>,
}

impl CourtResult {
    /// Create a scaffold result (no cases run).
    pub fn scaffold(court_id: &str) -> Self {
        Self {
            court_id: court_id.to_string(),
            status: CourtStatus::Scaffold,
            case_count: 0,
            pass_count: 0,
            residual_count: 0,
            skipped_count: 0,
            results: vec![],
        }
    }

    /// Returns true if the court can be sealed.
    /// A court is sealable only if:
    /// - It ran cases (`case_count > 0`)
    /// - All cases are accounted for (`pass_count + residual_count + skipped_count == case_count`)
    /// - There are no residuals (`residual_count == 0`)
    pub fn is_sealable(&self) -> bool {
        if self.case_count == 0 {
            return false;
        }
        if self.pass_count + self.residual_count + self.skipped_count != self.case_count {
            return false;
        }
        self.residual_count == 0
    }
}

/// Bypass court — MSRTC.BYPASS
pub mod bypass;
/// PMF court — MSRTC.PMF
pub mod pmf;
/// Raw rANS 64 court — MSRTC.RAW.RANS64
pub mod raw_rans64;
/// Raw rANS byte court — MSRTC.RAW.RANSBYTE
pub mod raw_ransbyte;
/// Reciprocal court — MSRTC.RECIPROCAL
pub mod reciprocal;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_court_ids_are_stable() {
        let ids = [
            "MSRTC.RAW.RANSBYTE",
            "MSRTC.RAW.RANS64",
            "MSRTC.RECIPROCAL",
            "MSRTC.PMF",
            "MSRTC.BYPASS",
        ];
        for id in ids {
            assert!(
                id.starts_with("MSRTC."),
                "Court ID must start with MSRTC.: {}",
                id
            );
        }
    }

    #[test]
    fn test_scaffold_is_not_sealable() {
        let r = CourtResult::scaffold("MSRTC.TEST");
        assert!(!r.is_sealable());
    }

    #[test]
    fn test_zero_case_is_not_sealable() {
        let r = CourtResult {
            court_id: "MSRTC.TEST".into(),
            status: CourtStatus::Passed,
            case_count: 0,
            pass_count: 0,
            residual_count: 0,
            skipped_count: 0,
            results: vec![],
        };
        assert!(!r.is_sealable());
    }

    #[test]
    fn test_passing_court_is_sealable() {
        let r = CourtResult {
            court_id: "MSRTC.TEST".into(),
            status: CourtStatus::Passed,
            case_count: 10,
            pass_count: 10,
            residual_count: 0,
            skipped_count: 0,
            results: vec![],
        };
        assert!(r.is_sealable());
    }

    #[test]
    fn test_failing_court_is_not_sealable() {
        let r = CourtResult {
            court_id: "MSRTC.TEST".into(),
            status: CourtStatus::Failed,
            case_count: 10,
            pass_count: 8,
            residual_count: 2,
            skipped_count: 0,
            results: vec![],
        };
        assert!(!r.is_sealable());
    }
}
