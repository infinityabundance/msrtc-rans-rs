# msrtc-rans-rs Surface Inventory
*Generated: 2026-08-04* — **Phase 4 — Streams & Allocation Complete**

## Upstream Commit
`0500356a8d6146dd8dc8911022cbeca19675614f` (Microsoft/mlvc)

## C++ Public Surface — Raw rANS Primitives

| Surface | Status | Notes |
|---------|--------|-------|
| `RansEncSymbol` | ⚠️ `partial` | Implemented; sealed in RAW.ENCODER.DIFFERENTIAL court (8 cases) |
| `RansDecSymbol` | ⚠️ `partial` | Implemented; sealed in RAW.DECODER.DIFFERENTIAL court (16 cases) |
| `RansEncoder::Put(raw)` | ⚠️ `partial` | Sealed via RAW.ENCODER.DIFFERENTIAL |
| `RansEncoder::Put(symbol)` | ⚠️ `partial` | Sealed via RAW.ENCODER.DIFFERENTIAL |
| `RansEncoder::Flush` | ⚠️ `partial` | Sealed via RAW.ENCODER.DIFFERENTIAL |
| `RansEncoder::Reset` | ⚠️ `partial` | Trivial state reset |
| `RansDecoder::Init` | ⚠️ `partial` | Sealed via RAW.DECODER.DIFFERENTIAL |
| `RansDecoder::Get` | ⚠️ `partial` | Sealed via RAW.DECODER.DIFFERENTIAL |
| `RansDecoder::Advance` | ⚠️ `partial` | Transactional; sealed via RAW.DECODER.DIFFERENTIAL |
| `RansDecoder::CheckEOF` | ⚠️ `partial` | Sealed via RAW.DECODER.DIFFERENTIAL |
| `Mul64Hi` | ⚠️ `partial` | Verified via u128 widening multiply; reciprocal arithmetic tested |
| Reciprocal preparation | ⚠️ `partial` | Verified vs exact division |
| Buffer growth (VecSink) | ⚠️ `partial` | Growth formula suffixes match Microsoft; verified fixed |
| Truncated-stream handling | ⚠️ `partial` | Transactional design verified |
| `try_new()` (checked symbol) | ⚠️ `partial` | Rejects scale_bits outside [2, 31] |
| `try_put_raw()` (checked encoder) | ⚠️ `partial` | Rejects scale_bits >= 32 |
| `try_get()` (checked decoder) | ⚠️ `partial` | Rejects scale_bits >= 32 |
| `try_advance()` (checked decoder) | ⚠️ `partial` | Rejects scale_bits >= 32 |
| `RawRansError` type | ⚠️ `partial` | Error types for checked API |

## C++ Public Surface — Entropy Coder

| Surface | Status | Notes |
|---------|--------|-------|
| `EntropyEncoder` constructor | ⚠️ `partial` | Implemented with `EncSymbol` bounds |
| `EntropyEncoder::Initialize` (PMF) | ⚠️ `partial` | Validates PMF: lengths, offsets, table, scale_bits, bypass_bits |
| `EntropyEncoder::Encode` | ⚠️ `partial` | Full encode path with bypass; sealed via ENTROPY.DIFFERENTIAL |
| `EntropyDecoder` constructor | ⚠️ `partial` | Implemented with `RansParams` bounds |
| `EntropyDecoder::Initialize` (CDF) | ⚠️ `partial` | Builds CDF table from PMF |
| `EntropyDecoder::Decode` | ⚠️ `partial` | Full decode path with CDF lookup and bypass; sealed via ENTROPY.DIFFERENTIAL |
| Bypass encoding (out-of-range values) | ⚠️ `partial` | Variable-width bypass for both variants; sealed via ENTROPY.DIFFERENTIAL |
| Bypass decoding (value reconstruction) | ⚠️ `partial` | Per-value offset reconstruction with mixed in-range/bypass; sealed via ENTROPY.DIFFERENTIAL |
| CDF table construction | ⚠️ `partial` | Frequency table to CDF mapping; tested internally |
| Distribution descriptors | ⚠️ `partial` | value_offset, bypass_sentinel, symbol_offset structure |
| PMF validation rules | ⚠️ `partial` | Length > 1, offsets non-empty, table dimension valid, scale_bits in range, all freq > 0 |
| `symbol_bits == 32` rejection | ✅ `divergent` | Intentionally rejected as intentional safety divergence |
| `bypass_bits == 32` rejection | ✅ `divergent` | Intentionally rejected as intentional safety divergence |
| `pmf_lengths.len() <= 1` rejection | ⚠️ `partial` | Rejected with `InvalidPmf` error |
| Encoder round-trip (C++ encode → Rust decode) | ⚠️ `partial` | Cross-validated in ENTROPY.DIFFERENTIAL |
| Rust encode → C++ decode cross-validate | ⚠️ `partial` | Cross-validated in ENTROPY.DIFFERENTIAL |

