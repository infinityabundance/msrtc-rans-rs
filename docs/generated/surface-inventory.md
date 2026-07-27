# msrtc-rans-rs Surface Inventory
*Generated: 2026-07-27T04:30:00Z* — **Updated after forensic review**

## Upstream Commit
`0500356a8d6146dd8dc8911022cbeca19675614f` (Microsoft/mlvc)

## C++ Public Surface

| Surface | Status | Notes |
|---------|--------|-------|
| `RansEncSymbol` | `partial` | Implemented; not yet differentially tested against oracle |
| `RansDecSymbol` | `partial` | Implemented; not yet differentially tested |
| `RansEncoder::Put(raw)` | `partial` | Equation matches; no oracle comparison |
| `RansEncoder::Put(symbol)` | `partial` | Reciprocal path matches raw path; no oracle comparison |
| `RansEncoder::Flush` | `partial` | Sequence matches; no oracle comparison |
| `RansEncoder::Reset` | `partial` | Trivial state reset |
| `RansDecoder::Init` | `partial` | Sequence matches; no oracle comparison |
| `RansDecoder::Get` | `partial` | Trivial mask operation |
| `RansDecoder::Advance` | `partial` | Transactional; no oracle comparison |
| `RansDecoder::CheckEOF` | `partial` | Trivial state check |
| `Mul64Hi` | `partial` | Verified via u128; no oracle comparison |
| Reciprocal preparation | `partial` | Verified vs exact division; no oracle comparison |
| Buffer growth (VecSink) | `partial` | Formula and suffix relocation match Microsoft; oracle allocation court pending |
| Truncated-stream handling | `partial` | Now transactional; no oracle comparison |
| `try_new()` (checked symbol) | `partial` | Rejects scale_bits outside [2, 31] |
| `try_put_raw()` (checked encoder) | `partial` | Rejects scale_bits >= 32 |
| `try_get()` (checked decoder) | `partial` | Rejects scale_bits >= 32 |
| `try_advance()` (checked decoder) | `partial` | Rejects scale_bits >= 32 |
| `RawRansError` type | `partial` | Error types for checked API |

## Python Public Surface

| Surface | Status | Notes |
|---------|--------|-------|
| `RansVariant.IntEnum` | `scaffold` | Type exists; discriminants not yet courted |
| `RansEncoderStream` | `scaffold` | Not implemented |
| `RansDecoderStream` | `scaffold` | Not implemented |
| `EntropyEncoder` | `scaffold` | Not implemented |
| `EntropyDecoder` | `scaffold` | Not implemented |
| `_msrtc_rans` module | `scaffold` | PyO3 module scaffold |

## Coverage Summary

| Category | Full | Partial | Scaffold | Divergent | Deferred | N/A |
|----------|------|---------|----------|-----------|----------|-----|
| C++ rANS primitives | 0 | 18 | 0 | 0 | 0 | 0 |
| C++ entropy coder | 0 | 0 | 7 | 0 | 0 | 0 |
| Python API | 0 | 0 | 6 | 0 | 0 | 0 |
| **Total** | **0** | **18** | **13** | **0** | **0** | **0** |
