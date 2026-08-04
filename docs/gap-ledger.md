# msrtc-rans-rs Gap Ledger
*Updated: 2026-08-04* — **Phase 4 — Four Courts Sealed**

## Overview

This document tracks known gaps between the current implementation and a complete, production-ready forensic-parity rANS entropy coder. Gaps are organized by category with impact assessment and suggested next actions.

---

## Docker Matrix

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| Only Debian 12 oracle cell exists | Medium | Oracle parity not verified on other Linux distributions | Add Ubuntu 22.04, Fedora 39, Alpine 3.19 Dockerfiles |
| No multi-compiler testing | Medium | GCC-only; MSVC/clang paths not tested | Add clang and MSVC oracle builds |
| No CI integration for oracle rebuild | High | Manual process; not reproducible by CI | Add GitHub Actions workflow for oracle Docker build |
| Rust courts still run host-side | Medium | Addendum requires Rust testing inside dedicated containers | Move court/test execution into the Docker matrix |
| Run-scoped naming not fully wired into receipts | Low | Receipts record abbreviated commands | Record per-case container names, labels, image digests in transcript |

---

## Court Coverage

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| `MSRTC.RAW.RANSBYTE` specific court | Low | Covered by RAW.ENCODER.DIFFERENTIAL | Scaffold exists |
| `MSRTC.RAW.RANS64` specific court | Low | Covered by RAW.ENCODER.DIFFERENTIAL | Scaffold exists |
| `MSRTC.RECIPROCAL` three-way court | Low | Arithmetic tested in unit tests | Scaffold exists |
| `MSRTC.PMF` court | Medium | PMF validation only tested in unit tests | Implement differential PMF court |
| `MSRTC.BYPASS` court | Low | Bypass covered by ENTROPY.DIFFERENTIAL | Scaffold exists |
| `MSRTC.STREAM` streaming court | ✅ **Resolved** | `MSRTC.STREAM.DIFFERENTIAL` sealed 24/24 | Complete |
| `MSRTC.BUFFER` buffer management court | ✅ **Resolved** | `IResizableBuffer`/`HeapResizableBuffer` implemented + unit-tested; growth formula matches Microsoft | Complete |
| `MSRTC.INVALID` invalid input court | Medium | Error handling not differentially tested | Scaffold exists |
| `MSRTC.PLATFORM` cross-platform court | Medium | Only x86_64 Linux tested | Scaffold exists |
| `MSRTC.CROSS` cross-variant court | Low | Both variants covered by all four sealed courts | Scaffold exists |

---

## Decoder Court

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| Decoder court needs dedicated oracle binary | ✅ **Resolved** | `decoder_oracle_cli` implemented | Complete |

---

## MLVC Integration

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| No MLVC integration test | ✅ **Resolved** | **Phase 7 PASS** — Rust wheel byte-identical to C++ `_msrtc_rans` in real MLVC coder paths (12/12 cases, identical bpp) | Complete |
| Full FrameLoop with trained weights + YUV | Medium | Exercises the same primitives; needs model checkpoints | Run when checkpoints are available |
| `.mlvc` decode-only mode | Medium | Bitstream container format | Differential test with real `.mlvc` files |
| No MLVC-compatible C API | Medium | MLVC needs C calling convention | Add `extern "C"` API to msrtc-rans if MLVC requires it |

---

## Python Extension

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| PyO3 module is scaffold only | ✅ **Resolved** | Full `msrtc.rans` API implemented | Complete |
| No Python type stubs | ✅ **Resolved** | `_msrtc_rans.pyi` present | Complete |
| No wheel builds | ✅ **Resolved** | maturin wheel builds; tested in Docker | Complete |
| No Python tests | ✅ **Resolved** | All 7 upstream tests pass | Complete |
| `Py_buffer` metadata validation (ndim/format/itemsize/alignment/exact capacity) | ✅ **Resolved** | Phase 8: FFI helpers now validate all buffer metadata; exact-size output writes | Complete |
| Wheel not a standalone `msrtc.rans` distribution | Medium | Package files layered into site-packages post-install | maturin mixed-project layout packaging |

---

## Documentation & Tooling

| Gap | Priority | Impact | Next Action |
|-----|----------|--------|-------------|
| `xtask gen` not implemented | Medium | Docs not regenerable | Implement `xtask gen` command |
| No API reference docs | Low | Developer experience | Add rustdoc examples |
| No fuzz testing | ✅ **Resolved** | Deterministic LCG sweeps + corruption battery sealed in `MSRTC.HARDENING`; Miri clean on core/entropy paths | Complete |
| No benchmark results | Low | Performance unknown | Run benchmark harness (Phase 9) |
| No formal verification | Low | Mathematical proof of correctness | Explore using Kani or Proptest (follow-up) |

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
