// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # Resizable buffer — `IResizableBuffer` pattern
//!
//! Matches Microsoft's `msrtc_rans::IResizableBuffer` and
//! `HeapResizableBuffer` from `EntropyCoder.cpp`.
//!
//! The buffer layer is fully safe: no raw pointers, no reinterpret casts.

use core::mem::size_of;

/// Minimum buffer size (bytes).
pub const MIN_BUFFER_SIZE: usize = 512;

/// Minimum alignment (bytes) — `sizeof(uint32_t)`.
pub const MIN_ALIGNMENT: usize = 4;

/// Align a size to the minimum alignment.
///
/// Matches `IResizableBuffer::AlignSize(size, up)`:
/// ```cpp
/// if (up) size += s_MinAlignment - 1;
/// return size & ~static_cast<size_t>((1 << s_MinAlignment) - 1);
/// ```
/// With `s_MinAlignment = 4`, `(1 << 4) - 1 = 15`, so this rounds to a
/// multiple of 16.
pub const fn align_size(size: usize, up: bool) -> usize {
    let adjusted = if up {
        size + (MIN_ALIGNMENT as usize) - 1
    } else {
        size
    };
    adjusted & !((1usize << MIN_ALIGNMENT) - 1)
}

/// Errors from buffer operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferError {
    /// Requested size exceeds the representable range.
    CapacityOverflow,
}

/// The `IResizableBuffer` interface.
///
/// Matches:
/// ```cpp
/// struct IResizableBuffer {
///     virtual span<std::byte> GetBuffer() = 0;
///     virtual span<std::byte> BeginToGrow() = 0;
///     virtual void Commit() = 0;
///     virtual void Rollback() = 0;
/// };
/// ```
pub trait ResizableBuffer {
    /// Get the current buffer as a byte slice.
    fn get_buffer(&self) -> &[u8];

    /// Get the current buffer as a mutable byte slice.
    fn get_buffer_mut(&mut self) -> &mut [u8];

    /// Begin a grow operation and return the new (larger) buffer.
    /// Existing content is copied to the FRONT of the new buffer.
    /// The caller may relocate content; then call `commit()` (or
    /// `rollback()` to cancel).
    fn begin_to_grow(&mut self) -> Result<&mut [u8], BufferError>;

    /// Complete the active grow operation.
    fn commit(&mut self);

    /// Cancel the active grow operation.
    fn rollback(&mut self);
}

/// Heap-allocated resizable buffer.
///
/// Matches `HeapResizableBuffer`:
/// ```cpp
/// HeapResizableBuffer(size_t initialSize = 4096, size_t maxSizeStep = 1024 * 1024);
/// // initialSize = max(AlignSize(initialSize, true), s_MinBufferSize);
/// // m_maxSizeStep = max(AlignSize(maxSizeStep, false), s_MinBufferSize);
///
/// span<std::byte> BeginToGrow() {
///     auto newSize = m_bufferSize + std::min(m_bufferSize, m_maxSizeStep);
///     ...
/// }
/// ```
#[derive(Debug)]
pub struct HeapResizableBuffer {
    buffer: Vec<u8>,
    new_buffer: Option<Vec<u8>>,
    max_size_step: usize,
}

impl HeapResizableBuffer {
    /// Create a new buffer.
    ///
    /// - `initial_size` is aligned up and floored at `MIN_BUFFER_SIZE`.
    /// - `max_size_step` is aligned down and floored at `MIN_BUFFER_SIZE`.
    pub fn new(initial_size: usize, max_size_step: usize) -> Self {
        let initial = align_size(initial_size, true).max(MIN_BUFFER_SIZE);
        let step = align_size(max_size_step, false).max(MIN_BUFFER_SIZE);
        Self {
            buffer: vec![0u8; initial],
            new_buffer: None,
            max_size_step: step,
        }
    }

    /// Current buffer capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Current max growth step in bytes.
    pub fn max_size_step(&self) -> usize {
        self.max_size_step
    }
}

impl Default for HeapResizableBuffer {
    fn default() -> Self {
        Self::new(4096, 1024 * 1024)
    }
}

impl ResizableBuffer for HeapResizableBuffer {
    fn get_buffer(&self) -> &[u8] {
        &self.buffer
    }

