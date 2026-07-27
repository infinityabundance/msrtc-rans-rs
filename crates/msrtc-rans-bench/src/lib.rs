// Copyright (c) Infinity Abundance.
// Licensed under the MIT license.

//! # msrtc-rans-bench
//!
//! Matched Rust and C++ benchmark harness for the msrtc_rans entropy coder.
//!
//! This crate provides benchmarking infrastructure that runs the Rust
//! implementation and the C++ oracle under matched conditions.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

pub mod harness {
    pub struct BenchConfig {
        pub variant: &'static str,
        pub symbol_count: usize,
        pub distribution_count: usize,
        pub iterations: usize,
    }
}
