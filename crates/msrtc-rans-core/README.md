# msrtc-rans-core

**Deterministic, no_std-capable rANS primitives for the msrtc_rans entropy coder.**

This crate implements the raw rANS encoding and decoding primitives used by Microsoft MLVC's `msrtc_rans` package. The arithmetic (reciprocal preparation, Mul64Hi, fast division) is structurally faithful to the C++ implementation.

## Status — Raw Engine Sealed ✅

| Court | Status |
|-------|--------|
| Raw encoder differential | ✅ Sealed (8/8 cases) |
| Raw decoder differential | ✅ Sealed (16/16 cases) |

Both `RansByteEncoder` / `RansByteDecoder` (u32 state, u8 unit) and `Rans64Encoder` / `Rans64Decoder` (u64 state, u32 unit) have been verified byte-for-byte against the pinned Microsoft C++ oracle. All 30 core tests pass.

## Features

- `#![no_std]` — suitable for embedded and WASM targets
- `#![forbid(unsafe_code)]` — pure safe Rust
- Macro-generated variants from a single `generate_rans_impl!` macro
- `VecSink` (reverse-order, matching C++ `ResizableBufferSink` behavior)
- `SliceSource` (zero-copy span-like source) with `seek(pos)` for continuation decoding
- `Decoder::from_state(source, state)` — construct a decoder with a saved state (persistent stream decoding)
- Checked API: `try_new()`, `try_put_raw()`, `try_get()`, `try_advance()`
- Transactional decoder — state not committed on failed advance

## Usage

```rust
use msrtc_rans_core::{RansByteEncoder, RansByteDecoder};
use msrtc_rans_core::sink::VecSink;
use msrtc_rans_core::source::SliceSource;

let sink = VecSink::<u8>::new(64);
let mut encoder = RansByteEncoder::new(sink);
encoder.put_raw(0, 128, 8);
encoder.flush();
let encoded = encoder.into_sink().encoded().to_vec();

let source = SliceSource::new(&encoded);
let mut decoder = RansByteDecoder::new(source);
assert!(decoder.init());
assert!(decoder.advance(0, 128, 8));
assert!(decoder.check_eof());
```

### Persistent continuation (stream mode)

```rust
use msrtc_rans_core::{RansByteDecoder, RansByteEncoder};
use msrtc_rans_core::sink::VecSink;
use msrtc_rans_core::source::SliceSource;

let sink = VecSink::<u8>::new(64);
let mut encoder = RansByteEncoder::new(sink);
encoder.put_raw(0, 128, 8);
encoder.flush();
let units = encoder.into_sink().encoded().to_vec();

// Save a decode cursor (position + state), then continue later.
let mut source = SliceSource::new(&units);
let mut decoder = RansByteDecoder::new(source);
assert!(decoder.init());
let pos = decoder.source().position();
let state = decoder.state();
// ... later, from a saved (pos, state):
let mut source2 = SliceSource::new(&units);
source2.seek(pos);
let mut decoder2 = RansByteDecoder::from_state(source2, state);
```

## Repository

Full project: [github.com/infinityabundance/msrtc-rans-rs](https://github.com/infinityabundance/msrtc-rans-rs)

## License

MIT — see [LICENSE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/LICENSE) and [NOTICE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/NOTICE) for attribution notices.
