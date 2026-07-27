// Copyright (c) 2026 Riaan de Beer
// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

#![allow(missing_docs)]

//! Arithmetic primitives for rANS.
//!
//! This module provides:
//! - `mul_hi_u64`: High 64 bits of 64x64 multiply (equivalent to `__umulh` / `Mul64Hi`)
//! - `compute_reciprocal`: Reciprocal frequency preparation for fast division
//!
//! The arithmetic here must match the Microsoft C++ implementation exactly.

use crate::Freq;

/// Returns the high 64 bits of the 128-bit product of two 64-bit integers.
///
/// Equivalent to `msrtc_rans::details::Mul64Hi(a, b)`.
#[inline]
pub fn mul_hi_u64(a: u64, b: u64) -> u64 {
    // Rust has the native widening_mul on nightly, but we use portable
    // u128 to avoid nightly dependency.
    let prod = a as u128 * b as u128;
    (prod >> 64) as u64
}

/// Compute the reciprocal shift for a given frequency.
///
/// This is `shift = ceil(log2(freq))` from the C++ implementation.
#[inline]
pub fn reciprocal_shift(freq: Freq) -> u32 {
    let mut shift = 1u32;
    while freq > (1u32 << shift) {
        shift += 1;
    }
    shift
}

/// Compute the fixed-point reciprocal frequency for a 64-bit state.
///
/// This implements the Alverson integer division method used in the Microsoft code
/// for the uint64_t state path:
///
/// ```cpp
/// // long divide: ((1 << (shift + bits - 1)) + freq-1) / freq
/// auto x0 = static_cast<state_t>(freq - 1);
/// auto x1 = static_cast<state_t>(1) << (shift + 31);
/// auto t1 = x1 / freq;
/// x0 += (x1 % freq) << 32;
/// auto t0 = x0 / freq;
/// m_freq_rcp = t0 + (t1 << 32);
/// ```
///
/// For the 32-bit state path we use the uint64_t extension method.
#[inline]
pub fn compute_reciprocal_u64(freq: Freq) -> u64 {
    // This matches the uint64_t specialization in the C++ source:
    // auto x0 = static_cast<state_t>(freq - 1);
    // auto x1 = static_cast<state_t>(1) << (shift + 31);
    // auto t1 = x1 / freq;
    // x0 += (x1 % freq) << 32;
    // auto t0 = x0 / freq;
    // m_freq_rcp = t0 + (t1 << 32);
    let x0: u64 = (freq - 1) as u64;
    let shift = reciprocal_shift(freq);
    let x1: u64 = 1u64 << (shift + 31);
    let t1 = x1 / freq as u64;
    let x0_adj = x0 + ((x1 % freq as u64) << 32);
    let t0 = x0_adj / freq as u64;
    t0 + (t1 << 32)
}

/// Compute the fixed-point reciprocal frequency for a 32-bit state.
///
/// For the 32-bit state path:
/// ```cpp
/// constexpr auto bits = sizeof(state_t) * CHAR_BIT;
/// auto nom = (static_cast<uint64_t>(1) << (shift + bits - 1)) + (freq - 1);
/// m_freq_rcp = static_cast<state_t>(nom / freq);
/// ```
#[inline]
pub fn compute_reciprocal_u32(freq: Freq) -> u32 {
    let shift = reciprocal_shift(freq);
    let bits: u32 = 32;
    let nom: u64 = (1u64 << (shift + bits - 1)) + (freq - 1) as u64;
    (nom / freq as u64) as u32
}

/// Compute the frequency-one reciprocal for 64-bit state.
///
/// When freq==1, the reciprocal is `~0u64` and shift is 0.
#[inline]
pub fn freq_one_reciprocal_u64() -> (u64, u32) {
    (!0u64, 0)
}

/// Compute the frequency-one reciprocal for 32-bit state.
#[inline]
pub fn freq_one_reciprocal_u32() -> (u32, u32) {
    (!0u32, 0)
}

