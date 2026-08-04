# msrtc-rans-rs Architecture

## Core/Shell Separation

```
                    ┌─────────────────────────┐
                    │   msrtc-rans-python      │  (PyO3 shell)
                    │   Python API surface     │
                    └──────────┬──────────────┘
                               │
                    ┌──────────▼──────────────┐
                    │     msrtc-rans           │  (Safe public API)
                    │  EntropyEncoder          │
                    │  EntropyDecoder          │
                    │  PMF / Bypass / CDF      │
                    └──────────┬──────────────┘
                               │
                    ┌──────────▼──────────────┐
                    │  msrtc-rans-core         │  (no_std deterministic)
                    │  RansByte / Rans64       │
                    │  Arithmetic / Sink/Src   │
                    └─────────────────────────┘
```

The architecture is layered in three tiers:

1. **Raw Primitives** (`msrtc-rans-core`) — Macro-generated rANS encoder/decoder for both `RansByte` (u32 state, u8 unit) and `Rans64` (u64 state, u32 unit). All arithmetic (reciprocal preparation, Mul64Hi, fast division) is structurally faithful to the C++ templates. This crate is `#![no_std]` and `#![forbid(unsafe_code)]`.

2. **Public API** (`msrtc-rans`) — Wraps the core primitives in a safe, high-level entropy coder API. Provides `EntropyEncoder` and `EntropyDecoder` with PMF validation, CDF table construction, and bypass coding for out-of-range values. Re-exports everything from `msrtc-rans-core`.

3. **Entropy Coder** (within `msrtc-rans/src/entropy.rs`) — Implements the full PMF→CDF pipeline, bypass encoding/decoding, distribution descriptors, and error handling. Supports both RansByte and Rans64 variants through the `EncoderVariantForS` and `RawEncoder` traits.

## Variant Architecture

| Variant | State Type | Unit Type | STATE_BITS | LOWER_BOUND | UNITS_PER_STATE |
|---------|-----------|-----------|------------|-------------|-----------------|
| RansByte | `u32` | `u8` | 31 | 1 << 23 | 4 |
| Rans64 | `u64` | `u32` | 63 | 1 << 31 | 2 |

Both variants are generated from a single `generate_rans_impl!` macro in `raw.rs`, matching the C++ template pattern.

## Entropy Layer

The entropy layer adds distribution-aware coding on top of the raw rANS primitives:

- **PMF Validation** — All probability mass functions are validated for: non-empty lengths list, non-empty offsets list, proper table dimension, scale_bits in range [2, 31], every frequency > 0
- **CDF Table Construction** — Cumulative distribution function derived from PMF table, used by decoder for symbol recovery
- **Distribution Descriptors** — Per-distribution metadata: value_offset, bypass_sentinel, symbol_offset
- **Bypass Coding** — Out-of-range values are encoded with explicit bit-packing (variable bypass_bits). Supports both u8 (RansByte) and u32 (Rans64) bypass payloads
- **Encode Path** — For each value: determine distribution → look up symbol → encode with raw rANS → if out of range, encode symbol sentinel then bypass value
- **Decode Path** — Read raw symbol via CDF → if bypass sentinel, decode bypass value → reconstruct original value with offset

## Court Infrastructure

The forensic court system (`msrtc-rans-court`) provides differential testing:

| Court ID | Description | Status |
|----------|-------------|--------|
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` | Raw encoder (RansByte + Rans64) vs C++ | ✅ Sealed (8 cases) |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` | Raw decoder (Rust→C++ and C++→Rust cross-validation) | ✅ Sealed (16 cases) |
| `MSRTC.ENTROPY.DIFFERENTIAL` | Full entropy coder (encoder diff, roundtrip, cross-validate) | ✅ Sealed (6 cases) |
| `MSRTC.ORACLE.BASELINE` | Oracle Python test baseline | 🟡 Partial (observed only) |
| `MSRTC.RAW.RANSBYTE` | RansByte-specific differential court | 🔲 Scaffold |
| `MSRTC.RAW.RANS64` | Rans64-specific differential court | 🔲 Scaffold |
| `MSRTC.RECIPROCAL` | Reciprocal arithmetic three-way comparison | 🔲 Scaffold |
| `MSRTC.PMF` | PMF initialization rules | 🔲 Scaffold |
| `MSRTC.BYPASS` | Bypass coding differential | 🔲 Scaffold |
| `MSRTC.STREAM` | Streaming buffer management | 🔲 Scaffold |
| `MSRTC.BUFFER` | Buffer management policies | 🔲 Scaffold |
| `MSRTC.CROSS` | Cross-variant tests | 🔲 Scaffold |
| `MSRTC.INVALID` | Invalid input handling | 🔲 Scaffold |
| `MSRTC.PYTHON.API` | Python API surface parity | 🔲 Scaffold |
| `MSRTC.PLATFORM` | Multi-platform verification | 🔲 Scaffold |

Each court produces:
- **Receipt** — JSON evidence package with court_id, run_id, per-case hashes, environment fingerprint, Docker digest
- **Transcript** — Human-readable case-by-case report
- **Manifest** — Linked receipt + transcript hash, chain-of-custody record

## Key Design Decisions

1. **Macro-generated concrete types** over generic traits — avoids `as` cast limitations and matches C++ template expansion
2. **`#![forbid(unsafe_code)]`** in all production crates (exception: Python extension uses PyO3 ABI)
3. **`#![no_std]`** in core crate for embedded/wasm compatibility
4. **VecSink (reverse-order)** matches C++ `ResizableBufferSink` behavior — writes are reversed during `encoded()` (see C++ `Buf::end()` semantics)
5. **SliceSource** matches C++ `span<const unit_t>` source — zero-copy read from byte slice
6. **Transactional decoder** — decoder state is only committed on successful advance, preventing partial state corruption on truncated streams
7. **PMF-based encode** — values are mapped through distribution descriptors to raw symbols, then encoded with the raw rANS engine; out-of-range values use bypass coding

## Residual Doctrines

All mismatches are preserved as structured residuals in `courts/residuals/`. See `docs/residual-ledger.md` for current state.

### Classification Categories

| Classification | Meaning |
|----------------|---------|
| `native_bug` | Bug in the Rust implementation |
| `oracle_bug` | Bug in the C++ oracle |
| `oracle_undefined_or_assert_only` | C++ relies on UB or debug assertions |
| `intentional_safety_divergence` | Rust intentionally rejects UB |
| `environmental` | Divergence due to environment |
| `unclassified` | Not yet investigated |

## Current Phase 4 Sealed State

Four differential courts sealed:
- **Encoder differential:** 8/8 cases passing (at `dcfd39e80852`)
- **Decoder differential:** 16/16 cases passing (at `dcfd39e80852`)
- **Entropy differential:** 6/6 cases passing (at `dcfd39e80852`)
- **Stream differential:** 24/24 cases passing (at `aad4dfce2757`) — multipart persistent streams

Test suite: 123 defined, 119 active, 4 ignored (Docker-gated). All published crates at version 0.3.0.
