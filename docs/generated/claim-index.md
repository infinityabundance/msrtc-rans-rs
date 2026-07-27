# msrtc-rans-rs Claim Index
*Generated: 2026-07-27T04:30:00Z* — **Updated after forensic review**

## Verified Claims (backed by self-consistency tests)

| Claim | Evidence | Status |
|-------|----------|--------|
| Raw encoder equation matches reference | Self-consistent (prepared == raw) | ⚠️ `partial` |
| Reciprocal division matches exact division | Arithmetic tests | ⚠️ `partial` |
| Encoder/decoder round-trips internally | Self-consistency tests pass | ⚠️ `partial` |
| VecSink growth no longer corrupts output | Growth boundary tests at 64, 65, 320, 321, 1000 | ✅ Fixed |
| Decoder advance is now transactional | Truncated-stream path preserved | ✅ Fixed |
| `Source::Outcome` abstraction removed | Simplified to `bool` | ✅ Cleaned |

## Claims NOT Made

| Claim | Reason |
|-------|--------|
| "Byte-identical to Microsoft oracle" | No differential court sealed |
| "Drop-in replacement" | Python API not implemented |
| "Works with MLVC" | MLVC integration not tested |
| "Performance competitive" | Benchmarks not run |
| "Memory-safe replacement" | Formal claim pending full parity verification |
| "Full parity for any raw primitive" | Every item is `partial` without oracle comparison |

## Open Residuals

| ID | Classification | Description | Status |
|----|---------------|-------------|--------|
| `MSRTC.RAW.SCALE32` | `oracle_undefined_or_assert_only` | `scale_bits=32` causes undefined shift in C++; Rust should reject deterministically | `open` |

## Methodological Gaps

| Gap | Impact | Next Action |
|-----|--------|-------------|
| No actual court receipts exist | Evidence not sealed | Implement receipt generation |
| `xtask gen` is a TODO | Docs not regenerable | Implement document generation |
| No `court-index.md` generated | Freshness gate incomplete | Implement court enumeration |
| Docker matrix incomplete | Only Debian oracle cell exists | Add Ubuntu, Fedora, Alpine |
| No run-scoped Docker naming | Resource identification | Add run IDs and labels |
