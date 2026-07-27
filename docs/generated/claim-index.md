# msrtc-rans-rs Claim Index
*Generated: 2026-07-27* — **Phase 3 — Three Courts Sealed**

## Verified Claims (backed by differential court receipts)

| Claim | Evidence | Status |
|-------|----------|--------|
| Raw RansByte encoder output matches C++ oracle for all 8 test cases | `MSRTC.RAW.ENCODER.DIFFERENTIAL` — 8/8 sealed receipt | ✅ **Proved** |
| Raw Rans64 encoder output matches C++ oracle for all 8 test cases | `MSRTC.RAW.ENCODER.DIFFERENTIAL` — 8/8 sealed receipt | ✅ **Proved** |
| Prepared symbols match raw division path (both variants) | Arithmetic tests + differential court | ✅ **Proved** |
| RansByte decoder output matches C++ oracle (both directions) | `MSRTC.RAW.DECODER.DIFFERENTIAL` — 16/16 sealed receipt | ✅ **Proved** |
| Rans64 decoder output matches C++ oracle (both directions) | `MSRTC.RAW.DECODER.DIFFERENTIAL` — 16/16 sealed receipt | ✅ **Proved** |
| Full entropy encode (PMF + bypass) matches C++ oracle | `MSRTC.ENTROPY.DIFFERENTIAL` — 6/6 sealed receipt | ✅ **Proved** |
| Full entropy decode matches C++ oracle | `MSRTC.ENTROPY.DIFFERENTIAL` — 6/6 sealed receipt | ✅ **Proved** |
| C++ encode → Rust decode cross-validation matches | `MSRTC.ENTROPY.DIFFERENTIAL` roundtrip cases | ✅ **Proved** |
| VecSink growth no longer corrupts output | Growth boundary tests at 64, 65, 320, 321, 1000 | ✅ **Fixed** |
| Decoder advance is now transactional | Truncated-stream path preserved | ✅ **Fixed** |
| `Source::Outcome` abstraction removed | Simplified to `bool` | ✅ **Cleaned** |
| Encoder rejects scale_bits >= 32 | Verified in unit tests | ✅ **Intentional safety divergence** |

## Claims NOT Made

| Claim | Reason |
|-------|--------|
| "Byte-identical to Microsoft oracle for all inputs" | Differential court covers 30 cases, not exhaustive |
| "Drop-in replacement" | Python API not implemented; API types differ slightly |
| "Works with MLVC" | MLVC integration not tested |
| "Performance competitive" | Benchmarks not run |
| "Memory-safe replacement" | Formal claim pending full memory safety audit |
| "No correctness bugs remain" | Residual MSRTC.RAW.SCALE32 open; further inputs may reveal issues |

## Open Residuals

| ID | Classification | Description | Status |
|----|---------------|-------------|--------|
| `MSRTC.RAW.SCALE32` | `intentional_safety_divergence` | `scale_bits=32` causes undefined shift in C++; Rust rejects deterministically with RawRansError | `open` |
| `MSRTC.RAW.SYMBOLBITS32` | `intentional_safety_divergence` | `symbol_bits=32` rejected in Rust; C++ has undefined behavior | `open` |
| `MSRTC.RAW.BYPASSBITS32` | `intentional_safety_divergence` | `bypass_bits=32` rejected in Rust; C++ has undefined behavior | `open` |

## Resolved Residuals

| ID | Classification | Court | Resolution |
|----|---------------|-------|------------|
| Entropy RansByte native mismatch (seed 0) | `native_bug` | `MSRTC.ENTROPY.DIFFERENTIAL` | ✅ **Resolved** — Now passes (6/6) |
| Entropy Rans64 native mismatch (seed 1) | `native_bug` | `MSRTC.ENTROPY.DIFFERENTIAL` | ✅ **Resolved** — Now passes (6/6) |

## Methodological Gaps

| Gap | Impact | Next Action |
|-----|--------|-------------|
| `xtask gen` is a TODO | Docs not regenerable | Implement document generation |
| Docker matrix incomplete | Only Debian oracle cell exists | Add Ubuntu, Fedora, Alpine |
| No fuzz testing | Random inputs not explored | Add differential fuzzing |
| No cross-platform testing | Only x86_64 Linux tested | Add ARM, macOS verification |