## C++ Public Surface — Streams & Allocation (Phase 4)

| Surface | Status | Notes |
|---------|--------|-------|
| `IResizableBuffer` interface | ⚠️ `partial` | `ResizableBuffer` trait: get_buffer / begin_to_grow / commit / rollback |
| `HeapResizableBuffer` | ⚠️ `partial` | `new = old + min(old, max_step)`; max_step floored at `MIN_BUFFER_SIZE=512`; initial size aligned to 16 |
| `ResizableBufferSink` (byte/u32) | ⚠️ `partial` | Safe backward-writing sink; growth relocates content to buffer END |
| `RansEncoderStream::Initialize` | ⚠️ `partial` | `RansEncoderStream<RansByte/Rans64>` persistent raw state across `push()` |
| `RansEncoderStream::Encode` (push) | ✅ `sealed` | Wire-parity sealed in STREAM.DIFFERENTIAL (24 results) |
| `RansEncoderStream::Flush(abort)` | ✅ `sealed` | Single flush; reset-for-reuse; abort via `reset()` |
| `RansDecoderStream::Open` | ⚠️ `partial` | Eager-init equivalent: first decode initializes raw decoder |
| `RansDecoderStream::Decode` (continue) | ✅ `sealed` | Persistent cursor (unit pos + state) via `from_state` + `seek` |
| `RansDecoderStream::Close` | ⚠️ `partial` | `close()` releases data; `isOpen()` reflects state |
| `RansDecoderStream::CheckEOF` / `DecodeEOF` | ✅ `sealed` | Position-exhausted + state==LowerBound; sealed in STREAM.DIFFERENTIAL |
| Multipart stream wire compatibility | ✅ `sealed` | Push order 2,1 → flush → decode 1,2; byte-identical to Microsoft |
| Buffer growth policy (max_size_step) | ⚠️ `partial` | Matches Microsoft: `old + min(old, max(old, 512))`; unit-tested |

## Python Public Surface

| Surface | Status | Notes |
|---------|--------|-------|
| `RansVariant.IntEnum` | ⚠️ `partial` | Implemented with `RansByte=1`, `Rans64=0` |
| `RansEncoderStream` | ⚠️ `partial` | **Persistent** encoder state across push(); matches Microsoft `RawRansEncoderStream`; multipart wire-parity proven |
| `RansDecoderStream` | ⚠️ `partial` | **Persistent** decode cursor; open/close/isOpen/decodeEOF; multipart decode proven |
| `EntropyEncoder` | ⚠️ `partial` | Full encode/push API with PMF initialization |
| `EntropyDecoder` | ⚠️ `partial` | Full decode API with stream and buffer modes |
| `_msrtc_rans` module | ⚠️ `partial` | PyO3 extension module; all 7 upstream tests pass |

## Coverage Summary

| Category | Full | Partial | Scaffold | Divergent | Deferred | N/A |
|----------|------|---------|----------|-----------|----------|-----|
| C++ rANS primitives | 0 | 18 | 0 | 0 | 0 | 0 |
| C++ entropy coder | 0 | 14 | 0 | 2 | 0 | 0 |
| C++ streams & allocation | 3 | 8 | 0 | 0 | 0 | 0 |
| Python API | 0 | 6 | 0 | 0 | 0 | 0 |
| **Total** | **3** | **46** | **0** | **2** | **0** | **0** |
