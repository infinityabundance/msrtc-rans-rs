# msrtc-rans-rs — Phase 0–4 Status Report

**Date:** 2026-08-04
**Phase:** 4 — Streams & Allocation Complete
**Upstream oracle commit:** `0500356a8d6146dd8dc8911022cbeca19675614f`
**Workspace version:** 0.3.0
**Sealed at:** `aad4dfce2757` (stream court), `dcfd39e80852` (raw + entropy courts)

---

## Executive Summary

The project delivers a **differential-tested native Rust replacement** for Microsoft MLVC's `msrtc_rans` entropy coder. Four forensic courts are sealed against the pinned C++ oracle:

| Court | Cases | Result | Sealed at |
|-------|------:|--------|-----------|
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` | 8 | ✅ 8/8 | `dcfd39e80852` |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` | 16 | ✅ 16/16 | `dcfd39e80852` |
| `MSRTC.ENTROPY.DIFFERENTIAL` | 6 | ✅ 6/6 | `dcfd39e80852` |
| `MSRTC.STREAM.DIFFERENTIAL` | 24 | ✅ 24/24 | `aad4dfce2757` |

Phase 4 adds the **streams & allocation layer**: persistent `RansEncoderStream` / `RansDecoderStream` (a single raw rANS state across `push()` calls, matching Microsoft's `RawRansEncoderStream`), the `IResizableBuffer` / `HeapResizableBuffer` pattern with Microsoft's growth formula, and a new `MSRTC.STREAM.DIFFERENTIAL` court proving **multipart wire parity** — the Rust multipart stream is byte-identical to Microsoft's, and both sides cross-decode each other's streams.

---

## What Was Implemented in Phase 4

### Streams (`msrtc-rans/src/stream.rs`)

- **`RansEncoderStream<S>`** — Generic persistent encoder stream; `push()` continues one raw rANS encoder state (exactly Microsoft's `RansEncoderStreamImpl`), `flush()` finalizes once and resets for reuse, `reset()` aborts the session.
- **`RansDecoderStream<S>`** — Generic persistent decoder stream; owns the message and keeps a persistent decode cursor `(unit position, state)` across sequential `decode()` calls (Microsoft's `RansDecoderStreamImpl`). First decode initializes the raw decoder; `check_eof()` requires source exhaustion AND `state == LowerBound`.
- **`RansVariant`** — Runtime variant enum (RansByte=1, Rans64=0) for the Python-facing API.
- **Core additions** — `RansDecoder::from_state(source, state)` and `SliceSource::seek(pos)` enable continuation decoding.

### Allocation (`msrtc-rans/src/buffer.rs`)

- **`ResizableBuffer` trait** — `get_buffer`, `begin_to_grow`, `commit`, `rollback` (Microsoft's `IResizableBuffer`).
- **`HeapResizableBuffer`** — Growth formula `new = old + min(old, max_size_step)` with `max_size_step` floored at `MIN_BUFFER_SIZE = 512`; initial size aligned to a multiple of 16; rollback cancels a pending grow.
- **`ResizableBufferSink`** — Safe backward-writing byte sink (`write_u8` / `write_u32`); growth relocates existing content to the END of the enlarged buffer (Microsoft's `newBuffer.last(content.size())`).

### Python Bindings (`msrtc-rans-python`)

- `RansEncoderStream` now wraps the **persistent** Rust encoder stream (no more independent segments — the previous multipart layout was wrong).
- `RansDecoderStream` keeps a persistent cursor; `decodeEOF` enforces full consumption + EOF state.
- All **7 upstream Python tests** pass inside Docker, including `test_encode_decode_multi_part_0` (push two batches → flush → decode in reverse order → decodeEOF) and `test_rans_encoder_stream_0` (256-symbol stream).

### Stream Differential Court (`MSRTC.STREAM.DIFFERENTIAL`)

- New C++ oracle CLI `stream_oracle_cli` exercising Microsoft's persistent `RansEncoderStream` / `RansDecoderStream` over multipart casefiles (encode + decode modes).
- **8 multipart cases** (RansByte + Rans64; 1, 2, and 3 batches; 256-symbol batches; bypass bits 2 and 4) × **3 sub-cases**:
  1. Wire parity — Rust flush bytes vs Microsoft stream bytes
  2. Microsoft stream → Rust persistent decoder (values + EOF)
  3. Rust stream → Microsoft persistent decoder (values + EOF)
- **24/24 sealed** at clean commit `aad4dfce2757`.

---

## What Was Implemented in Phases 0–3 (Summary)

### Phase 0 — Oracle Baseline
- Microsoft MLVC pinned at `0500356a8d6146dd8dc8911022cbeca19675614f`; Docker Debian 12 oracle cell; reference fixtures captured; workspace scaffolded.

### Phase 1 — Raw rANS Engine
- Raw `RansByte` / `Rans64` encoders and decoders, reciprocal preparation, transactional decoder advance, Microsoft-compatible buffer growth. Sealed via `MSRTC.RAW.ENCODER.DIFFERENTIAL` (8/8) and `MSRTC.RAW.DECODER.DIFFERENTIAL` (16/16).

### Phase 3 — Entropy Coder
- `EntropyEncoder` / `EntropyDecoder` with PMF validation, distribution descriptors, CDF construction, `upper_bound` symbol lookup, variable-width bypass coding, value reconstruction. Sealed via `MSRTC.ENTROPY.DIFFERENTIAL` (6/6).

### Phase 6 — Python Drop-in
- PyO3 extension `_msrtc_rans` implementing the `msrtc.rans` Python API; all 7 upstream tests pass; single-message bitstreams byte-match the reference fixtures.

---

## Courts Sealed

| Court ID | Cases | Passes | Residuals | Status |
|----------|-------|--------|-----------|--------|
| `MSRTC.INVENTORY` | — | — | — | 📋 Scaffold |
| `MSRTC.ORACLE.BASELINE` | 7 | 7 | 0 | 🟡 Observed (no formal receipt) |
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` | 8 | 8 | 0 | ✅ Sealed |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` | 16 | 16 | 0 | ✅ Sealed |
| `MSRTC.ENTROPY.DIFFERENTIAL` | 6 | 6 | 0 | ✅ Sealed |
| `MSRTC.STREAM.DIFFERENTIAL` | 24 | 24 | 0 | ✅ Sealed |
| `MSRTC.RAW.RANSBYTE` | — | — | — | 📋 Scaffold |
| `MSRTC.RAW.RANS64` | — | — | — | 📋 Scaffold |
| `MSRTC.RECIPROCAL` | — | — | — | 📋 Scaffold |
| `MSRTC.PMF` | — | — | — | 📋 Scaffold |
| `MSRTC.BYPASS` | — | — | — | 📋 Scaffold |
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
- **Multipart stream (persistent encoder):** ✅ Byte-identical to Microsoft `RansEncoderStream` (8/8 wire cases)
- **Multipart stream decoding:** ✅ Both directions, values + EOF (16/16 decode cases)
- **Cross-validation (C++ encode → Rust decode):** ✅ Matches

---

## Python Compatibility

- **Upstream Python tests:** 7/7 passing against the Rust extension in Docker
- **Bitstream fixtures:** RansByte `0500bd040001a10003000b00` and Rans64 `0500a1bd04000000110a002f03000300` match byte-for-byte
- **Multipart:** persistent stream semantics; wire-parity proven against Microsoft oracle
- **Wheels:** built with maturin; `msrtc.rans` package layered into site-packages

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
- `courts/receipts/` — 4 sealed receipt JSON files
- `courts/transcripts/` — Human-readable court transcripts
- `courts/manifests/` — Receipt + transcript hash-linked manifests
- `courts/residuals/` — 3 active residuals (safety divergences) + 2 resolved residual records
- `docs/generated/surface-inventory.md` — Complete surface enumeration
- `docs/generated/parity-matrix.md` — Current parity status
- `docs/generated/claim-index.md` — Verified and unverified claims
- `docs/generated/court-index.md` — Court registry with statuses
- `dockerfiles/` — `Dockerfile.oracle` (4 oracle CLIs), `Dockerfile.rust`, `Dockerfile.python`
- `oracle/harness/` — `oracle_cli`, `raw_oracle_cli`, `decoder_oracle_cli`, `stream_oracle_cli`
- Docker image `msrtc-rans-rs-oracle:debian12` — Green baseline

---

### Phase 7 — MLVC Integration (PASS)
- Installed the Rust wheel in a dedicated container and ran the **real MLVC code paths** (`conversion/_coder.py` GaussianEncoder/BitEstimator, `src/models/entropy_models.py` AEHelper, `src/utils/stream_helper.py`) against both the C++ `_msrtc_rans` and the Rust wheel.
- **12/12 bitstreams byte-identical** (SHA-256 verified), per-case bpp identical, reconstruction identical, aggregate bits identical (5,626,520).
- Evidence: `integration/mlvc/evidence/`; harness: `integration/mlvc/`.

---

### Phase 8 — Hardening (SEALED)
- **Property sweeps**: RansByte scale-bits 2..=23 and Rans64 2..=31 roundtrips (218 checks), boundary freq patterns, prepared-vs-raw byte equivalence.
- **Corruption robustness**: truncated + bit-flipped streams never panic; failed advances are transactional (state preserved).
- **Entropy sweeps**: roundtrip + stream multipart sweeps (32 checks, both variants, bypass outliers).
- **Allocation-failure injection**: growth overflow is a typed `CapacityOverflow` error, never a panic.
- **Fixes found**: `advance_unchecked` fails transactionally on `value < start` (C++ asserts/wraps); bypass payload decoding rejects `total_bits >= 64` shift overflow (C++ UB).
- **Miri**: clean on core prepared/raw + transactional paths and the entropy corrupt-stream path (no UB in the forbid(unsafe_code) crates).
- **Python FFI**: `Py_buffer` metadata validation before raw casts; exact-size output writes (all 7 upstream tests still pass).
- Sealed as `MSRTC.HARDENING` at `ddea9a07b112` (4/4).

### Phase 9 — Performance (COMPLETE)
- Deterministic release benchmark harness (`msrtc-rans-bench`): raw byte/64 encode+decode, prepared encode, entropy encode+decode (both variants), stream multipart.
- Raw primitives: 185–433 M items/s; entropy: 63–109 M items/s; streams: 79–113 M items/s.
- Optimization: bypass encoding now uses a fixed 40-entry stack buffer instead of a per-value `Vec` — **+19% entropy byte encode, +20% stream encode**; all sealed courts re-verified byte-identical.
- Report: `docs/phase9-report.md`.

### Docker Matrix (PASS)
- Three Rust cells (Ubuntu 24.04, Fedora 40, Alpine 3.20/musl) build and run the full workspace test suite inside dedicated containers — 131 tests pass in each; `cargo fmt --check` clean, clippy ok. No host-side cargo.
- Supporting cells: pinned C++ oracle (7/7 upstream Python tests) and Rust wheel (`msrtc.rans` import, no C++).
- Evidence: `evidence/docker-matrix/DOCKER_MATRIX_EVIDENCE.json`; compose: `compose/matrix.compose.yml`; cells: `dockerfiles/Dockerfile.matrix-{ubuntu,fedora,alpine}`.

---

## Next Phases

| Phase | Scope |
|-------|-------|
| 9 | Performance — profiling, monomorphization, bounds-check elimination, intrinsics |
| — | Docker matrix — Ubuntu/Fedora/Alpine cells; run-scoped names/labels/digests |
| — | Corpus expansion — encoder 8→100+, decoder 16→100+, entropy 6→100+, stream 8→100+ |
