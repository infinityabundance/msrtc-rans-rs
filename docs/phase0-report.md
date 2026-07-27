# msrtc-rans-rs — Phase 3 Final Report

**Date:** 2026-07-27
**Phase:** 3 — Full Entropy Coder with Differential Verification
**Upstream oracle commit:** `0500356a8d6146dd8dc8911022cbeca19675614f`
**Workspace version:** 0.2.0 (→ 0.2.1)
**Sealed at:** `dcfd39e80852`

---

## Executive Summary

Phase 3 delivers the **complete entropy coder** — PMF validation, CDF table construction, bypass coding, and full encode/decode pipeline — with **three sealed differential courts** proving byte-identical output to the Microsoft C++ oracle. The project has transitioned from a raw rANS primitive implementation to a fully differential-tested entropy coding library with 102 test functions (99 active, 3 ignored).

---

## What Was Implemented in Phase 3

### Entropy Coder (`msrtc-rans/src/entropy.rs`)

- **`EntropyEncoder`** — Public struct wrapping encoder state; supports `new()`, `initialize(pmf, symbol_bits, bypass_bits)`, and `encode(values) -> Vec<u8>`
- **`EntropyDecoder`** — Public struct wrapping decoder state; supports `new()`, `initialize(pmf, symbol_bits, bypass_bits)`, and `decode(bytes) -> Vec<i32>`
- **PMF Validation** — Rejects empty lengths lists, mismatched dimensions, invalid scale_bits, zero frequencies, scale_bits > 31, bypass_bits > 31
- **Distribution Descriptors** — Per-distribution metadata for value offset, bypass sentinel, and symbol offset
- **CDF Table Construction** — Cumulative distribution function derived from PMF frequencies
- **Bypass Coding** — Variable-width bypass encoding for out-of-range values; supports both u8 (RansByte) and u32 (Rans64) bypass payloads with configurable bypass bits
- **Encode Path** — Value→distribution→symbol→raw rANS encode, with bypass sentinel for out-of-range values
- **Decode Path** — Raw rANS decode→CDF symbol recovery→bypass decode→value reconstruction
- **Cross-Variant Support** — Both RansByte and Rans64 via `EncoderVariantForS` and `RawEncoder` traits
- **Error Handling** — `EntropyError` enum with `InvalidPmf`, `InvalidParams`, `InvalidState`, `InvalidStream`, `RawRansError` variants

### Entropy Tests

- **30 test functions** covering:
  - Encoder/decoder initialization (both variants)
  - PMF validation (rejects invalid inputs)
  - Reference bitstream match (encode matches oracle hex)
  - In-range value encoding/decoding (round-trip)
  - Bypass encoding/decoding (various bypass bits: 2, 3, 8)
  - Mixed in-range + bypass values
  - Large positive and extreme negative outlier bypass
  - Multiple bypasses per stream
  - `symbol_bits=32` rejection safety check
  - `bypass_bits=32` rejection safety check
  - Misaligned stream rejection (1, 2, 3 extra bytes resistance)

### Differential Courts

Three differential courts were built and sealed:

| Court ID | Cases | Passes | Result | What It Proves |
|----------|-------|--------|--------|----------------|
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` | 8 | 8 | ✅ Sealed | Raw RansByte/Rans64 encoder matches C++ oracle |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` | 16 | 16 | ✅ Sealed | Raw decoder matches C++ (both directions) |
| `MSRTC.ENTROPY.DIFFERENTIAL` | 6 | 6 | ✅ Sealed | Full entropy coder matches C++ (encode, decode, cross-validate) |
| **Total** | **30** | **30** | **✅ All pass** | |

### Oracle Infrastructure

- **`oracle_cli`** — Entropy coder oracle binary (reads casefiles from stdin, writes hex output)
- **`raw_oracle_cli`** — Raw encoder oracle binary
- **`decoder_oracle_cli`** — Decoder oracle binary
- **Receipt system** — JSON receipts, human-readable transcripts, manifests with hash linking
- **Residual persistence** — Structured mismatch records written to `courts/residuals/`

---

## Courts Sealed

| Court ID | Cases | Passes | Residuals | Status |
|----------|-------|--------|-----------|--------|
| `MSRTC.INVENTORY` | — | — | — | 📋 Scaffold |
| `MSRTC.ORACLE.BASELINE` | 7 | 7 | 0 | 🟡 Observed (no formal receipt) |
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` | 8 | 8 | 0 | ✅ Sealed |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` | 16 | 16 | 0 | ✅ Sealed |
| `MSRTC.ENTROPY.DIFFERENTIAL` | 6 | 6 | 0 | ✅ Sealed |
| `MSRTC.RAW.RANSBYTE` | — | — | — | 📋 Scaffold |
| `MSRTC.RAW.RANS64` | — | — | — | 📋 Scaffold |
| `MSRTC.RECIPROCAL` | — | — | — | 📋 Scaffold |
| `MSRTC.PMF` | — | — | — | 📋 Scaffold |
| `MSRTC.BYPASS` | — | — | — | 📋 Scaffold |
| `MSRTC.STREAM` | — | — | — | 📋 Scaffold |
| `MSRTC.BUFFER` | — | — | — | 📋 Scaffold |
| `MSRTC.CROSS` | — | — | — | 📋 Scaffold |
| `MSRTC.INVALID` | — | — | — | 📋 Scaffold |
| `MSRTC.PYTHON.API` | — | — | — | 📋 Scaffold |
| `MSRTC.PLATFORM` | — | — | — | 📋 Scaffold |

