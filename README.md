# msrtc-rans-rs

**Native Rust forensic-parity replacement for Microsoft MLVC's `msrtc_rans` entropy coder.**

A fully differential-tested, `#![forbid(unsafe_code)]`, `#![no_std]`-compatible Rust implementation of the rANS entropy coder used by Microsoft's MLVC framework. Every encoder/decoder primitive, entropy distribution, bypass path, and **persistent stream** is verified byte-for-byte against the pinned C++ oracle in sealed forensic courts.

---

## Current Status — Phase 4 Complete ✅

| Component | Status |
|-----------|--------|
| Upstream oracle source pin | ✅ **Sealed** — commit `0500356a` |
| Oracle baseline observation | ✅ **Sealed** — 7/7 Python tests pass |
| Docker Debian oracle cell | ✅ **Sealed** — Docker image with oracle CLIs |
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` court | ✅ **Sealed** — 8/8 cases pass |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` court | ✅ **Sealed** — 16/16 cases pass |
| `MSRTC.ENTROPY.DIFFERENTIAL` court | ✅ **Sealed** — 6/6 cases pass |
| `MSRTC.STREAM.DIFFERENTIAL` court | ✅ **Sealed** — 24/24 cases pass |
| `IResizableBuffer` / `HeapResizableBuffer` | ✅ **Implemented** — Microsoft growth formula, rollback |
| Persistent `RansEncoderStream` / `RansDecoderStream` | ✅ **Sealed** — multipart wire-parity with Microsoft |
| Python multipart streaming | ✅ **Working** — persistent stream; all 7 upstream tests pass |
| `scale_bits == 32` safety rejection | ✅ **Residual** — intentional safety divergence |
| `symbol_bits == 32` safety rejection | ✅ **Residual** — intentional safety divergence |
| `bypass_bits == 32` safety rejection | ✅ **Residual** — intentional safety divergence |
| Docker matrix (multi-distro) | 🔲 Pending — Debian only |
| MLVC integration | 🔲 Phase 7 — next |
| Performance benchmarking | 🔲 Phase 9 |

### Test Suite

| Metric | Count |
|--------|-------|
| Total test functions defined | 123 |
| Active tests | 119 |
| Ignored tests (Docker-gated) | 4 |
| Differential court cases (sealed) | 54 |
| Receipts sealed | 4 |

---

## Architecture

```
                    ┌──────────────────────┐
                    │  msrtc-rans-python    │  (PyO3 extension)
                    │  Python API surface   │
                    │  persistent streams   │
                    └──────────┬───────────┘
                               │
                    ┌──────────▼───────────┐
                    │    msrtc-rans         │  (Safe public API)
                    │  EntropyEncoder/Decr  │
                    │  PMF, Bypass, CDF     │
                    │  Streams + Buffer     │
                    └──────────┬───────────┘
                               │
                    ┌──────────▼───────────┐
                    │  msrtc-rans-core      │  (no_std deterministic)
                    │  RansByte / Rans64    │
                    │  Arithmetic / Sink    │
                    └──────────────────────┘
```

### Crate Descriptions

| Crate | Description |
|-------|-------------|
| `msrtc-rans-core` | Deterministic `no_std` rANS primitives (RansByte, Rans64, arithmetic, VecSink, `from_state`, `seek`) |
| `msrtc-rans` | Safe public Rust entropy-coder API: EntropyEncoder/Decoder, PMF, bypass, CDF, **buffer** (IResizableBuffer/HeapResizableBuffer), **stream** (persistent RansEncoderStream/RansDecoderStream) |
| `msrtc-rans-python` | Python extension (`_msrtc_rans`) via PyO3 — persistent multipart streams |
| `msrtc-rans-oracle` | Developer-only C++ oracle adapter — *not published on crates.io* |
| `msrtc-rans-casefile` | Deterministic casefile/residual formats |
| `msrtc-rans-court` | Differential forensic courts with seal/receipt/transcript infrastructure |
| `msrtc-rans-bench` | Matched Rust/C++ benchmark harness |
| `xtask` | Build orchestration and freshness checks — *not published* |

