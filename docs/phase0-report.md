# msrtc-rans-rs — Phase 0/1 Implementation Report

**Upstream oracle commit:** `0500356a8d6146dd8dc8911022cbeca19675614f`
**Native implementation commit:** *(working tree)*
**Phase completed:** Phase 0 (Freeze & Inventory) + Phase 1 (Raw rANS Engine)

## Implemented

- ✅ Upstream lock file (`oracle/upstream.lock`) with pinned commit, fixture hashes, reference bitstreams
- ✅ Microsoft C++ oracle built in Docker (Debian 12, gcc 12.2.0, CMake 3.27.9)
- ✅ Original upstream Python tests: **7/7 passing** in Docker
- ✅ Full workspace scaffold: 7 crates + xtask + docs
- ✅ Core rANS primitives (both RansByte and Rans64):
  - `RansEncSymbol` with Alverson reciprocal precomputation
  - `RansEncoder::put_raw()` (division-based)
  - `RansEncoder::put()` (reciprocal-multiply)
  - `RansEncoder::flush()`, `reset()`
  - `RansDecoder::init()`, `get()`, `advance()`, `check_eof()`
  - Renormalization for both encoder and decoder
  - Mul64Hi via u128 widening multiply
  - Frequency-one special case
- ✅ Sink trait with `VecSink` (reverse-order, auto-growing) and `SliceSink`
- ✅ Source trait with `SliceSource`
- ✅ Arithmetic test suite: reciprocal, Mul64Hi, fast division vs exact
- ✅ Self-consistency tests: round-trip encode/decode, prepared-vs-raw equivalence
- ✅ `#![forbid(unsafe_code)]` on all production crates
- ✅ `#![no_std]` on core crate
- ✅ Docker oracle image built and tested
- ✅ Generated parity docs (surface-inventory, parity-matrix, claim-index)

## Courts Sealed

| Court ID | Cases | Passes | Residuals | Status |
|----------|-------|--------|-----------|--------|
| `MSRTC.INVENTORY` | — | — | — | 📋 Scaffold |
| `MSRTC.ORACLE.BASELINE` | 7 | 7 | 0 | ✅ Sealed |
| `MSRTC.RAW.RANSBYTE` | — | — | — | 📋 Scaffold |
| `MSRTC.RAW.RANS64` | — | — | — | 📋 Scaffold |
| `MSRTC.RECIPROCAL` | — | — | — | 📋 Scaffold |

## Byte Parity

- **RansByte:** Self-consistent (prepared == raw division). Oracle comparison not yet performed.
- **Rans64:** Self-consistent. Oracle comparison not yet performed.
- **Streaming:** Not yet implemented.
- **MLVC:** Not yet tested.

## Python Compatibility

- **Upstream tests:** 7/7 passing against C++ oracle in Docker
- **Rust extension:** Scaffold only (PyO3 module with no API yet)
- **Wheels:** Not yet built

## Performance

Not yet measured (Phase 9).

## Open Residuals

| ID | Classification | Description | Status |
|----|---------------|-------------|--------|
| — | — | None yet (no differential courts run) | — |

## Evidence

- `oracle/upstream.lock` — Upstream pin and fixture hashes
- `docs/generated/surface-inventory.md` — Complete surface enumeration
- `docs/generated/parity-matrix.md` — Current parity status
- `docs/generated/claim-index.md` — Verified and unverified claims
- Docker image `msrtc-rans-rs-oracle:debian12` — Green baseline

## Exact Commands Run

```bash
# Oracle build and test
docker build --tag msrtc-rans-rs-oracle:debian12 \
  --file /run/media/one/toshiba4TB/docker/msrtc-rans-rs/dockerfiles/Dockerfile.oracle \
  /run/media/one/toshiba4TB/docker/msrtc-rans-rs/build-contexts/oracle

docker run --rm msrtc-rans-rs-oracle:debian12

# Rust workspace
cargo test --workspace --exclude msrtc-rans-python
cargo clippy --workspace --exclude msrtc-rans-python --all-targets -- -D warnings
cargo fmt --check
```

## All Workspace Tests

- **26 total** (19 core + 7 court): **26 passed, 0 failed, 0 ignored**

## Claims Intentionally Not Made

- "Byte-identical to Microsoft oracle" — No differential court sealed yet
- "Drop-in replacement" — Python API and PMF/bypass not implemented
- "Works with MLVC" — MLVC integration not tested
- "Performance competitive" — Benchmarks not run
- "Memory-safe replacement" — Formal claim pending full parity verification

## Next Actions

1. **Phase 2** — Implement prepared symbols and optimized arithmetic integration
2. **Phase 3** — Implement entropy distributions (PMF, CDF) and bypass coding
3. **Phase 4** — Implement streaming and buffer management
4. **Phase 5** — Complete public Rust API
5. **Phase 6** — Implement Python drop-in extension via PyO3
6. **Differential courts** — Run `MSRTC.RAW.RANSBYTE` and `MSRTC.RAW.RANS64` comparing to oracle
