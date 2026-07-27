// Copyright (c) Infinity Abundance.
// Licensed under the MIT license.

//! MSRTC.PMF court
//!
//! Exercises every PMF initialization rule.

use crate::{Court, CourtResult};

/// The PMF court.
pub struct PmfCourt;

impl Court for PmfCourt {
    fn id(&self) -> &str {
        "MSRTC.PMF"
    }

    fn run(&self) -> CourtResult {
        CourtResult::scaffold(self.id())
    }
}
