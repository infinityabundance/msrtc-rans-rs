# msrtc-rans-rs Parity Matrix
*Generated: 2026-07-27* — **Phase 3 — Three Courts Sealed**

## Raw rANS Primitives

| Operation | RansByte | Rans64 | Current Status |
|-----------|----------|--------|----------------|
| Encoder initialization | ⚠️ | ⚠️ | `partial` — sealed in DIFFERENTIAL court |
| `Put(start, freq, scale)` | ⚠️ | ⚠️ | `partial` — sealed in DIFFERENTIAL court |
| `Put(symbol)` | ⚠️ | ⚠️ | `partial` — sealed in DIFFERENTIAL court |
| `Flush()` | ⚠️ | ⚠️ | `partial` — sealed in DIFFERENTIAL court |
| `Reset()` | ⚠️ | ⚠️ | `partial` — trivial state reset |
| Renormalization | ⚠️ | ⚠️ | `partial` — sealed in DIFFERENTIAL court |
| Mul64Hi | ⚠️ | ⚠️ | `partial` — verified via u128 widening |
| Reciprocal (freq>1) | ⚠️ | ⚠️ | `partial` — verified vs exact division |
| Reciprocal (freq=1) | ⚠️ | ⚠️ | `partial` — special case handling |
| Decoder `Init()` | ⚠️ | ⚠️ | `partial` — sealed in DECODER.DIFFERENTIAL |
| Decoder `Get()` | ⚠️ | ⚠️ | `partial` — sealed in DECODER.DIFFERENTIAL |
| Decoder `Advance()` | ⚠️ | ⚠️ | `partial` — transactional; sealed in DECODER.DIFFERENTIAL |
| Decoder `CheckEOF()` | ⚠️ | ⚠️ | `partial` — sealed in DECODER.DIFFERENTIAL |

## Prepared Symbols

| Feature | Status | Notes |
|---------|--------|-------|
| Encoder symbol precomputation | ⚠️ `partial` | Matches division reference; sealed in DIFFERENTIAL |
| Decoder symbol (start/freq) | ⚠️ `partial` | Simple struct |
| Quotient via Mul64Hi | ⚠️ `partial` | Both u64 and u32 paths |

## Entropy Coder

| Feature | Status | Notes |
|---------|--------|-------|
| `EntropyEncoder::new()` | ⚠️ `partial` | Constructs uninitialized encoder |
| `EntropyEncoder::initialize()` | ⚠️ `partial` | Full PMF validation and descriptor init |
| `EntropyEncoder::encode()` | ⚠️ `partial` | Sealed in ENTROPY.DIFFERENTIAL (6 cases) |
| `EntropyDecoder::new()` | ⚠️ `partial` | Constructs uninitialized decoder |
| `EntropyDecoder::initialize()` | ⚠️ `partial` | CDF table construction from PMF |
| `EntropyDecoder::decode()` | ⚠️ `partial` | Sealed in ENTROPY.DIFFERENTIAL |
| Encode in-range values (PMF path) | ⚠️ `partial` | Sealed in ENTROPY.DIFFERENTIAL |
| Encode out-of-range values (bypass) | ⚠️ `partial` | Variable bypass bits per distribution |
| Decode in-range values (CDF path) | ⚠️ `partial` | Sealed in ENTROPY.DIFFERENTIAL |
| Decode bypass values | ⚠️ `partial` | Value offset reconstruction |
| Mixed in-range + bypass encoding | ⚠️ `partial` | Sealed in ENTROPY.DIFFERENTIAL (both variants) |
| C++ encode → Rust decode cross | ⚠️ `partial` | Sealed in ENTROPY.DIFFERENTIAL |
| Rust encode → C++ decode cross | — | Implemented via encoder diff court |
| PMF validation (empty lengths) | ⚠️ `partial` | Rejected with InvalidPmf |
| PMF validation (invalid scale_bits) | ⚠️ `partial` | Rejected for scale_bits > 31 |
| PMF validation (bypass_bits > 31) | ⚠️ `partial` | Rejected for bypass_bits > 31 |
| Multi-distribution encoding | ⚠️ `partial` | Multiple PMFs with offsets |
| Bypass bits 2, 3, 8 | ⚠️ `partial` | Variable bypass widths tested |
| Multiple bypass values | ⚠️ `partial` | Multiple out-of-range values per stream |
| Large positive outlier bypass | ⚠️ `partial` | i32 max value tested |
| Extreme negative outlier bypass | ⚠️ `partial` | i32 min + 1 tested |

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
| No MLVC integration test | 🟡 **Gap** | Not started |

## Legend

- ✅ `full` — Implemented, differentially tested, receipt sealed
- ⚠️ `partial` — Implemented but not yet differentially verified
- 🔲 `scaffold` — Exists but untested
- ❌ `divergent` — Known behavioral difference
- — `n/a` — Not applicable
- 🔴 `fixed` — Bug that was found and corrected
- 🟢 `sealed` — Court sealed with receipt
- 🟡 `residual` — Classified, structured mismatch record
