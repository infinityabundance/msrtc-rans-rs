// Copyright (c) Infinity Abundance.
// Licensed under the MIT license.

//! MSRTC.RAW.RANS64 court
//!
//! Differentially proves the raw u64-state, u32-unit rANS variant.

use crate::{Court, CourtResult};

/// The Rans64 court.
pub struct Rans64Court;

impl Court for Rans64Court {
    fn id(&self) -> &str {
        "MSRTC.RAW.RANS64"
    }

    fn run(&self) -> CourtResult {
        CourtResult::scaffold(self.id())
    }
}

#[cfg(test)]
mod tests {
    use msrtc_rans_core::Rans64Encoder;
    use msrtc_rans_core::sink::VecSink;

    #[test]
    fn test_rans64_encoder_creation() {
        let sink = VecSink::<u32>::new(64);
        let encoder = Rans64Encoder::new(sink);
        assert_eq!(encoder.state(), 1u64 << 31);
    }
}
