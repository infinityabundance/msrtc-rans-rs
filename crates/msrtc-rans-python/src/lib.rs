// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # msrtc-rans-python
//!
//! Python extension module `_msrtc_rans` providing the msrtc.rans Python API.
//!
//! This crate uses PyO3 to create a CPython extension module that is
//! import-path-compatible with the existing `msrtc.rans` package.
//!
//! Stream classes wrap the persistent Rust stream types from
//! `msrtc_rans::stream`, matching Microsoft's `RansEncoderStream` /
//! `RansDecoderStream` semantics:
//!
//! - `RansEncoderStream` keeps a single persistent raw rANS encoder state
//!   across `push()` calls and flushes it once (`Flush(abort=false)`),
//!   exactly like Microsoft's `RansEncoderStreamImpl`.
//! - `RansDecoderStream` owns the encoded message and keeps a persistent
//!   decode cursor (unit position + rANS state) across `decode()` calls,
//!   matching Microsoft's `RansDecoderStreamImpl`.

#![allow(missing_docs)]
#![allow(unsafe_op_in_unsafe_fn)]

use pyo3::exceptions::PyValueError;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyByteArray;
use pyo3::types::PyBytes;

use msrtc_rans::entropy::{EntropyDecoder, EntropyEncoder};
use msrtc_rans::stream::RansDecoderStream as CoreDecoderStream;
use msrtc_rans::stream::RansEncoderStream as CoreEncoderStream;
use msrtc_rans::variant::Rans64;
use msrtc_rans::variant::RansByte;

// ---------------------------------------------------------------------------
// FFI-based buffer helpers using PyObject_GetBuffer (part of Python 3.11+ API)
//
// PyBUF flags (numeric for safety across PyO3 versions):
//   READ  = PyBUF_ND | PyBUF_FORMAT = 8 | 4 = 12  (PyBUF_CONTIG)
//   WRITE = READ | PyBUF_WRITABLE  = 12 | 1 = 13
// ---------------------------------------------------------------------------

const BUF_READ: i32 = 12; // PyBUF_ND | PyBUF_FORMAT = PyBUF_CONTIG
const BUF_WRITE: i32 = 13; // PyBUF_CONTIG | PyBUF_WRITABLE

unsafe fn buffer_to_i32_slice(buf: &ffi::Py_buffer) -> &[i32] {
    let n = (buf.len as usize) / 4;
    std::slice::from_raw_parts(buf.buf as *const i32, n)
}

unsafe fn buffer_to_u8_slice(buf: &ffi::Py_buffer) -> &[u8] {
    let n = buf.len as usize;
    std::slice::from_raw_parts(buf.buf as *const u8, n)
}

fn get_i32_buffer(obj: &Bound<'_, PyAny>) -> PyResult<Vec<i32>> {
    let mut buf: ffi::Py_buffer = unsafe { std::mem::zeroed() };
    let ret = unsafe { ffi::PyObject_GetBuffer(obj.as_ptr(), &mut buf, BUF_READ) };
    if ret != 0 {
        return Err(PyValueError::new_err("cannot get i32 buffer from object"));
    }
    let vec = unsafe { buffer_to_i32_slice(&buf).to_vec() };
    unsafe { ffi::PyBuffer_Release(&mut buf) };
    Ok(vec)
}

fn get_u8_buffer(obj: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if let Ok(bytes) = obj.downcast::<PyBytes>() {
        return Ok(bytes.as_bytes().to_vec());
    }
    if let Ok(ba) = obj.downcast::<PyByteArray>() {
        let slice = unsafe { ba.as_bytes() };
        return Ok(slice.to_vec());
    }
    let mut buf: ffi::Py_buffer = unsafe { std::mem::zeroed() };
    let ret = unsafe { ffi::PyObject_GetBuffer(obj.as_ptr(), &mut buf, BUF_READ) };
    if ret != 0 {
        return Err(PyValueError::new_err("cannot get buffer from object"));
    }
    let vec = unsafe { buffer_to_u8_slice(&buf).to_vec() };
    unsafe { ffi::PyBuffer_Release(&mut buf) };
    Ok(vec)
}

