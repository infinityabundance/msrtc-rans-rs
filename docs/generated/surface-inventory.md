# msrtc-rans-rs Surface Inventory
*Generated: 2026-07-27* — **Phase 3 — Full Entropy Coder Differential**

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
| `RansDecoder::Get` | ⚠️ `partial` | Trivial mask operation |
| `RansDecoder::Advance` | ⚠️ `partial` | Transactional; sealed via RAW.DECODER.DIFFERENTIAL |
| `RansDecoder::CheckEOF` | ⚠️ `partial` | Trivial state check |
| `Mul64Hi` | ⚠️ `partial` | Verified via u128 widening multiply; reciprocal arithmetic tested |
| Reciprocal preparation | ⚠️ `partial` | Verified vs exact division |
| Buffer growth (VecSink) | ⚠️ `partial` | Growth formula suffixes match Microsoft; allocation court pending |
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

## Python Public Surface

| Surface | Status | Notes |
|---------|--------|-------|
| `RansVariant.IntEnum` | 🔲 `scaffold` | Type exists; discriminants not yet courted |
| `RansEncoderStream` | 🔲 `scaffold` | Not implemented |
| `RansDecoderStream` | 🔲 `scaffold` | Not implemented |
| `EntropyEncoder` | 🔲 `scaffold` | Not implemented |
| `EntropyDecoder` | 🔲 `scaffold` | Not implemented |
| `_msrtc_rans` module | 🔲 `scaffold` | PyO3 module scaffold |

## Coverage Summary

| Category | Full | Partial | Scaffold | Divergent | Deferred | N/A |
|----------|------|---------|----------|-----------|----------|-----|
| C++ rANS primitives | 0 | 18 | 0 | 0 | 0 | 0 |
| C++ entropy coder | 0 | 14 | 0 | 2 | 0 | 0 |
| Python API | 0 | 0 | 6 | 0 | 0 | 0 |
| **Total** | **0** | **32** | **6** | **2** | **0** | **0** |