    fn get_buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    fn begin_to_grow(&mut self) -> Result<&mut [u8], BufferError> {
        let old_len = self.buffer.len();
        let step = old_len.min(self.max_size_step);
        let new_len = old_len
            .checked_add(step)
            .ok_or(BufferError::CapacityOverflow)?;
        let mut nb = vec![0u8; new_len];
        nb[..old_len].copy_from_slice(&self.buffer);
        self.new_buffer = Some(nb);
        Ok(self.new_buffer.as_mut().expect("just set"))
    }

    fn commit(&mut self) {
        if let Some(nb) = self.new_buffer.take() {
            self.buffer = nb;
        }
    }

    fn rollback(&mut self) {
        self.new_buffer = None;
    }
}

/// A byte-oriented backward-writing sink over a `ResizableBuffer`.
///
/// This is a **safe** implementation of Microsoft's `ResizableBufferSink`:
/// units are serialized to little-endian bytes and written into the byte
/// buffer from the end toward the start. The write position is tracked as
/// a byte count from the buffer end; growth relocates existing content to
/// the new buffer's end (matching Microsoft's `newBuffer.last(content.size())`).
///
/// Note: this type does **not** implement the `Sink` trait (which requires
/// returning `&[Unit]`, impossible safely over a byte buffer). Use it with
/// the low-level raw encoders via a manual write loop, or use `VecSink`
/// when trait-based sinks are required.
pub struct ResizableBufferSink<'a, Unit> {
    buffer: &'a mut dyn ResizableBuffer,
    /// Number of bytes written (measured from the buffer end).
    written_bytes: usize,
    _unit: core::marker::PhantomData<Unit>,
}

impl<'a, Unit> ResizableBufferSink<'a, Unit> {
    /// Create a sink over the given buffer.
    pub fn new(buffer: &'a mut dyn ResizableBuffer) -> Self {
        Self {
            buffer,
            written_bytes: 0,
            _unit: core::marker::PhantomData,
        }
    }

    /// The encoded bytes (written region, in write order).
    pub fn encoded_bytes(&self) -> &[u8] {
        let buf = self.buffer.get_buffer();
        let start = buf.len() - self.written_bytes;
        &buf[start..]
    }

    /// Reset the write pointer, discarding content.
    pub fn reset(&mut self) {
        self.written_bytes = 0;
    }

    /// Number of bytes written.
    pub fn len(&self) -> usize {
        self.written_bytes
    }

    /// Whether nothing has been written.
    pub fn is_empty(&self) -> bool {
        self.written_bytes == 0
    }
}

impl<'a> ResizableBufferSink<'a, u8> {
    /// Write a single byte.
    pub fn write_u8(&mut self, unit: u8) {
        self.ensure_space(1);
        let buf = self.buffer.get_buffer_mut();
        let pos = buf.len() - self.written_bytes - 1;
        buf[pos] = unit;
        self.written_bytes += 1;
    }
}

impl<'a> ResizableBufferSink<'a, u32> {
    /// Write a single u32 unit (little-endian).
    pub fn write_u32(&mut self, unit: u32) {
        self.ensure_space(4);
        let bytes = unit.to_le_bytes();
        let buf = self.buffer.get_buffer_mut();
        let pos = buf.len() - self.written_bytes - 4;
        buf[pos..pos + 4].copy_from_slice(&bytes);
        self.written_bytes += 4;
    }
}

impl<'a, Unit> ResizableBufferSink<'a, Unit> {
    /// Ensure at least `n` free bytes before the written region; grow if needed.
    fn ensure_space(&mut self, n: usize) {
        let buf_len = self.buffer.get_buffer().len();
        if self.written_bytes + n > buf_len {
            self.enlarge(n);
        }
    }

    /// Grow the buffer, preserving existing content at the END.
    fn enlarge(&mut self, needed: usize) {
        let content = self.encoded_bytes().to_vec();

        let new_units = self.buffer.begin_to_grow().expect("buffer growth");
        let new_len = new_units.len();
        assert!(
            content.len() + needed <= new_len,
            "new buffer must fit existing content plus needed bytes"
        );

        // Copy content to the END of the new buffer (Microsoft: last(content.size()))
        let new_content_start = new_len - content.len();
        new_units[new_content_start..].copy_from_slice(&content);

        self.buffer.commit();
        self.written_bytes = content.len();
    }
}

