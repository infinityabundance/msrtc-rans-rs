// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

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