---

## Quick Start

```bash
# Build everything except the Python extension
cargo build --workspace --exclude msrtc-rans-python

# Run all tests (core + entropy + streams + courts)
cargo test --workspace --exclude msrtc-rans-python

# Run a sealed court (requires Docker oracle image)
cargo run -p msrtc-rans-court --bin seal -- encoder
cargo run -p msrtc-rans-court --bin seal -- decoder
cargo run -p msrtc-rans-court --bin seal -- entropy
cargo run -p msrtc-rans-court --bin seal -- stream
```

### Docker Oracle

```bash
# Build the C++ oracle image (Debian 12, gcc 12.2.0) — four CLIs:
# oracle_cli, raw_oracle_cli, decoder_oracle_cli, stream_oracle_cli
docker build --tag msrtc-rans-rs-oracle:debian12 \
  --file dockerfiles/Dockerfile.oracle /path/to/build-context

# Run the upstream Python tests
docker run --rm msrtc-rans-rs-oracle:debian12
```

---

## Oracle Pin

The Microsoft C++ oracle is pinned at:

- **Repository:** https://github.com/microsoft/mlvc
- **Commit:** `0500356a8d6146dd8dc8911022cbeca19675614f`
- **Subdirectory:** `packages/msrtc_rans`
- **License:** MIT

See `oracle/upstream.lock` for full fixture hashes, reference bitstreams, and build environment metadata. The oracle contract proves that the Rust implementation produces byte-identical output to the C++ implementation for all tested code paths.

---

## Published Crates (crates.io)

| Crate | Version | Documentation | Description |
|-------|---------|---------------|-------------|
| [`msrtc-rans-core`](https://crates.io/crates/msrtc-rans-core) | 0.3.0 | [docs.rs](https://docs.rs/msrtc-rans-core) | Deterministic no_std rANS primitives |
| [`msrtc-rans`](https://crates.io/crates/msrtc-rans) | 0.3.0 | [docs.rs](https://docs.rs/msrtc-rans) | Safe public Rust entropy-coder API + streams/buffer |
| [`msrtc-rans-casefile`](https://crates.io/crates/msrtc-rans-casefile) | 0.3.0 | [docs.rs](https://docs.rs/msrtc-rans-casefile) | Casefile/residual formats |
| [`msrtc-rans-court`](https://crates.io/crates/msrtc-rans-court) | 0.3.0 | [docs.rs](https://docs.rs/msrtc-rans-court) | Differential forensic courts |
| [`msrtc-rans-bench`](https://crates.io/crates/msrtc-rans-bench) | 0.3.0 | [docs.rs](https://docs.rs/msrtc-rans-bench) | Benchmark harness |

---

## Forensic Courts

The project uses a novel **forensic court system** to prove correctness:

1. **Casefiles** — Deterministic test inputs with SHA-256 content hashes
2. **Oracle CLIs** — C++ binaries that process casefiles and produce canonical outputs
3. **Differential comparison** — Rust output vs. C++ output, byte-for-byte
4. **Receipts** — Machine-verified evidence packages (court_id, run_id, case results, environment fingerprint)
5. **Residuals** — Structured mismatch records with classification and resolution tracking

### Sealed Courts

| Court ID | Cases | Result |
|----------|-------|--------|
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` | 8 | ✅ All pass |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` | 16 | ✅ All pass |
| `MSRTC.ENTROPY.DIFFERENTIAL` | 6 | ✅ All pass |
| `MSRTC.STREAM.DIFFERENTIAL` | 24 | ✅ All pass |

---

## License

MIT — see [LICENSE](LICENSE) and [NOTICE](NOTICE) for attribute notices.

**Author:** Riaan de Beer — [github.com/infinityabundance](https://github.com/infinityabundance) — rdebeer.infinityabundance@gmail.com
