// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! MSRTC.RECIPROCAL court
//!
//! Compares three implementations:
//! 1. Mathematical division reference
//! 2. Microsoft reciprocal-multiply oracle
//! 3. Optimized Rust reciprocal-multiply path

use crate::{Court, CourtResult};

/// The Reciprocal court.
pub struct ReciprocalCourt;

impl Court for ReciprocalCourt {
    fn id(&self) -> &str {
        "MSRTC.RECIPROCAL"
    }

    fn run(&self) -> CourtResult {
        CourtResult::scaffold(self.id())
    }
}

#[cfg(test)]
mod tests {
    use msrtc_rans_core::arithmetic;

    #[test]
    fn test_u64_reciprocal_3() {
        // freq=3 should produce a reciprocal such that
        // mul_hi(x, rcp) >> shift == x / 3 for all x up to 2^31
        let freq: u32 = 3;
        let rcp = arithmetic::compute_reciprocal_u64(freq);
        let shift = arithmetic::reciprocal_shift(freq) - 1;

        for x in [1u64, 10, 100, 1u64 << 20, 1u64 << 30] {
            let fast = arithmetic::fast_quotient_u64(x, rcp, shift);
            let exact = x / freq as u64;
            assert_eq!(fast, exact, "freq={} x={}", freq, x);
        }
    }

    #[test]
    fn test_u32_reciprocal_3() {
        let freq: u32 = 3;
        let rcp = arithmetic::compute_reciprocal_u32(freq);
        let rcp_shift = arithmetic::reciprocal_shift(freq) - 1 + 32;

        for x in [1u32, 10, 100, 1u32 << 20, 1u32 << 30] {
            let fast = arithmetic::fast_quotient_u32(x, rcp, rcp_shift);
            let exact = x / freq;
            assert_eq!(fast, exact, "freq={} x={}", freq, x);
        }
    }

    #[test]
    fn test_freq_one_special_case() {
        let (rcp, shift) = arithmetic::freq_one_reciprocal_u64();
        assert_eq!(rcp, !0u64);
        assert_eq!(shift, 0);

        // For freq=1: q = mul_hi(x, !0) >> 0 = x - 1 (for x > 0)
        let x: u64 = 100;
        let q = arithmetic::fast_quotient_u64(x, rcp, shift);
        assert_eq!(q, x - 1);
    }
}
