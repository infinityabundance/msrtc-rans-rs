# msrtc-rans-rs Gap Ledger
*Updated: 2026-07-27* — **Phase 3 — Three Courts Sealed**

## Overview

This document tracks known gaps between the current implementation and a complete, production-ready forensic-parity rANS entropy coder. Gaps are organized by category with impact assessment and suggested next actions.

---

## Docker Matrix

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| Only Debian 12 oracle cell exists | Medium | Oracle parity not verified on other Linux distributions | Add Ubuntu 22.04, Fedora 39, Alpine 3.19 Dockerfiles |
| No multi-compiler testing | Medium | GCC-only; MSVC/clang paths not tested | Add clang and MSVC oracle builds |
| No CI integration for oracle rebuild | High | Manual process; not reproducible by CI | Add GitHub Actions workflow for oracle Docker build |
| No run-scoped Docker naming | Low | Docker resource identification cumbersome | Add run IDs, labels, and cleanup scripts |

---

## Court Coverage

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| `MSRTC.RAW.RANSBYTE` specific court | Low | Covered by RAW.ENCODER.DIFFERENTIAL | Scaffold exists |
| `MSRTC.RAW.RANS64` specific court | Low | Covered by RAW.ENCODER.DIFFERENTIAL | Scaffold exists |
| `MSRTC.RECIPROCAL` three-way court | Low | Arithmetic tested in unit tests | Scaffold exists |
| `MSRTC.PMF` court | Medium | PMF validation only tested in unit tests | Implement differential PMF court |
| `MSRTC.BYPASS` court | Low | Bypass covered by ENTROPY.DIFFERENTIAL | Scaffold exists |
| `MSRTC.STREAM` streaming court | Low | Streaming not implemented | Scaffold exists |
| `MSRTC.BUFFER` buffer management court | Low | VecSink growth formula not cross-checked | Scaffold exists |
| `MSRTC.INVALID` invalid input court | Medium | Error handling not differntially tested | Scaffold exists |
| `MSRTC.PLATFORM` cross-platform court | Medium | Only x86_64 Linux tested | Scaffold exists |
| `MSRTC.CROSS` cross-variant court | Low | Cross-variant encoding not tested | Scaffold exists |

---

## Decoder Court

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| Decoder court needs dedicated oracle binary | ✅ **Resolved** | `decoder_oracle_cli` implemented | Complete |

---

## MLVC Integration

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| No MLVC integration test | High | Cannot claim drop-in replacement | Build MLVC test harness |
| No MLVC-compatible C API | Medium | MLVC needs C calling convention | Add `extern "C"` API to msrtc-rans |
| No MLVC bitstream compatibility check | Medium | Bitstream format differences unknown | Differential test with MLVC bitstreams |

---

## Python Extension

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| PyO3 module is scaffold only | High | No Python API available | Implement Python bindings |
| No Python type stubs | Medium | Poor IDE experience | Add `.pyi` stub files |
| No wheel builds | Medium | pip install not working | Add `maturin` build config |
| No Python tests | High | No coverage for Python path | Port upstream Python tests |

---

## Documentation & Tooling

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| `xtask gen` not implemented | Medium | Docs not regenerable | Implement `xtask gen` command |
| No API reference docs | Low | Developer experience | Add rustdoc examples |
| No fuzz testing | Medium | Random inputs untested | Add `cargo fuzz` harness |
| No benchmark results | Low | Performance unknown | Run benchmark harness |
| No formal verification | Low | Mathematical proof of correctness | Explore using Kani or Proptest |

---

## Receipt & Evidence Infrastructure

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| No receipt regeneration script | Medium | Receipts not reproducible from clean state | Add `xtask seal --all` |
| No receipt hash chain | Low | Receipts not linked | Implement receipt chaining |
| No receipt freshness check | Low | Stale receipts not detected | Add `xtask check` |

---

## Legend

- ✅ **Resolved** — Gap has been addressed
- High — Blocks a major milestone (e.g., drop-in replacement claim)
- Medium — Important for production readiness
- Low — Nice to have
