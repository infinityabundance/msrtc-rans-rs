# msrtc-rans

**Safe public Rust entropy-coder API for msrtc_rans.**

This crate provides the high-level entropy coder API, wrapping the raw rANS primitives from `msrtc-rans-core` with PMF validation, CDF table construction, bypass coding, distribution descriptors, **persistent streams**, and the **resizable buffer** allocation layer.

## Status — Phases 3 + 4 Sealed ✅

| Feature | Status |
|---------|--------|
| `EntropyEncoder` | ✅ Implemented and courted |
| `EntropyDecoder` | ✅ Implemented and courted |
| PMF validation | ✅ Implemented and courted |
| CDF table construction | ✅ Implemented and courted |
| Bypass coding (variable-width) | ✅ Implemented and courted |
| `RansEncoderStream` (persistent push) | ✅ Sealed — `MSRTC.STREAM.DIFFERENTIAL` 24/24 |
| `RansDecoderStream` (persistent cursor) | ✅ Sealed — `MSRTC.STREAM.DIFFERENTIAL` 24/24 |
| `ResizableBuffer` / `HeapResizableBuffer` | ✅ Implemented — Microsoft growth formula |
| Entropy differential court | ✅ Sealed (6/6 cases) |
| Stream differential court | ✅ Sealed (24/24 cases) |

The full entropy encode/decode pipeline (PMF → CDF → bypass) and the persistent stream layer have been verified byte-for-byte against the pinned Microsoft C++ oracle. All 46 tests in this crate pass.

## Features

- `#![forbid(unsafe_code)]` — pure safe Rust
- Both RansByte (u8) and Rans64 (u32) variant support
- PMF validation: rejects empty tables, invalid dimensions, zero frequencies
- Bypass coding for out-of-range values with configurable bypass bits
- Mixed in-range and bypass value streams
- **Persistent streams** — one raw rANS state across `push()` calls (Microsoft `RawRansEncoderStream`); LIFO multipart layout
- **Resizable buffer** — `new = old + min(old, max_size_step)` growth, rollback, backward-writing sink
- Comprehensive error handling via `EntropyError` enum
- Re-exports all `msrtc-rans-core` types and traits

## Usage

### Entropy Encoding/Decoding

```rust
use msrtc_rans::entropy::EntropyEncoder;
use msrtc_rans::variant::RansByte;

let mut encoder = EntropyEncoder::<RansByte>::new();

let pmf_lengths = vec![2i32];
let pmf_offsets = vec![0i32];
let pmf_table = vec![1i32, 3i32];  // frequencies
let symbol_bits = 16;
let bypass_bits = 4;

encoder.initialize(&pmf_lengths, &pmf_offsets, &pmf_table, symbol_bits, bypass_bits)
    .expect("valid PMF");

let indices = vec![0i32, 0i32, 0i32, 0i32];
let values = vec![-2i32, 1i32, 0i32, 1i32];
let mut buffer = Vec::new();
encoder.encode(&indices, &values, &mut buffer).expect("encode");
```

### Multipart stream (persistent encoder)

```rust
use msrtc_rans::entropy::EntropyEncoder;
use msrtc_rans::stream::{RansEncoderStream, RansDecoderStream};
use msrtc_rans::variant::RansByte;

// Batch A pushed first, batch B second → decode B first, then A (LIFO).
let mut stream = RansEncoderStream::<RansByte>::new();
stream.push(&encoder_a, &indices_a, &values_a).expect("push a");
stream.push(&encoder_b, &indices_b, &values_b).expect("push b");
let data = stream.flush().expect("flush");

let mut dstream = RansDecoderStream::<RansByte>::open_on(&data);
// decode batch B, then batch A, then decode_eof().
```

### Resizable buffer

```rust
use msrtc_rans::buffer::{HeapResizableBuffer, ResizableBufferSink};

let mut buffer = HeapResizableBuffer::new(4096, 1024 * 1024);
let mut sink = ResizableBufferSink::<u8>::new(&mut buffer);
sink.write_u8(0xAB);
sink.write_u8(0xCD);
assert_eq!(sink.encoded_bytes(), &[0xCD, 0xAB]); // backward-written
```

### Raw rANS (re-exported from msrtc-rans-core)

```rust
use msrtc_rans::{RansByteEncoder, RansByteDecoder, VecSink, SliceSource};

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

## Repository

Full project: [github.com/infinityabundance/msrtc-rans-rs](https://github.com/infinityabundance/msrtc-rans-rs)

## License

MIT — see [LICENSE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/LICENSE) and [NOTICE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/NOTICE) for attribution notices.
