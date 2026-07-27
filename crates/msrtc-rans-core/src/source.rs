// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! Source traits for rANS decoder input.
//!
//! A source provides units (bytes or words) to the decoder during
//! initialization and renormalization.

/// Trait for a source that provides rANS units to the decoder.
///
/// Equivalent to the `operator(unit_t&)` call on the C++ source type,
/// simplified to a `bool` return since the decoder uses boolean outcomes.
pub trait Source<Unit>: Sized {
    /// Read the next unit from the source.
    /// Returns `true` if a unit was available, `false` if exhausted.
    fn read(&mut self, unit: &mut Unit) -> bool;

    /// Check whether the source has been fully consumed.
    fn is_exhausted(&self) -> bool;
}

/// A simple slice-based source that reads units from a `&[Unit]`.
///
/// Equivalent to the C++ `span<const unit_t>` based source used by
/// `RansDecoder` and `RansDecoderStreamImpl`.
#[derive(Debug, Clone)]
pub struct SliceSource<'a, Unit> {
    data: &'a [Unit],
    pos: usize,
}

impl<'a, Unit: Copy> SliceSource<'a, Unit> {
    /// Create a new source reading from the given slice.
    pub fn new(data: &'a [Unit]) -> Self {
        Self { data, pos: 0 }
    }

    /// Current read position.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Remaining units in the source.
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
}

impl<'a, Unit: Copy> Source<Unit> for SliceSource<'a, Unit> {
    #[inline]
    fn read(&mut self, unit: &mut Unit) -> bool {
        if self.pos < self.data.len() {
            *unit = self.data[self.pos];
            self.pos += 1;
            true
        } else {
            false
        }
    }

    #[inline]
    fn is_exhausted(&self) -> bool {
        self.pos >= self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slice_source_basic() {
        let data = [1u8, 2, 3, 4];
        let mut source = SliceSource::new(&data);

        let mut unit = 0u8;
        assert!(source.read(&mut unit));
        assert_eq!(unit, 1);
        assert!(source.read(&mut unit));
        assert_eq!(unit, 2);
        assert!(source.read(&mut unit));
        assert_eq!(unit, 3);
        assert!(source.read(&mut unit));
        assert_eq!(unit, 4);

        assert!(!source.read(&mut unit));
        assert!(source.is_exhausted());
    }

    #[test]
    fn test_empty_source() {
        let data: [u8; 0] = [];
        let mut source = SliceSource::new(&data);
        let mut unit = 0u8;
        assert!(!source.read(&mut unit));
        assert!(source.is_exhausted());
    }
}