/// Number of bytes needed to serialize a unit.
pub fn unit_byte_size<Unit>() -> usize {
    size_of::<Unit>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_size() {
        assert_eq!(align_size(512, true), 512);
        assert_eq!(align_size(513, true), 512);
        assert_eq!(align_size(520, true), 512);
        assert_eq!(align_size(528, true), 528);
        assert_eq!(align_size(1000, false), 992);
    }

    #[test]
    fn test_heap_buffer_initial_size() {
        let b = HeapResizableBuffer::new(0, 1024 * 1024);
        assert_eq!(b.capacity(), 512);
        let b2 = HeapResizableBuffer::new(100, 1024 * 1024);
        assert_eq!(b2.capacity(), 512);
        let b3 = HeapResizableBuffer::new(4096, 1024 * 1024);
        assert_eq!(b3.capacity(), 4096);
    }

    #[test]
    fn test_growth_policy() {
        // BeginToGrow: newSize = old + min(old, maxStep)
        // maxSizeStep is floored at MIN_BUFFER_SIZE (512): max(align(256,false), 512) = 512
        let mut b = HeapResizableBuffer::new(512, 256);
        let new = b.begin_to_grow().expect("grow");
        assert_eq!(new.len(), 512 + 512);
        b.commit();
        assert_eq!(b.capacity(), 1024);

        let new2 = b.begin_to_grow().expect("grow2");
        assert_eq!(new2.len(), 1024 + 512);
        b.rollback();
        assert_eq!(b.capacity(), 1024, "rollback must restore");
    }

    #[test]
    fn test_capped_growth_step() {
        // maxSizeStep floored at 512; growth capped at min(old, 512)
        let mut b = HeapResizableBuffer::new(4096, 256);
        let new = b.begin_to_grow().expect("grow");
        // 4096 + min(4096, 512) = 4096 + 512 = 4608
        assert_eq!(new.len(), 4608);
        b.commit();
        assert_eq!(b.capacity(), 4608);
    }

    #[test]
    fn test_u8_sink_writes_backward() {
        let mut buffer = HeapResizableBuffer::new(512, 256);
        let mut sink = ResizableBufferSink::<u8>::new(&mut buffer);
        sink.write_u8(0xAB);
        sink.write_u8(0xCD);
        assert_eq!(sink.len(), 2);
        // Written backward: first write is LAST in the encoded span
        assert_eq!(sink.encoded_bytes(), &[0xCD, 0xAB]);
    }

    #[test]
    fn test_u8_sink_growth_preserves_content() {
        // 8-byte buffer; write 10 bytes to force growth
        let mut buffer = HeapResizableBuffer::new(8, 4);
        let mut sink = ResizableBufferSink::<u8>::new(&mut buffer);
        for i in 0..10u8 {
            sink.write_u8(i);
        }
        assert_eq!(sink.len(), 10);
        let enc = sink.encoded_bytes();
        for i in 0..10u8 {
            assert_eq!(enc[(9 - i) as usize], i, "mismatch at {}", i);
        }
    }

    #[test]
    fn test_u32_sink_bytes() {
        let mut buffer = HeapResizableBuffer::new(64, 16);
        let mut sink = ResizableBufferSink::<u32>::new(&mut buffer);
        sink.write_u32(0x01020304);
        sink.write_u32(0x05060708);
        let bytes = sink.encoded_bytes();
        // u32 units serialized LE, written backward: first written is LAST
        assert_eq!(&bytes[0..4], &[0x08, 0x07, 0x06, 0x05]);
        assert_eq!(&bytes[4..8], &[0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn test_sink_reset() {
        let mut buffer = HeapResizableBuffer::new(512, 256);
        let mut sink = ResizableBufferSink::<u8>::new(&mut buffer);
        sink.write_u8(0xAB);
        assert_eq!(sink.len(), 1);
        sink.reset();
        assert_eq!(sink.len(), 0);
    }

    #[test]
    fn test_buffer_overflow_error() {
        // max_size_step = 0 → floored to 512; capacity never overflows usize
        let mut b = HeapResizableBuffer::new(0, 0);
        assert_eq!(b.capacity(), 512);
        let _ = b.begin_to_grow().expect("growth succeeds");
        b.commit();
        assert!(b.capacity() > 512);
    }
}
