// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! Error types for msrtc-rans-core.

use core::fmt;

/// Errors that can occur during raw rANS operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawRansError {
    /// `scale_bits` is out of the valid range for the variant.
    ///
    /// For RansByte: valid range is `[2, 30]`.
    /// For Rans64: the defined safe range is `[2, 31]`. `scale_bits == 32` causes
    /// undefined-shift behavior in the C++ oracle and is rejected by this implementation.
    InvalidScaleBits {
        /// The provided scale_bits value.
        provided: u32,
        /// The maximum safe value for this variant.
        max_safe: u32,
    },
    /// Frequency or start values are out of range.
    InvalidParameters,
}

impl fmt::Display for RawRansError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RawRansError::InvalidScaleBits { provided, max_safe } => {
                write!(
                    f,
                    "invalid scale_bits {}: must be in [2, {}]",
                    provided, max_safe
                )
            }
            RawRansError::InvalidParameters => {
                write!(f, "invalid rANS parameters")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RawRansError {}
