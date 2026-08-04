# msrtc-rans-rs Parity Matrix
*Generated: 2026-08-04* — **Phase 4 — Four Courts Sealed**

## Raw rANS Primitives

| Operation | RansByte | Rans64 | Current Status |
|-----------|----------|--------|----------------|
| Encoder initialization | 🔒 | 🔒 | `sealed` — sealed in ENCODER.DIFFERENTIAL court |
| `Put(start, freq, scale)` | 🔒 | 🔒 | `sealed` — sealed in ENCODER.DIFFERENTIAL court |
| `Put(symbol)` | 🔒 | 🔒 | `sealed` — sealed in ENCODER.DIFFERENTIAL court |
| `Flush()` | 🔒 | 🔒 | `sealed` — sealed in ENCODER.DIFFERENTIAL court |
| `Reset()` | 🔒 | 🔒 | `sealed` — trivial state reset |
| Renormalization | 🔒 | 🔒 | `sealed` — sealed in ENCODER.DIFFERENTIAL court |
| Mul64Hi | 🔒 | 🔒 | `sealed` — verified via u128 widening |
| Reciprocal (freq>1) | 🔒 | 🔒 | `sealed` — verified vs exact division |
| Reciprocal (freq=1) | 🔒 | 🔒 | `sealed` — special case handling |
| Decoder `Init()` | 🔒 | 🔒 | `sealed` — sealed in DECODER.DIFFERENTIAL |
| Decoder `Get()` | 🔒 | 🔒 | `sealed` — sealed in DECODER.DIFFERENTIAL |
| Decoder `Advance()` | 🔒 | 🔒 | `sealed` — transactional; sealed in DECODER.DIFFERENTIAL |
| Decoder `CheckEOF()` | 🔒 | 🔒 | `sealed` — sealed in DECODER.DIFFERENTIAL |

## Prepared Symbols

| Feature | Status | Notes |
|---------|--------|-------|
| Encoder symbol precomputation | 🔒 `sealed` | Matches division reference; sealed in DIFFERENTIAL |
| Decoder symbol (start/freq) | 🔒 `sealed` | Simple struct |
| Quotient via Mul64Hi | 🔒 `sealed` | Both u64 and u32 paths |

## Entropy Coder

| Feature | Status | Notes |
|---------|--------|-------|
| `EntropyEncoder::new()` | 🔒 `sealed` | Constructs uninitialized encoder |
| `EntropyEncoder::initialize()` | 🔒 `sealed` | Full PMF validation and descriptor init |
| `EntropyEncoder::encode()` | 🔒 `sealed` | Sealed in ENTROPY.DIFFERENTIAL (6 cases) |
| `EntropyDecoder::new()` | 🔒 `sealed` | Constructs uninitialized decoder |
| `EntropyDecoder::initialize()` | 🔒 `sealed` | CDF table construction from PMF |
| `EntropyDecoder::decode()` | 🔒 `sealed` | Sealed in ENTROPY.DIFFERENTIAL |
| Encode in-range values (PMF path) | 🔒 `sealed` | Sealed in ENTROPY.DIFFERENTIAL |
| Encode out-of-range values (bypass) | 🔒 `sealed` | Variable bypass bits per distribution |
| Decode in-range values (CDF path) | 🔒 `sealed` | Sealed in ENTROPY.DIFFERENTIAL |
| Decode bypass values | 🔒 `sealed` | Value offset reconstruction |
| Mixed in-range + bypass encoding | 🔒 `sealed` | Sealed in ENTROPY.DIFFERENTIAL (both variants) |
| C++ encode → Rust decode cross | 🔒 `sealed` | Sealed in ENTROPY.DIFFERENTIAL |
| Rust encode → C++ decode cross | 🔒 `sealed` | Implemented via encoder diff court |
| PMF validation (empty lengths) | 🔒 `sealed` | Rejected with InvalidPmf |
| PMF validation (invalid scale_bits) | 🔒 `sealed` | Rejected for scale_bits > 31 |
| PMF validation (bypass_bits > 31) | 🔒 `sealed` | Rejected for bypass_bits > 31 |
| Multi-distribution encoding | 🔒 `sealed` | Multiple PMFs with offsets |
| Bypass bits 2, 3, 8 | 🔒 `sealed` | Variable bypass widths tested |
| Multiple bypass values | 🔒 `sealed` | Multiple out-of-range values per stream |
| Large positive outlier bypass | 🔒 `sealed` | i32 max value tested |
| Extreme negative outlier bypass | 🔒 `sealed` | i32 min + 1 tested |

## Streams & Allocation (Phase 4)

| Feature | RansByte | Rans64 | Current Status |
|---------|----------|--------|----------------|
| `IResizableBuffer` pattern | 🔒 | 🔒 | `sealed` — `ResizableBuffer` trait + `HeapResizableBuffer` |
| Buffer growth policy | 🔒 | 🔒 | `sealed` — `new = old + min(old, max_step)`; max_step floored at 512; unit-tested |
| Growth relocation (content to END) | 🔒 | 🔒 | `sealed` — matches Microsoft `newBuffer.last(content.size())` |
| Rollback semantics | 🔒 | 🔒 | `sealed` — `rollback()` discards pending grow |
| `RansEncoderStream` persistent push | 🔒 | 🔒 | `sealed` — STREAM.DIFFERENTIAL wire parity (8/8) |
| `RansEncoderStream::Flush` once | 🔒 | 🔒 | `sealed` — flush returns full stream; reset for reuse |
| `RansEncoderStream::Reset` (abort) | 🔒 | 🔒 | `sealed` — discards session (C++ `Flush(abort=true)`) |
| `RansDecoderStream` persistent cursor | 🔒 | 🔒 | `sealed` — STREAM.DIFFERENTIAL (16/16 decode results) |
| `RansDecoderStream::CheckEOF` | 🔒 | 🔒 | `sealed` — source EOF + state == LowerBound |
| `RansDecoderStream::DecodeEOF` | 🔒 | 🔒 | `sealed` — STREAM.DIFFERENTIAL decode sub-cases |
| Multipart wire layout (LIFO decode) | 🔒 | 🔒 | `sealed` — push 2,1 → flush → decode 1,2 matches Microsoft |
| Python multipart API | 🔒 | 🔒 | `sealed` — upstream `test_encode_decode_multi_part_0` passes |

## Known Issues

| Issue | Severity | Status |
|-------|----------|--------|
| VecSink growth corrupts output | 🔴 **Fixed** | Growth now copies content to new buffer end |
| Decoder commits state early | 🔴 **Fixed** | Now transactional |
| `scale_bits == 32` edge case | 🟡 **Residual** | Intentional safety divergence — Rust rejects |
| `symbol_bits == 32` rejection | 🟡 **Residual** | Intentional safety divergence — Rust rejects |
| `bypass_bits == 32` rejection | 🟡 **Residual** | Intentional safety divergence — Rust rejects |
| No Docker matrix (multi-distro) | 🟡 **Gap** | Debian only; Ubuntu, Fedora, Alpine pending |
| No receipt regeneration infrastructure | 🟡 **Gap** | `xtask gen` not implemented |
| No MLVC integration test | 🟡 **Gap** | Phase 7 pending |

## Legend

- 🔒 `sealed` — Implemented, differentially tested, receipt sealed
- ⚠️ `partial` — Implemented but not yet differentially verified
- 🔲 `scaffold` — Exists but untested
- ✅ `divergent` — Known behavioral difference (intentional)
- 🔴 `fixed` — Bug that was found and corrected
- 🟢 `sealed` — Court sealed with receipt
- 🟡 `residual` — Classified, structured mismatch record