fn write_i32_buffer(obj: &Bound<'_, PyAny>, data: &[i32]) -> PyResult<()> {
    let mut buf: ffi::Py_buffer = unsafe { std::mem::zeroed() };
    let ret = unsafe { ffi::PyObject_GetBuffer(obj.as_ptr(), &mut buf, BUF_WRITE) };
    if ret != 0 {
        return Err(PyValueError::new_err(
            "cannot get writable i32 buffer from object",
        ));
    }
    let n = data.len() * 4;
    let copy_len = n.min(buf.len as usize);
    unsafe {
        let dst = buf.buf as *mut u8;
        let src = data.as_ptr() as *const u8;
        std::ptr::copy_nonoverlapping(src, dst, copy_len);
        ffi::PyBuffer_Release(&mut buf);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Module-level constants
// ---------------------------------------------------------------------------

#[pyfunction]
fn rans_byte() -> i32 {
    1
}

#[pyfunction]
fn rans_64() -> i32 {
    0
}

// ---------------------------------------------------------------------------
// RansEncoderStream
// ---------------------------------------------------------------------------

/// Persistent raw encoder held by the Python `RansEncoderStream`.
///
/// Matches Microsoft's `RawRansEncoderStream`: one raw rANS encoder state
/// persists across `push()` calls; `flush()` finalizes it once.
enum PyEncoderStream {
    None,
    Byte(CoreEncoderStream<RansByte>),
    S64(CoreEncoderStream<Rans64>),
}

#[pyclass(name = "RansEncoderStream")]
struct RansEncoderStream {
    stream: PyEncoderStream,
    #[allow(dead_code)]
    variant: i32,
    #[allow(dead_code)]
    _initial_size: usize,
    #[allow(dead_code)]
    _max_size_step: usize,
}

impl RansEncoderStream {
    /// Push a batch encoded with a RansByte entropy encoder.
    fn push_byte(
        &mut self,
        encoder: &EntropyEncoder<RansByte>,
        indices: &[i32],
        values: &[i32],
    ) -> PyResult<()> {
        match &mut self.stream {
            PyEncoderStream::Byte(s) => s
                .push(encoder, indices, values)
                .map_err(|e| PyValueError::new_err(format!("encode failed: {}", e))),
            PyEncoderStream::S64(_) => Err(PyValueError::new_err(
                "encoder stream variant mismatch: stream is Rans64, encoder is RansByte",
            )),
            PyEncoderStream::None => Err(PyValueError::new_err("invalid state")),
        }
    }

    /// Push a batch encoded with a Rans64 entropy encoder.
    fn push_64(
        &mut self,
        encoder: &EntropyEncoder<Rans64>,
        indices: &[i32],
        values: &[i32],
    ) -> PyResult<()> {
        match &mut self.stream {
            PyEncoderStream::S64(s) => s
                .push(encoder, indices, values)
                .map_err(|e| PyValueError::new_err(format!("encode failed: {}", e))),
            PyEncoderStream::Byte(_) => Err(PyValueError::new_err(
                "encoder stream variant mismatch: stream is RansByte, encoder is Rans64",
            )),
            PyEncoderStream::None => Err(PyValueError::new_err("invalid state")),
        }
    }
}

#[pymethods]
impl RansEncoderStream {
    #[new]
    #[pyo3(signature = (variant=1, *, initialSize=4096, maxSizeStep=1048576))]
    fn new(variant: i32, initialSize: usize, maxSizeStep: usize) -> PyResult<Self> {
        let stream = match variant {
            1 => PyEncoderStream::Byte(CoreEncoderStream::new()),
            0 => PyEncoderStream::S64(CoreEncoderStream::new()),
            _ => {
                return Err(PyValueError::new_err(format!(
                    "unknown rANS variant value: {}",
                    variant
                )));
            }
        };
        Ok(Self {
            stream,
            variant,
            _initial_size: initialSize,
            _max_size_step: maxSizeStep,
        })
    }

    fn flush(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data: Vec<u8> = match &mut self.stream {
            PyEncoderStream::Byte(s) => s
                .flush()
                .map_err(|e| PyValueError::new_err(format!("flush failed: {}", e)))?,
            PyEncoderStream::S64(s) => s
                .flush()
                .map_err(|e| PyValueError::new_err(format!("flush failed: {}", e)))?,
            PyEncoderStream::None => {
                return Err(PyValueError::new_err(
                    "invalid state: stream not initialized",
                ));
            }
        };

        if data.is_empty() {
            return Err(PyValueError::new_err("invalid state: empty output"));
        }

        let ptr = unsafe {
            ffi::PyBytes_FromStringAndSize(
                data.as_ptr() as *const ffi::Py_ssize_t as *const i8,
                data.len() as ffi::Py_ssize_t,
            )
        };
        if ptr.is_null() {
            return Err(PyValueError::new_err("failed to create PyBytes"));
        }
        let obj: Py<PyAny> = unsafe { Bound::from_owned_ptr(py, ptr).unbind() };
        Ok(obj)
    }

    fn reset(&mut self) {
        match &mut self.stream {
            PyEncoderStream::Byte(s) => s.reset(),
            PyEncoderStream::S64(s) => s.reset(),
            PyEncoderStream::None => {}
        }
    }
}

// ---------------------------------------------------------------------------
// RansDecoderStream
// ---------------------------------------------------------------------------

/// Persistent decoder held by the Python `RansDecoderStream`.
///
/// Matches Microsoft's `RansDecoderStreamImpl`: the raw decoder is
/// initialized on the first `decode()` and its cursor persists across
/// subsequent calls.
enum PyDecoderStream {
    None,
    Byte(CoreDecoderStream<RansByte>),
    S64(CoreDecoderStream<Rans64>),
}

#[pyclass(name = "RansDecoderStream")]
struct RansDecoderStream {
    stream: PyDecoderStream,
    #[allow(dead_code)]
    variant: i32,
}

impl RansDecoderStream {
    fn byte_stream_mut(&mut self) -> PyResult<&mut CoreDecoderStream<RansByte>> {
        match &mut self.stream {
            PyDecoderStream::Byte(s) => Ok(s),
            PyDecoderStream::S64(_) => Err(PyValueError::new_err(
                "decoder stream variant mismatch: stream is Rans64",
            )),
            PyDecoderStream::None => Err(PyValueError::new_err("decoder stream is not open")),
        }
    }

    fn s64_stream_mut(&mut self) -> PyResult<&mut CoreDecoderStream<Rans64>> {
        match &mut self.stream {
            PyDecoderStream::S64(s) => Ok(s),
            PyDecoderStream::Byte(_) => Err(PyValueError::new_err(
                "decoder stream variant mismatch: stream is RansByte",
            )),
            PyDecoderStream::None => Err(PyValueError::new_err("decoder stream is not open")),
        }
    }
}

#[pymethods]
impl RansDecoderStream {
    #[new]
    #[pyo3(signature = (data=None, *, variant=1))]
    fn new(data: Option<Bound<'_, PyAny>>, variant: i32) -> PyResult<Self> {
        let stream = match variant {
            1 => match data {
                Some(ref obj) => {
                    PyDecoderStream::Byte(CoreDecoderStream::open_on(&get_u8_buffer(obj)?))
                }
                None => PyDecoderStream::Byte(CoreDecoderStream::new()),
            },
            0 => match data {
                Some(ref obj) => {
                    PyDecoderStream::S64(CoreDecoderStream::open_on(&get_u8_buffer(obj)?))
                }
                None => PyDecoderStream::S64(CoreDecoderStream::new()),
            },
            _ => {
                return Err(PyValueError::new_err(format!(
                    "unknown rANS variant value: {}",
                    variant
                )));
            }
        };
        Ok(Self { stream, variant })
    }

    fn open(&mut self, data: Bound<'_, PyAny>) -> PyResult<()> {
        let bytes = get_u8_buffer(&data)?;
        match self.variant {
            1 => {
                self.stream = PyDecoderStream::Byte(CoreDecoderStream::open_on(&bytes));
            }
            0 => {
                self.stream = PyDecoderStream::S64(CoreDecoderStream::open_on(&bytes));
            }
            _ => return Err(PyValueError::new_err("unknown rANS variant value")),
        }
        Ok(())
    }

    fn close(&mut self) {
        self.stream = PyDecoderStream::None;
    }

    #[pyo3(name = "isOpen")]
    fn is_open(&self) -> bool {
        !matches!(self.stream, PyDecoderStream::None)
    }

    #[pyo3(name = "decodeEOF")]
    fn decode_eof(&mut self) -> PyResult<()> {
        let result = match &mut self.stream {
            PyDecoderStream::Byte(s) => s.decode_eof(),
            PyDecoderStream::S64(s) => s.decode_eof(),
            PyDecoderStream::None => {
                return Err(PyValueError::new_err("decoder stream is not open"));
            }
        };
        result.map_err(|e| PyValueError::new_err(format!("decodeEOF failed: {}", e)))?;
        self.stream = PyDecoderStream::None;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// EntropyEncoder
// ---------------------------------------------------------------------------

#[pyclass(name = "EntropyEncoder")]
struct PyEntropyEncoder {
    byte_encoder: Option<EntropyEncoder<RansByte>>,
    _64_encoder: Option<EntropyEncoder<Rans64>>,
    variant: i32,
}

#[pymethods]
impl PyEntropyEncoder {
    #[new]
    #[pyo3(signature = (*, pmfLengths, pmfOffsets, pmfTable, variant=1, symbolBits=16, bypassBits=4))]
    fn new(
        pmfLengths: Bound<'_, PyAny>,
        pmfOffsets: Bound<'_, PyAny>,
        pmfTable: Bound<'_, PyAny>,
        variant: i32,
        symbolBits: u32,
        bypassBits: u32,
    ) -> PyResult<Self> {
        let lengths = get_i32_buffer(&pmfLengths)?;
        let offsets = get_i32_buffer(&pmfOffsets)?;
        let table = get_i32_buffer(&pmfTable)?;

        match variant {
            1 => {
                let mut encoder = EntropyEncoder::<RansByte>::new();
                encoder
                    .initialize(&lengths, &offsets, &table, symbolBits, bypassBits)
                    .map_err(|e| PyValueError::new_err(format!("encoder init failed: {}", e)))?;
                Ok(Self {
                    byte_encoder: Some(encoder),
                    _64_encoder: None,
                    variant,
                })
            }
            0 => {
                let mut encoder = EntropyEncoder::<Rans64>::new();
                encoder
                    .initialize(&lengths, &offsets, &table, symbolBits, bypassBits)
                    .map_err(|e| PyValueError::new_err(format!("encoder init failed: {}", e)))?;
                Ok(Self {
                    byte_encoder: None,
                    _64_encoder: Some(encoder),
                    variant,
                })
            }
            _ => Err(PyValueError::new_err(format!(
                "invalid variant: {}",
                variant
            ))),
        }
    }

    #[pyo3(signature = (stream, indices, values))]
    fn encode(
        &self,
        stream: &mut RansEncoderStream,
        indices: Bound<'_, PyAny>,
        values: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let indices_vec = get_i32_buffer(&indices)?;
        let values_vec = get_i32_buffer(&values)?;

        if indices_vec.len() != values_vec.len() {
            return Err(PyValueError::new_err(
                "indices and values must have the same length",
            ));
        }

        match self.variant {
            1 => {
                if let Some(ref encoder) = self.byte_encoder {
                    stream.push_byte(encoder, &indices_vec, &values_vec)
                } else {
                    Err(PyValueError::new_err("byte encoder not initialized"))
                }
            }
            0 => {
                if let Some(ref encoder) = self._64_encoder {
                    stream.push_64(encoder, &indices_vec, &values_vec)
                } else {
                    Err(PyValueError::new_err("64 encoder not initialized"))
                }
            }
            _ => Err(PyValueError::new_err("invalid variant")),
        }
    }
}

// ---------------------------------------------------------------------------
// EntropyDecoder
// ---------------------------------------------------------------------------

#[pyclass(name = "EntropyDecoder")]
struct PyEntropyDecoder {
    byte_decoder: Option<EntropyDecoder<RansByte>>,
    _64_decoder: Option<EntropyDecoder<Rans64>>,
    variant: i32,
}

#[pymethods]
impl PyEntropyDecoder {
    #[new]
    #[pyo3(signature = (*, pmfLengths, pmfOffsets, pmfTable, variant=1, symbolBits=16, bypassBits=4))]
    fn new(
        pmfLengths: Bound<'_, PyAny>,
        pmfOffsets: Bound<'_, PyAny>,
        pmfTable: Bound<'_, PyAny>,
        variant: i32,
        symbolBits: u32,
        bypassBits: u32,
    ) -> PyResult<Self> {
        let lengths = get_i32_buffer(&pmfLengths)?;
        let offsets = get_i32_buffer(&pmfOffsets)?;
        let table = get_i32_buffer(&pmfTable)?;

        match variant {
            1 => {
                let mut decoder = EntropyDecoder::<RansByte>::new();
                decoder
                    .initialize(&lengths, &offsets, &table, symbolBits, bypassBits)
                    .map_err(|e| PyValueError::new_err(format!("decoder init failed: {}", e)))?;
                Ok(Self {
                    byte_decoder: Some(decoder),
                    _64_decoder: None,
                    variant,
                })
            }
            0 => {
                let mut decoder = EntropyDecoder::<Rans64>::new();
                decoder
                    .initialize(&lengths, &offsets, &table, symbolBits, bypassBits)
                    .map_err(|e| PyValueError::new_err(format!("decoder init failed: {}", e)))?;
                Ok(Self {
                    byte_decoder: None,
                    _64_decoder: Some(decoder),
                    variant,
                })
            }
            _ => Err(PyValueError::new_err(format!(
                "invalid variant: {}",
                variant
            ))),
        }
    }

    #[pyo3(signature = (values, indices, data))]
    fn decode(
        &self,
        py: Python<'_>,
        values: Bound<'_, PyAny>,
        indices: Bound<'_, PyAny>,
        data: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let indices_vec = get_i32_buffer(&indices)?;
        let num_values = indices_vec.len();

        // Try stream-based decode
        if let Ok(py_stream) = data.extract::<Py<RansDecoderStream>>() {
            let mut stream_ref = py_stream.borrow_mut(py);
            let mut decoded = vec![0i32; num_values];

            match self.variant {
                1 => {
                    if let Some(ref decoder) = self.byte_decoder {
                        let core = stream_ref.byte_stream_mut()?;
                        core.decode(decoder, &mut decoded, &indices_vec)
                            .map_err(|e| PyValueError::new_err(format!("decode failed: {}", e)))?;
                    } else {
                        return Err(PyValueError::new_err("byte decoder not initialized"));
                    }
                }
                0 => {
                    if let Some(ref decoder) = self._64_decoder {
                        let core = stream_ref.s64_stream_mut()?;
                        core.decode(decoder, &mut decoded, &indices_vec)
                            .map_err(|e| PyValueError::new_err(format!("decode failed: {}", e)))?;
                    } else {
                        return Err(PyValueError::new_err("64 decoder not initialized"));
                    }
                }
                _ => return Err(PyValueError::new_err("invalid variant")),
            }

            drop(stream_ref);
            write_i32_buffer(&values, &decoded)?;
            return Ok(());
        }

        // Buffer-based decode
        let data_vec = get_u8_buffer(&data)?;
        let mut decoded = vec![0i32; num_values];

        match self.variant {
            1 => {
                if let Some(ref decoder) = self.byte_decoder {
                    decoder
                        .decode(&mut decoded, &indices_vec, &data_vec)
                        .map_err(|e| PyValueError::new_err(format!("decode failed: {}", e)))?;
                } else {
                    return Err(PyValueError::new_err("byte decoder not initialized"));
                }
            }
            0 => {
                if let Some(ref decoder) = self._64_decoder {
                    decoder
                        .decode(&mut decoded, &indices_vec, &data_vec)
                        .map_err(|e| PyValueError::new_err(format!("decode failed: {}", e)))?;
                } else {
                    return Err(PyValueError::new_err("64 decoder not initialized"));
                }
            }
            _ => return Err(PyValueError::new_err("invalid variant")),
        }

        write_i32_buffer(&values, &decoded)
    }
}

// ---------------------------------------------------------------------------
// Module definition
// ---------------------------------------------------------------------------

#[pymodule]
fn _msrtc_rans(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    // Module-level constants matching C++ enum names used by types.py
    m.add("RansByte", 1)?; // RansVariant.RansByte = 1
    m.add("Rans64", 0)?; // RansVariant.Rans64 = 0
    // Also add as functions for alternative access
    m.add_function(wrap_pyfunction!(rans_byte, m)?)?;
    m.add_function(wrap_pyfunction!(rans_64, m)?)?;
    m.add_class::<RansEncoderStream>()?;
    m.add_class::<RansDecoderStream>()?;
    m.add_class::<PyEntropyEncoder>()?;
    m.add_class::<PyEntropyDecoder>()?;
    Ok(())
}
