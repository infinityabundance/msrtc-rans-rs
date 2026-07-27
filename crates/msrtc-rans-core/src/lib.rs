// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com
// Derived from Microsoft MLVC msrtc_rans (MIT)
// See NOTICE file for attribution.

#![no_std]
#![forbid(unsafe_code)]
#![allow(missing_docs)]
#![allow(clippy::assign_op_pattern)]

//! # msrtc-rans-core
//!
//! Deterministic, no_std-capable rANS primitives for the msrtc_rans entropy coder.
//!
//! This crate implements the raw rANS encoding and decoding primitives used by
//! Microsoft MLVC's `msrtc_rans` package. The arithmetic (reciprocal preparation,
//! Mul64Hi, fast division) is structurally faithful to the C++ implementation.
//!
//! **Status:** The encoder and decoder equations, constants, and symbol preparation
//! are implemented and self-consistent. Byte-level parity against the pinned
//! Microsoft C++ oracle has not yet been established by differential court.
//! See the `msrtc-rans-court` crate for ongoing parity verification.

extern crate alloc;

pub mod arithmetic;
pub mod raw;
pub mod sink;
pub mod source;
pub mod variant;

/// Frequency type used throughout the rANS implementation - corresponds to `rans_freq_t` (uint32_t)
pub type Freq = u32;

/// Re-export key types for convenience.
pub use raw::{
    Rans64DecSymbol, Rans64Decoder, Rans64EncSymbol, Rans64Encoder, RansByteDecSymbol,
    RansByteDecoder, RansByteEncSymbol, RansByteEncoder,
};
pub use variant::RansVariant;
