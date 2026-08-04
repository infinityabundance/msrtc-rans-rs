# Phase 7 — MLVC Integration

**Status: PASS — the Rust wheel is a verified drop-in for the C++ `_msrtc_rans` inside the real MLVC code paths.**

## What was proven

The same deterministic MLVC integration harness ran twice — once backed by the **pinned Microsoft C++ `_msrtc_rans`** and once by the **Rust `msrtc-rans-rs` wheel** — inside two dedicated Debian containers. The runs are **byte-identical**:

| Metric | C++ backend | Rust backend | Match |
|--------|------------|--------------|-------|
| Cases | 12 | 12 | — |
| Bitstream SHA-256 per case | — | — | ✅ identical (12/12) |
| Bitstream length per case | — | — | ✅ identical |
| bpp per case (`8·len/(W·H)`) | — | — | ✅ identical |
| Reconstruction (encode→decode→decodeEOF) | all pass | all pass | ✅ identical |
| Aggregate bits | 5,626,520 | 5,626,520 | ✅ identical |

Evidence: `evidence/` — `MLVC_INTEGRATION_EVIDENCE.json`, per-backend `report.json`, all 24 preserved bitstreams (12 per backend), and the byte-level `mlvc_comparison.json`.

## The real MLVC code exercised

The harness imports and drives the **actual pinned MLVC source** (commit `0500356a`), not a reimplementation:

- **`video/conversion/_coder.py`** — `GaussianEncoder.encode_y`/`decode_y` (both `index_space` modes) and `BitEstimator.encode_z`/`decode_z`, each calling `EntropyEncoder.push` / `EntropyDecoder.decode` with `symbolBits=16, bypassBits=2`.
- **`video/conversion/types.py`** — `GaussianCoderPmf` / `BitEstimatorPmf` dataclasses.
- **`video/src/models/entropy_models.py`** — the torch `AEHelper` path: `GaussianEncoder.build_pmf()` (deterministic, no weights), `_encode`/`_decode` over torch tensors with in-place numpy decode.
- **`video/src/utils/stream_helper.py`** — `open_encoder_streams`, `flush_encoder_streams`, `open_decoder_streams`, `check_decoder_eof`, including the multi-stream tuple branch.

PMF tables are generated with MLVC's own `quantize_pmf` (entropy_models.py) converted to the frequency-table layout the entropy coder requires (every entry ≥ 1, sum = 2^16) — the same layout as the sealed upstream fixtures, verified empirically against the C++ oracle.

bpp accounting mirrors `video/conversion/_frame_loop.py`: `bpp = bitstream_size_bits / (image_width * image_height)` with payload bits `8 * len(stream)`.

## Reproduce

```bash
# Build contexts live under the project Docker volume:
#   build-contexts/mlvc/{mlvc-upstream,harness,source}

# C++ backend (upstream module built from the pinned MLVC commit)
docker build --tag msrtc-rans-rs-mlvc-cpp:debian12 \
  -f integration/mlvc/Dockerfile.mlvc-cpp /path/to/build-context
docker run --rm -v $PWD/evidence/cpp:/out msrtc-rans-rs-mlvc-cpp:debian12

# Rust backend (msrtc-rans-rs wheel built in-image)
docker build --tag msrtc-rans-rs-mlvc-rust:debian12 \
  -f integration/mlvc/Dockerfile.mlvc-rust /path/to/build-context
docker run --rm -v $PWD/evidence/rust:/out msrtc-rans-rs-mlvc-rust:debian12

# Byte-level comparison (also inside a container)
docker run --rm -v $PWD/evidence/cpp:/cpp -v $PWD/evidence/rust:/rust \
  msrtc-rans-rs-mlvc-rust:debian12 \
  python /workspace/harness/compare.py --cpp /cpp --rust /rust
```

## Scope and honesty

**Proved:** the four real MLVC `msrtc.rans` call sites produce byte-identical bitstreams and identical bpp/reconstruction when backed by C++ vs Rust.

**Not covered (requires model checkpoints):** the full `FrameLoop` with trained weights and YUV video, `.mlvc` decode-only mode, and the rate-control loop. Those exercise the same `msrtc.rans` primitives proven here; they are future work once checkpoints are available.

## Files

- `mlvc_harness.py` — deterministic harness (both backends)
- `compare.py` — byte-for-byte comparison
- `Dockerfile.mlvc-cpp` / `Dockerfile.mlvc-rust` — backend images
- `evidence/` — sealed run artifacts (reports, bitstreams, comparison, evidence record)
