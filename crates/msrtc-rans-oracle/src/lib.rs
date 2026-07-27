// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # msrtc-rans-oracle
//!
//! Developer-only Microsoft C++ oracle adapter for differential testing.
//!
//! This crate provides a test-only interface to the pinned Microsoft C++
//! rANS implementation. It is never linked, bundled, or required by the
//! production Rust or Python package.
//!
//! ## Safety
//!
//! This crate uses `unsafe` to call the C++ oracle via FFI. This is
//! acceptable because:
//! - It is a test-only dependency
//! - The oracle interface is narrow and well-defined
//! - All interactions are captured as deterministic casefiles

#![allow(missing_docs)]

/// Placeholder for oracle FFI bindings.
/// The actual oracle integration is built inside Docker and
/// communicates via casefiles, not direct linking.
pub mod oracle {
    /// Oracle variant selection
    pub enum OracleVariant {
        RansByte,
        Rans64,
    }

    /// Oracle result
    pub struct OracleResult {
        pub status: Result<(), String>,
        pub output: Vec<u8>,
    }
}