---

## Byte Parity

- **RansByte raw encoder:** ✅ Byte-identical to C++ oracle (8/8 cases)
- **Rans64 raw encoder:** ✅ Byte-identical to C++ oracle (8/8 cases)
- **RansByte raw decoder:** ✅ Byte-identical (Rust↔C++ both directions, 16/16 cases)
- **Rans64 raw decoder:** ✅ Byte-identical (Rust↔C++ both directions, 16/16 cases)
- **EntropyEncoder (PMF + bypass):** ✅ Byte-identical to C++ oracle (6/6 cases)
- **EntropyDecoder (CDF + bypass):** ✅ Byte-identical to C++ oracle (6/6 cases)
- **Cross-validation (C++ encode → Rust decode):** ✅ Matches

---

## Python Compatibility

- **Upstream Python tests:** 7/7 passing against C++ oracle in Docker
- **Rust extension:** Scaffold only (PyO3 module with no API yet)
- **Wheels:** Not yet built

---

## Open Residuals

| ID | Classification | Description | Status |
|----|---------------|-------------|--------|
| `MSRTC.RAW.SCALE32` | `intentional_safety_divergence` | `scale_bits=32` causes UB in C++; Rust rejects | `open` |
| `MSRTC.RAW.SYMBOLBITS32` | `intentional_safety_divergence` | `symbol_bits=32` causes UB in C++; Rust rejects | `open` |
| `MSRTC.RAW.BYPASSBITS32` | `intentional_safety_divergence` | `bypass_bits=32` causes UB in C++; Rust rejects | `open` |

All three residuals are **intentional safety divergences** — Rust is correct to reject these inputs; C++ has undefined behavior.

---

## Evidence

- `oracle/upstream.lock` — Upstream pin and fixture hashes
- `courts/receipts/` — 3 sealed receipt JSON files
- `courts/transcripts/` — Human-readable court transcripts
- `courts/manifests/` — Receipt + transcript hash-linked manifests
- `courts/residuals/` — 3 active residuals (safety divergences) + 2 stale resolved residuals
- `docs/generated/surface-inventory.md` — Complete surface enumeration
- `docs/generated/parity-matrix.md` — Current parity status
- `docs/generated/claim-index.md` — Verified and unverified claims
- `docs/generated/court-index.md` — Court registry with statuses
- Docker image `msrtc-rans-rs-oracle:debian12` — Green baseline

---

## Exact Commands Run (Phase 3 Sealing)

```bash
# Seal raw encoder differential court
cargo run -p msrtc-rans-court --bin seal -- --encoder

# Seal raw decoder differential court
cargo run -p msrtc-rans-court --bin seal -- --decoder

# Seal entropy differential court  
cargo run -p msrtc-rans-court --bin seal -- --entropy

# Verify all tests pass
cargo test --workspace --exclude msrtc-rans-python
# 99 tests passed (30 core + 30 entropy + 39 court), 3 ignored

# Lint check
cargo clippy --workspace --exclude msrtc-rans-python --all-targets -- -D warnings
cargo fmt --check
```

---

## All Workspace Tests

| Crate | Test Count | Status |
|-------|-----------|--------|
| `msrtc-rans-core` | 30 | ✅ All pass |
| `msrtc-rans` (entropy) | 30 + 1 doc (ignored) | ✅ All pass |
| `msrtc-rans-court` | 42 (39 pass, 3 ignored) | ✅ 39 pass, 3 ignored |
| **Total** | **102 defined (99 active, 3 ignored)** | **✅ All active pass** |

---

## Claims Intentionally Not Made

- "Byte-identical to Microsoft oracle for all inputs" — Differential court covers 30 specific cases, not exhaustive
- "Drop-in replacement" — Python API not implemented; MLVC integration not tested
- "Works with MLVC" — MLVC integration not tested
- "Performance competitive" — Benchmarks not run
- "Memory-safe replacement" — Formal claim pending full memory safety audit

---

## Next Actions

1. **Phase 4** — Streaming buffer management and multi-buffer support
2. **Phase 5** — Python extension implementation via PyO3
3. **Phase 6** — Docker matrix (Ubuntu, Fedora, Alpine oracle cells)
4. **Phase 7** — MLVC integration harness and bitstream compatibility
5. **Phase 8** — Differential fuzzing for random-input coverage
6. **Phase 9** — Performance benchmarking and optimization
7. **Tooling** — Implement `xtask gen` for reproducible docs; add CI workflow
