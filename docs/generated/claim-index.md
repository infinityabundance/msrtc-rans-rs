# msrtc-rans-rs Claim Index
*Generated: 2026-08-04* — **Phases 4 + 7 + 8 — Five Courts Sealed — All Verified**

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
| Multipart stream flush bytes match Microsoft byte-for-byte | `MSRTC.STREAM.DIFFERENTIAL` wire sub-cases — 8/8 | ✅ **Proved** |
| Microsoft stream → Rust persistent decoder recovers all values + EOF | `MSRTC.STREAM.DIFFERENTIAL` — 8/8 | ✅ **Proved** |
| Rust stream → Microsoft persistent decoder recovers all values + EOF | `MSRTC.STREAM.DIFFERENTIAL` — 8/8 | ✅ **Proved** |
| Multipart Python API matches upstream test (push 2 → flush → decode 1,2 → decodeEOF) | Upstream `test_encode_decode_multi_part_0` + `MSRTC.STREAM.DIFFERENTIAL` | ✅ **Proved** |
| `IResizableBuffer` / `HeapResizableBuffer` growth policy matches Microsoft | `new = old + min(old, max_step)`, max_step floored at 512; unit tests | ✅ **Implemented** |
| **Rust wheel is a drop-in for C++ `_msrtc_rans` in real MLVC code paths** | **Phase 7 integration** — 12/12 bitstreams byte-identical, identical bpp, identical reconstruction (conversion/_coder.py, entropy_models.py, stream_helper.py) | ✅ **Proved** |
| **Raw engine survives property sweeps + corruption** | **MSRTC.HARDENING** — 218 raw sweep checks, transactional truncation/flip robustness | ✅ **Proved** |
| **Entropy coder survives roundtrip + corrupt-stream sweeps** | **MSRTC.HARDENING** — 32 entropy/stream checks, no panics on corrupt input | ✅ **Proved** |
| **Python FFI validates buffer metadata** | ndim/format/itemsize/alignment/shape checks; exact-size output writes | ✅ **Implemented** |
| VecSink growth no longer corrupts output | Growth boundary tests at 64, 65, 320, 321, 1000 | ✅ **Fixed** |
| Decoder advance is now transactional | Truncated-stream path preserved | ✅ **Fixed** |
| `Source::Outcome` abstraction removed | Simplified to `bool` | ✅ **Cleaned** |
| Encoder rejects scale_bits >= 32 | Verified in unit tests | ✅ **Intentional safety divergence** |
| Rust rejects symbol_bits == 32 | Verified in unit tests | ✅ **Intentional safety divergence** |
| Rust rejects bypass_bits == 32 | Verified in unit tests | ✅ **Intentional safety divergence** |

## Claims NOT Made

| Claim | Reason |
|-------|--------|
| "Byte-identical to Microsoft oracle for all inputs" | Differential courts cover 54 sealed cases, not exhaustive |
| "Full MLVC pipeline (trained weights + YUV) proven" | Phase 7 proves the real `msrtc.rans` call sites; full FrameLoop needs model checkpoints |
| "Performance competitive" | Benchmarks not run (Phase 9 pending) |
| "Memory-safe replacement" | Formal claim pending full memory safety audit (Phase 8 pending) |
| "No correctness bugs remain" | Residual `MSRTC.RAW.SCALE32` open; further inputs may reveal issues |

## Open Residuals

| ID | Classification | Description | Status |
|----|---------------|-------------|--------|
| `MSRTC.RAW.SCALE32` | `intentional_safety_divergence` | `scale_bits=32` causes undefined shift in C++; Rust rejects deterministically with RawRansError | `open` |
| `MSRTC.RAW.SYMBOLBITS32` | `intentional_safety_divergence` | `symbol_bits=32` rejected in Rust; C++ has undefined behavior | `open` |
| `MSRTC.RAW.BYPASSBITS32` | `intentional_safety_divergence` | `bypass_bits=32` rejected in Rust; C++ has undefined behavior | `open` |
| `MSRTC.RAW.BYPASSSHIFT` | `intentional_safety_divergence` | Corrupt streams with a huge decoded bypass count → Rust rejects (`total_bits >= 64`); C++ shifts ≥ 32 (UB) | `open` |
| `MSRTC.RAW.LOWSTATE` | `intentional_safety_divergence` | RansByte `scale_bits > 23` with small freq drains state below LowerBound (C++ identical; Rust documents the operational domain) | `open` |
| `MSRTC.RAW.CORRUPTADVANCE` | `intentional_safety_divergence` | `value < start` on corrupt streams → Rust fails transactionally; C++ asserts (debug) / wraps (release) | `open` |

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
| No fuzz testing | Random inputs not explored | Add differential fuzzing (Phase 8) |
| No cross-platform testing | Only x86_64 Linux tested | Add ARM, macOS verification (Phase 8) |
