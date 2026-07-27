// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! RANS variant definitions and macros.
//!
//! Defines the two supported rANS variants via a macro:
//! - `RansByte`: 32-bit state, 8-bit units
//! - `Rans64`: 64-bit state, 32-bit units

/// RANS variant enum for runtime dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RansVariant {
    /// 32-bit state with 8-bit units
    RansByte,
    /// 64-bit state with 32-bit units
    Rans64,
}

impl RansVariant {
    /// Returns the size of the state type in bytes.
    pub const fn state_size(&self) -> usize {
        match self {
            RansVariant::RansByte => 4,
            RansVariant::Rans64 => 8,
        }
    }

    /// Returns the size of the unit type in bytes.
    pub const fn unit_size(&self) -> usize {
        match self {
            RansVariant::RansByte => 1,
            RansVariant::Rans64 => 4,
        }
    }
}

/// Marker trait for rANS type-level parameters.
pub trait RansParams {
    /// The state type (u32 or u64).
    type State: Copy + Clone + Default + core::fmt::Debug + PartialEq + Eq;
    /// The unit type (u8 or u32).
    type Unit: Copy + Clone + Default + core::fmt::Debug + PartialEq + Eq;
    /// Human-readable name.
    const NAME: &'static str;
    /// STATE_BITS
    const STATE_BITS: u32;
    /// MAX_SCALE_BITS
    const MAX_SCALE_BITS: u32;
    /// LOWER_BOUND
    const LOWER_BOUND: Self::State;
    /// UNIT_BITS
    const UNIT_BITS: u32;
    /// UNITS_PER_STATE
    const UNITS_PER_STATE: usize;
}

/// RansByte variant: u32 state, u8 unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RansByte;

impl RansParams for RansByte {
    type State = u32;
    type Unit = u8;
    const NAME: &'static str = "RansByte";
    const STATE_BITS: u32 = 31;
    const MAX_SCALE_BITS: u32 = 30;
    const LOWER_BOUND: u32 = 1u32 << 23;
    const UNIT_BITS: u32 = 8;
    const UNITS_PER_STATE: usize = 4;
}

/// Rans64 variant: u64 state, u32 unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rans64;

impl RansParams for Rans64 {
    type State = u64;
    type Unit = u32;
    const NAME: &'static str = "Rans64";
    const STATE_BITS: u32 = 63;
    const MAX_SCALE_BITS: u32 = 32;
    const LOWER_BOUND: u64 = 1u64 << 31;
    const UNIT_BITS: u32 = 32;
    const UNITS_PER_STATE: usize = 2;
}
