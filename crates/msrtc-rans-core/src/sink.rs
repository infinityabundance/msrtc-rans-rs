// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! Sink traits for rANS encoder output.
//!
//! A sink is a write-only consumer of units (bytes or words).
//! The rANS encoder writes units in reverse order (high to low).

use alloc::vec::Vec;

/// Trait for a sink that accepts rANS units.
///
/// Equivalent to the `operator(unit_t)` call on the C++ sink type.
pub trait Sink<Unit>: Sized {
    /// Write a single unit to the sink.
    fn write(&mut self, unit: Unit);

    /// Returns a slice of the encoded data so far.
    fn encoded(&self) -> &[Unit];
}

/// A simple Vec-based sink that stores units in reverse order.
#[derive(Debug)]
pub struct VecSink<Unit> {
    buffer: Vec<Unit>,
    pos: usize,
}

impl<Unit: Default + Copy + Clone> VecSink<Unit> {
    /// Create a new sink with the given initial capacity.
    pub fn new(initial_capacity: usize) -> Self {
        let capacity = initial_capacity.max(64);
        let mut buffer = Vec::with_capacity(capacity);
        buffer.resize(capacity, Unit::default());
        Self {
            pos: capacity,
            buffer,
        }
    }

    /// Create a new sink with an exact initial capacity (no minimum floor).
    /// Used for testing growth at precise boundaries.
    #[doc(hidden)]
    pub fn with_exact_capacity(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        buffer.resize(capacity, Unit::default());
        Self {
            pos: capacity,
            buffer,
        }
    }

    /// Get the encoded output as a slice of units (from write position to end).
    pub fn encoded(&self) -> &[Unit] {
        &self.buffer[self.pos..]
    }

    /// Number of units written.
    pub fn len(&self) -> usize {
        self.buffer.len() - self.pos
    }

    /// Whether no data has been written.
    pub fn is_empty(&self) -> bool {
        self.buffer.len() == self.pos
    }

    /// Reset the sink, discarding all written data.
    pub fn reset(&mut self) {
        self.pos = self.buffer.len();
    }

    /// Ensure there's enough room before `pos` to write one more unit.
    ///
    /// On growth, copies the existing encoded suffix to the end of the new allocation,
    /// matching Microsoft's `ResizableBufferSink::enlargeBuffer()` which copies the
    /// content span into `newBuffer.last(content.size())`.
    fn ensure_space(&mut self) {
        if self.pos == 0 {
            let old_len = self.buffer.len();
            let content_len = old_len - self.pos; // = old_len (since pos==0)
            let new_len = old_len + old_len.max(256);

            // Extend the buffer; new elements are zeroed (Unit::default())
            self.buffer.resize(new_len, Unit::default());

            // Move the existing encoded content to the last `content_len` slots
            let new_pos = new_len - content_len;
            self.buffer.copy_within(0..content_len, new_pos);
            self.pos = new_pos;
        }
    }
}

impl<Unit: Default + Copy + Clone> Sink<Unit> for VecSink<Unit> {
    #[inline]
    fn write(&mut self, unit: Unit) {
        self.ensure_space();
        self.pos -= 1;
        self.buffer[self.pos] = unit;
    }

    #[inline]
    fn encoded(&self) -> &[Unit] {
        &self.buffer[self.pos..]
    }
}

/// A fixed-size slice sink that writes into a pre-allocated buffer from the end.
pub struct SliceSink<'a, Unit> {
    buffer: &'a mut [Unit],
    pos: usize,
}

impl<'a, Unit: Copy> SliceSink<'a, Unit> {
    /// Create a new sink writing into the given buffer from the end.
    pub fn new(buffer: &'a mut [Unit]) -> Self {
        let pos = buffer.len();
        Self { buffer, pos }
    }

    /// Returns the encoded data (the portion that was written).
    pub fn encoded(&self) -> &[Unit] {
        &self.buffer[self.pos..]
    }

    /// Number of units written.
    pub fn len(&self) -> usize {
        self.buffer.len() - self.pos
    }

    /// Whether no data has been written.
    pub fn is_empty(&self) -> bool {
        self.buffer.len() == self.pos
    }
}

impl<'a, Unit: Copy> Sink<Unit> for SliceSink<'a, Unit> {
    #[inline]
    fn write(&mut self, unit: Unit) {
        debug_assert!(self.pos > 0, "SliceSink overflow");
        self.pos -= 1;
        self.buffer[self.pos] = unit;
    }

    #[inline]
    fn encoded(&self) -> &[Unit] {
        &self.buffer[self.pos..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec_sink_basic() {
        let mut sink = VecSink::<u8>::new(64);
        assert!(sink.is_empty());

        sink.write(0xAB);
        sink.write(0xCD);
        assert_eq!(sink.len(), 2);
        // Data written end-to-start: first write goes to end of encoded output
        assert_eq!(sink.encoded(), &[0xCD, 0xAB]);
    }

    #[test]
    fn test_vec_sink_growth_at_64_boundary() {
        // Exact 64 capacity: write 64 items then 65th triggers growth
        let mut sink = VecSink::<u8>::with_exact_capacity(64);
        for i in 0..65u8 {
            sink.write(i);
        }
        assert_eq!(sink.len(), 65);
        // Verify every unit is present and correct (reversed)
        for i in 0..65u8 {
            assert_eq!(
                sink.encoded()[(64 - i) as usize],
                i,
                "mismatch at position {}",
                i
            );
        }
    }

    #[test]
    fn test_vec_sink_growth_at_65_boundary() {
        // Write exactly to first-growth boundary and one past
        let mut sink = VecSink::<u8>::with_exact_capacity(65);
        for i in 0..66u8 {
            sink.write(i);
        }
        assert_eq!(sink.len(), 66);
        for i in 0..66u8 {
            assert_eq!(
                sink.encoded()[(65 - i) as usize],
                i,
                "mismatch at position {}",
                i
            );
        }
    }

    #[test]
    fn test_vec_sink_growth_at_320_boundary() {
        // Growth step is min(old_len, 256) when old_len=320: step=256, new=576
        let mut sink = VecSink::<u8>::with_exact_capacity(320);
        for i in 0..321u16 {
            sink.write(i as u8);
        }
        assert_eq!(sink.len(), 321);
        for i in 0..321u16 {
            assert_eq!(
                sink.encoded()[(320 - i) as usize],
                i as u8,
                "mismatch at {}",
                i
            );
        }
    }

    #[test]
    fn test_vec_sink_multiple_growths() {
        // Force several growth cycles: 64 → 320 → 576 → ...
        let mut sink = VecSink::<u8>::with_exact_capacity(64);
        for i in 0..1000u16 {
            sink.write(i as u8);
        }
        assert_eq!(sink.len(), 1000);
        for i in 0..1000u16 {
            assert_eq!(
                sink.encoded()[(999 - i) as usize],
                i as u8,
                "mismatch at {}",
                i
            );
        }
    }

    #[test]
    fn test_slice_sink_basic() {
        let mut buf = [0u8; 16];
        let mut sink = SliceSink::new(&mut buf[..]);
        sink.write(0xAB);
        sink.write(0xCD);
        assert_eq!(sink.len(), 2);
        assert_eq!(sink.encoded(), &[0xCD, 0xAB]);
    }
}