/// Perform fast division by frequency using the reciprocal.
///
/// For u64 state: `Mul64Hi(x, freq_rcp) >> rcp_shift`
/// For u32 state: `(u64(x) * freq_rcp) >> rcp_shift`
#[inline]
pub fn fast_quotient_u64(x: u64, freq_rcp: u64, rcp_shift: u32) -> u64 {
    mul_hi_u64(x, freq_rcp) >> rcp_shift
}

#[inline]
pub fn fast_quotient_u32(x: u32, freq_rcp: u32, rcp_shift: u32) -> u32 {
    ((x as u64 * freq_rcp as u64) >> rcp_shift) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mul_hi_u64() {
        // 3 * 5 = 15, high part of small numbers is 0
        assert_eq!(mul_hi_u64(3, 5), 0);
        // !0u64 * 1 = 2^64-1, high part is 0
        assert_eq!(mul_hi_u64(!0u64, 1u64), 0);
        // 2^63 * 2^63 = 2^126, high 64 bits = 2^62
        let a: u64 = 1u64 << 63;
        assert_eq!(mul_hi_u64(a, a), 1u64 << 62);
        // !0u64 * !0u64 = (2^64-1)^2 = 2^128 - 2*2^64 + 1
        // high bits: 2^64 - 2
        assert_eq!(mul_hi_u64(!0u64, !0u64), !0u64 - 1);
    }

    #[test]
    fn test_reciprocal_shift() {
        // C++: shift=1; while (freq > (1<<shift)) shift++;
        assert_eq!(reciprocal_shift(1), 1);
        assert_eq!(reciprocal_shift(2), 1); // 2 > 2? no
        assert_eq!(reciprocal_shift(3), 2); // 3 > 2? yes; 3 > 4? no
        assert_eq!(reciprocal_shift(4), 2); // 4 > 2? yes; 4 > 4? no
        assert_eq!(reciprocal_shift(5), 3); // 5 > 2? yes; 5 > 4? yes; 5 > 8? no
        assert_eq!(reciprocal_shift(8), 3); // 8 > 2? yes; 8 > 4? yes; 8 > 8? no
        assert_eq!(reciprocal_shift(9), 4);
    }

    #[test]
    fn test_compute_reciprocal_u64() {
        let rcp = compute_reciprocal_u64(3);
        assert!(rcp > 0);
        // With freq=3, x=LowerBound (1<<31), quotient should be ~715827882
        let x: u64 = 1u64 << 31;
        let q = fast_quotient_u64(x, rcp, reciprocal_shift(3) - 1);
        // q should be floor(x/freq) = floor(2^31/3) = 715827882
        assert_eq!(q, (1u64 << 31) / 3);
    }

    #[test]
    fn test_fast_division_matches_exact() {
        // Verify that fast division matches exact division for a range of values
        // Note: freq=1 uses a special fast path, not compute_reciprocal_u64
        let freqs = [2u32, 3, 5, 10, 100, 1000, 0x7FFF];
        for freq in freqs {
            let shift = reciprocal_shift(freq) - 1;
            let rcp = compute_reciprocal_u64(freq);

            for x in [1u64, 100, 1u64 << 20, 1u64 << 31, 1u64 << 40, 1u64 << 50] {
                let fast = fast_quotient_u64(x, rcp, shift);
                let exact = x / freq as u64;
                assert_eq!(fast, exact, "freq={} x={}", freq, x);
            }
        }
    }

    #[test]
    fn test_freq_one_special_case_division() {
        // For freq=1: q = mul_hi(x, !0) >> 0 = x - 1 (for x > 0)
        let (rcp, shift) = freq_one_reciprocal_u64();
        assert_eq!(rcp, !0u64);
        assert_eq!(shift, 0);

        let x: u64 = 100;
        let q = fast_quotient_u64(x, rcp, shift);
        assert_eq!(q, x - 1);
    }
}
