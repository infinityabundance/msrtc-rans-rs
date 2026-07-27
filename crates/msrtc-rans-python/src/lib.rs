// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # msrtc-rans-python
//!
//! Python extension module `_msrtc_rans` providing the msrtc.rans Python API.
//!
//! This crate uses PyO3 to create a CPython extension module that is
//! import-path-compatible with the existing `msrtc.rans` package.

#![allow(missing_docs)]
#![allow(unsafe_op_in_unsafe_fn)]

use pyo3::exceptions::PyValueError;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyByteArray;
use pyo3::types::PyBytes;

use msrtc_rans::entropy::{EntropyDecoder, EntropyEncoder};
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

#[pyclass(name = "RansEncoderStream")]
struct RansEncoderStream {
    segments: Vec<Vec<u8>>,
    #[allow(dead_code)]
    variant: i32,
    #[allow(dead_code)]
    _initial_size: usize,
    #[allow(dead_code)]
    _max_size_step: usize,
}

#[pymethods]
impl RansEncoderStream {
    #[new]
    #[pyo3(signature = (variant=1, *, initialSize=4096, maxSizeStep=1048576))]
    fn new(variant: i32, initialSize: usize, maxSizeStep: usize) -> Self {
        Self {
            segments: Vec::new(),
            variant,
            _initial_size: initialSize,
            _max_size_step: maxSizeStep,
        }
    }

    fn flush(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let total_len: usize = self.segments.iter().map(|s| s.len()).sum();
        let mut buffer = Vec::with_capacity(total_len);
        for segment in self.segments.iter().rev() {
            buffer.extend_from_slice(segment);
        }
        self.segments.clear();
        // Use FFI to create PyBytes (PyBytes::new not available in PyO3 0.22 abi3)
        let ptr = unsafe {
            ffi::PyBytes_FromStringAndSize(
                buffer.as_ptr() as *const ffi::Py_ssize_t as *const i8,
                buffer.len() as ffi::Py_ssize_t,
            )
        };
        if ptr.is_null() {
            return Err(PyValueError::new_err("failed to create PyBytes"));
        }
        let obj: Py<PyAny> = unsafe { Bound::from_owned_ptr(py, ptr).unbind() };
        Ok(obj)
    }

    fn reset(&mut self) {
        self.segments.clear();
    }
}

// ---------------------------------------------------------------------------
// RansDecoderStream
// ---------------------------------------------------------------------------

#[pyclass(name = "RansDecoderStream")]
struct RansDecoderStream {
    data: Option<Vec<u8>>,
    offset: usize,
    #[allow(dead_code)]
    _variant: i32,
}

#[pymethods]
impl RansDecoderStream {
    #[new]
    #[pyo3(signature = (data=None, *, variant=1))]
    fn new(data: Option<Bound<'_, PyAny>>, variant: i32) -> PyResult<Self> {
        let vec = match data {
            Some(ref obj) => Some(get_u8_buffer(obj)?),
            None => None,
        };
        Ok(Self {
            data: vec,
            offset: 0,
            _variant: variant,
        })
    }

    fn open(&mut self, data: Bound<'_, PyAny>) -> PyResult<()> {
        self.data = Some(get_u8_buffer(&data)?);
        self.offset = 0;
        Ok(())
    }

    fn close(&mut self) {
        self.data = None;
        self.offset = 0;
    }

    #[pyo3(name = "isOpen")]
    fn is_open(&self) -> bool {
        self.data.is_some()
    }

    #[pyo3(name = "decodeEOF")]
    fn decode_eof(&mut self) -> PyResult<()> {
        if let Some(ref data) = self.data {
            if self.offset != data.len() {
                return Err(PyValueError::new_err(format!(
                    "decodeEOF: stream not fully consumed (offset={}, len={})",
                    self.offset,
                    data.len()
                )));
            }
        }
        self.close();
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

        match self.variant {
            1 => {
                if let Some(ref encoder) = self.byte_encoder {
                    let mut buffer = Vec::new();
                    encoder
                        .encode(&indices_vec, &values_vec, &mut buffer)
                        .map_err(|e| PyValueError::new_err(format!("encode failed: {}", e)))?;
                    stream.segments.push(buffer);
                    Ok(())
                } else {
                    Err(PyValueError::new_err("byte encoder not initialized"))
                }
            }
            0 => {
                if let Some(ref encoder) = self._64_encoder {
                    let mut buffer = Vec::new();
                    encoder
                        .encode(&indices_vec, &values_vec, &mut buffer)
                        .map_err(|e| PyValueError::new_err(format!("encode failed: {}", e)))?;
                    stream.segments.push(buffer);
                    Ok(())
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
            let stream_data = stream_ref
                .data
                .as_ref()
                .ok_or_else(|| PyValueError::new_err("RansDecoderStream is not open"))?;
            let remaining = stream_data[stream_ref.offset..].to_vec();
            let current_offset = stream_ref.offset;

            let mut decoded = vec![0i32; num_values];

            let consumed = match self.variant {
                1 => {
                    if let Some(ref decoder) = self.byte_decoder {
                        decoder
                            .decode_partial(&mut decoded, &indices_vec, &remaining)
                            .map_err(|e| PyValueError::new_err(format!("decode failed: {}", e)))?
                    } else {
                        return Err(PyValueError::new_err("byte decoder not initialized"));
                    }
                }
                0 => {
                    if let Some(ref decoder) = self._64_decoder {
                        decoder
                            .decode_partial(&mut decoded, &indices_vec, &remaining)
                            .map_err(|e| PyValueError::new_err(format!("decode failed: {}", e)))?
                    } else {
                        return Err(PyValueError::new_err("64 decoder not initialized"));
                    }
                }
                _ => return Err(PyValueError::new_err("invalid variant")),
            };

            stream_ref.offset = current_offset + consumed;
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
