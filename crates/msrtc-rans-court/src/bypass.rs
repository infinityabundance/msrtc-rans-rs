// Copyright (c) 2026 Riaan de Beer
// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! MSRTC.BYPASS court
//!
//! Proves exact out-of-range coding for bypass values.

use crate::{Court, CourtResult};

/// The Bypass court.
pub struct BypassCourt;

impl Court for BypassCourt {
    fn id(&self) -> &str {
        "MSRTC.BYPASS"
    }

    fn run(&self) -> CourtResult {
        CourtResult::scaffold(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bypass_court_is_scaffold() {
        let court = BypassCourt;
        let r = court.run();
        assert!(!r.is_sealable());
    }
}
