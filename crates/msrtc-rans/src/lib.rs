// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com
// Derived from Microsoft MLVC msrtc_rans (MIT)
// See NOTICE file for attribution.

//! # msrtc-rans
//!
//! Safe public Rust entropy-coder API for msrtc_rans.
//!
//! This crate provides the public API for the rANS entropy coder,
//! including entropy encoder/decoder, streams, and distribution
//! construction.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

extern crate alloc;

/// Re-export the core rANS primitives.
pub use msrtc_rans_core::*;

/// Entropy encoder/decoder (high-level PMF/bypass/CDF pipeline).
pub mod entropy;
