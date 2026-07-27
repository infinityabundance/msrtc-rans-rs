# msrtc-rans

**Safe public Rust entropy-coder API for msrtc_rans.**

This crate provides the high-level entropy coder API, wrapping the raw rANS primitives from `msrtc-rans-core` with PMF validation, CDF table construction, bypass coding, and distribution descriptors.

## Status

| Feature | Status |
|---------|--------|
| `EntropyEncoder` | ✅ Implemented |
| `EntropyDecoder` | ✅ Implemented |
| PMF validation | ✅ Implemented |
| CDF table construction | ✅ Implemented |
| Bypass coding (variable-width) | ✅ Implemented |
| Entropy differential court | ✅ Sealed (6/6 cases) |

The full entropy encode/decode pipeline (PMF → CDF → bypass) has been verified byte-for-byte against the pinned Microsoft C++ oracle.

## Features

- `#![forbid(unsafe_code)]` — pure safe Rust
- Both RansByte (u8) and Rans64 (u32) variant support
- PMF validation: rejects empty tables, invalid dimensions, zero frequencies
- Bypass coding for out-of-range values with configurable bypass bits
- Mixed in-range and bypass value streams
- Comprehensive error handling via `EntropyError` enum
- Re-exports all `msrtc-rans-core` types and traits

## Usage

```rust
use msrtc_rans::entropy::EntropyEncoder;
use msrtc_rans_core::RansByte;

let mut encoder = EntropyEncoder::<RansByte>::new();

let pmf_lengths = vec![2u32];
let pmf_offsets = vec![0u32];
let pmf_table = vec![1u32, 3u32];  // frequencies
let symbol_bits = 16;
let bypass_bits = 4;

encoder.initialize(&pmf_lengths, &pmf_offsets, &pmf_table, symbol_bits, bypass_bits)
    .expect("valid PMF");

let values = vec![-2i32, 1i32, 0i32, 1i32];
let encoded = encoder.encode(&values).expect("encode");
```

## Repository

Full project: [github.com/infinityabundance/msrtc-rans-rs](https://github.com/infinityabundance/msrtc-rans-rs)

## License

MIT — see [LICENSE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/LICENSE) and [NOTICE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/NOTICE) for attribution notices.
