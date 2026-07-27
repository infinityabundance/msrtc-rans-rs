# Oracle Contract — msrtc-rans-rs

## Summary

The oracle contract establishes a **reproducible, auditable bridge** between the native Rust `msrtc-rans-rs` implementation and the pinned Microsoft MLVC C++ oracle. It defines what is pinned, what is proved, and how evidence is preserved.

---

## What Is Pinned

| Artifact | Location | Content |
|----------|----------|---------|
| Upstream repository | `oracle/upstream.lock → [upstream]` | `https://github.com/microsoft/mlvc`, commit `0500356a8d6146dd8dc8911022cbeca19675614f`, subdirectory `packages/msrtc_rans` |
| Oracle Docker image | `msrtc-rans-rs-oracle:debian12` | Debian 12, gcc 12.2.0, CMake 3.27.9, Python 3.11.2, x86_64 |
| Oracle CLI binaries | Inside Docker: `/workspace/bin/oracle_cli`, `/workspace/bin/raw_oracle_cli`, `/workspace/bin/decoder_oracle_cli` | Deterministic CLI that reads casefiles from stdin and writes outputs to stdout |
| Reference bitstreams | `oracle/upstream.lock → [reference_bitstreams]` | RansByte hex `0500bd040001a10003000b00` (12 bytes), Rans64 hex `0500a1bd04000000110a002f03000300` (16 bytes) |
| Test fixtures | `oracle/upstream.lock → [test_fixtures]` | SHA-256 hashes of gaussian encoder output and bit estimator output |

---

## What Is Proven

### Layer 1: Oracle Baseline (observed, not receipted)

The C++ oracle builds and runs correctly in the Docker environment:

- **Upstream Python tests:** 7/7 passing
- **Oracle CLI binaries:** Produce deterministic output
- **Reference bitstreams:** Matched to expected hex values

### Layer 2: Raw Encoder Differential (sealed)

| Claim | Evidence |
|-------|----------|
| Rust `RansByteEncoder` output matches C++ | `MSRTC.RAW.ENCODER.DIFFERENTIAL` — 8 cases, all pass |
| Rust `Rans64Encoder` output matches C++ | Same court, both raw and prepared modes |
| Prepared symbols match raw division | True within Rust (self-consistent) and vs oracle |

### Layer 3: Raw Decoder Differential (sealed)

| Claim | Evidence |
|-------|----------|
| Rust decoder matches C++ decoder | `MSRTC.RAW.DECODER.DIFFERENTIAL` — 16 cases, all pass |
| Rust decode of C++-encoded stream matches | Cross-validation both directions |
| Truncated-stream handling matches | Transactional design validated |

### Layer 4: Entropy Coder Differential (sealed)

| Claim | Evidence |
|-------|----------|
| Full entropy encode (PMF + bypass) matches C++ | `MSRTC.ENTROPY.DIFFERENTIAL` — 6 cases, all pass |
| Full entropy decode matches C++ | Same court, both RansByte and Rans64 |
| C++ encode → Rust decode roundtrip matches | Cross-validation |

---

## What Is NOT Proven

- **Exhaustive coverage** — The differential courts cover 30 specific cases, not all possible inputs
- **Multi-platform parity** — Only x86_64 Linux tested
- **Multi-compiler parity** — Only gcc 12.2.0 tested
- **MLVC integration** — The oracle is the `msrtc_rans` sub-package, not the full MLVC framework
- **Long-term stability** — Future C++ compiler versions may change behaviour (especially around shift UB)

---

## How to Rebuild the Oracle Contract

### Prerequisites

- Docker with `--platform linux/amd64` support
- The oracle build context (Dockerfile + C++ harness files)

### Steps

```bash
# 1. Build the Docker oracle image
docker build --tag msrtc-rans-rs-oracle:debian12 \
  --file /path/to/Dockerfile.oracle \
  /path/to/build-context

# 2. Verify the upstream Python tests
docker run --rm msrtc-rans-rs-oracle:debian12

# 3. Run all differential courts (seals receipts)
cargo run -p msrtc-rans-court --bin seal -- --all

# 4. Verify new receipts
ls courts/receipts/
ls courts/transcripts/
ls courts/manifests/
```

### Manual Oracle CLI Invocation

```bash
# Raw encoder test
echo -n -e '\x01\x00\x00\x00\x00\x00\x00\x00\x08\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00\x00\x80\x00\x00\x00' | \
  docker run -i --rm msrtc-rans-rs-oracle:debian12 /workspace/bin/raw_oracle_cli /dev/stdin

# Entropy encoder test
echo -n -e '<binary casefile>' | \
  docker run -i --rm msrtc-rans-rs-oracle:debian12 /workspace/bin/oracle_cli /dev/stdin

# Decoder cross-validation
echo -n -e '<binary casefile>' | \
  docker run -i --rm msrtc-rans-rs-oracle:debian12 /workspace/bin/decoder_oracle_cli /dev/stdin
```

---

## Evidence Chain

```
upstream.lock (pinned commit + fixture hashes)
    │
    ▼
Docker image (build environment + oracle CLI)
    │
    ▼
Casefiles (deterministic test inputs, content-addressed by SHA-256)
    │
    ▼
Differential Court (Rust vs C++ byte comparison)
    │
    ├── Receipt (JSON: court_id, run_id, per-case hashes)
    ├── Transcript (TXT: human-readable case report)
    └── Manifest (JSON: linked receipt + transcript hash)
```

Each receipt contains:
- `court_id` — Unique court identifier
- `run_id` — `{timestamp}_{short_commit}`
- `oracle_commit` — Full SHA-256 of pinned oracle commit
- `rust_commit` — Full SHA-256 of Rust commit (with `-dirty` suffix if applicable)
- `docker_image_digest` — Docker image content hash
- `environment_sha256` — Fingerprint of the build environment
- `cases` — Per-case status, oracle SHA-256, native SHA-256
- `commands` — Exact commands used to produce the receipt

---

## Oracle Pin Properties

| Property | Value |
|----------|-------|
| Deterministic | ✅ Output is byte-identical for identical inputs |
| Reproducible | ✅ Same commit + same Docker → same receipts |
| Auditable | ✅ Receipts contain full environment fingerprint |
| Extensible | ✅ New cases can be added to any court's generator |
| Tamper-evident | ✅ Casefiles are content-addressed by SHA-256 |
