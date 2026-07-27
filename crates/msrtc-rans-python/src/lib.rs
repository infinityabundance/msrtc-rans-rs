// Copyright (c) 2026 Riaan de Beer
// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # msrtc-rans-python
//!
//! Python extension module `_msrtc_rans` providing the msrtc.rans Python API.
//!
//! This crate uses PyO3 to create a CPython extension module that is
//! import-path-compatible with the existing `msrtc.rans` package.
//!
//! ## Safety
//!
//! This crate uses `unsafe` for PyO3 FFI interactions (required by the
//! Python C API). All unsafe is contained in the PyO3-generated bindings.

#![allow(missing_docs)]

use pyo3::prelude::*;

/// Python module for msrtc rANS implementation.
#[pymodule]
fn _msrtc_rans(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
