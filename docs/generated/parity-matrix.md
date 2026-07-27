# msrtc-rans-rs Parity Matrix
*Generated: 2026-07-27T04:30:00Z* — **Updated after forensic review**

## Raw rANS Primitives

| Operation | RansByte | Rans64 | Current Status |
|-----------|----------|--------|----------------|
| Encoder initialization | ⚠️ | ⚠️ | `partial` — self-consistent, no oracle comparison |
| `Put(start, freq, scale)` | ⚠️ | ⚠️ | `partial` |
| `Put(symbol)` | ⚠️ | ⚠️ | `partial` — prepared matches raw, no oracle |
| `Flush()` | ⚠️ | ⚠️ | `partial` |
| `Reset()` | ⚠️ | ⚠️ | `partial` |
| Renormalization | ⚠️ | ⚠️ | `partial` |
| Mul64Hi | ⚠️ | ⚠️ | `partial` |
| Reciprocal (freq>1) | ⚠️ | ⚠️ | `partial` |
| Reciprocal (freq=1) | ⚠️ | ⚠️ | `partial` |
| Decoder `Init()` | ⚠️ | ⚠️ | `partial` |
| Decoder `Get()` | ⚠️ | ⚠️ | `partial` |
| Decoder `Advance()` | ⚠️ | ⚠️ | `partial` — transactional |
| Decoder `CheckEOF()` | ⚠️ | ⚠️ | `partial` |

## Prepared Symbols

| Feature | Status | Notes |
|---------|--------|-------|
| Encoder symbol precomputation | ⚠️ `partial` | Matches division reference; no oracle |
| Decoder symbol (start/freq) | ⚠️ `partial` | Simple struct |
| Quotient via Mul64Hi | ⚠️ `partial` | Both u64 and u32 paths |

## Known Issues

| Issue | Severity | Status |
|-------|----------|--------|
| VecSink growth corrupts output | 🔴 Fixed | Growth now copies content to new buffer end |
| Decoder commits state early | 🔴 Fixed | Now transactional |
| `scale_bits == 32` edge case | 🟡 Residual | Needs classification |
| No raw oracle differential court | 🟡 Gap | `MSRTC.RAW.*` courts return scaffold |
| No receipt or transcript infrastructure | 🟡 Gap | Pending implementation |

## Legend

- ✅ `full` — Implemented, differentially tested, receipt sealed
- ⚠️ `partial` — Implemented but not yet differentially verified
- 🔲 `scaffold` — Exists but untested
- ❌ `divergent` — Known behavioral difference
- — `n/a` — Not applicable
- 🔴 `fixed` — Bug that was found and corrected
