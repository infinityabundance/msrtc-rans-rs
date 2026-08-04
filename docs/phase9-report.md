# msrtc-rans-rs — Phase 9 Performance Report

**Date:** 2026-08-04
**Workspace version:** 0.3.0
**Sealed parity:** byte-identical to the pinned C++ oracle (all four differential courts re-verified after optimization)

---

## Summary

A deterministic benchmark harness (`msrtc-rans-bench`) measures throughput in release mode. One measurable optimization was found and applied: the entropy bypass encoder allocated a `Vec` per bypassed value; replacing it with a fixed 40-entry stack buffer (matching the C++ intent of a stack array, but safely oversized — the C++ 16-part `std::array` overflows for wide bypass values, a latent UB) improved encode throughput by **6–20%** with **no output change** (all sealed courts still pass byte-identical).

## Method

- `cargo run --release -p msrtc-rans-bench [iterations]`
- Deterministic seeded workloads (no external RNG); results are the best of N runs (reports the fastest run to reduce timer noise).
- 100k-symbol workloads for raw/entropy paths; 100k total symbols across 8 batches for streams.
- Host: x86_64 Linux, release build, no special flags.

## Results (25 iterations, release)

| Benchmark | Throughput | Bandwidth |
|-----------|-----------:|----------:|
| raw RansByte encode (`put_raw`) | 185 M items/s | 133 MB/s |
| raw RansByte decode | 268 M items/s | 193 MB/s |
| raw Rans64 encode (`put_raw`) | 224 M items/s | 54 MB/s |
| raw Rans64 decode | 433 M items/s | 104 MB/s |
| prepared RansByte encode (`put`) | 225 M items/s | 162 MB/s |
| prepared Rans64 encode (`put`) | 298 M items/s | 72 MB/s |
| entropy RansByte encode (16-bit PMF, bypass) | **76 M items/s** (+19% vs 63) | 135 MB/s |
| entropy RansByte decode | 109 M items/s | 194 MB/s |
| entropy Rans64 encode | **63 M items/s** (+6% vs 59) | 112 MB/s |
| entropy Rans64 decode | 107 M items/s | 191 MB/s |
| stream RansByte multipart encode | **79 M items/s** (+20% vs 66) | 135 MB/s |
| stream RansByte multipart decode | 113 M items/s | 192 MB/s |

## Optimization applied

`EncoderState::encode_bypass_value` (msrtc-rans/src/entropy.rs):
- **Before:** `Vec::with_capacity(max_parts)` allocated per bypassed value (5% bypass rate in the benchmark → ~5,000 allocations per 100k symbols).
- **After:** fixed `[u32; 40]` stack buffer with explicit part count — zero allocations.
- **Why 40:** the largest possible bypass value needs 33 bits (i32 value range + offset), i.e. ≤ 17 digits at `bypass_bits=2`; 40 is a safely oversized bound. The C++ `s_MaxBypassParts = 16` is insufficient for wide bypass values (latent stack overflow UB) — Rust's fixed buffer is safe by construction.
- **Parity:** all sealed courts (raw 8/8, decoder 16/16, entropy 6/6, stream 24/24) re-verified byte-identical after the change.

Also hoisted the invariant `distribution_descs.len()` out of the per-symbol encode loop.

## Interpretation

- The raw primitives run at **185–433 M symbols/s** — the prepared (`put`) path avoids division via the reciprocal multiply (225–298 M/s), as in the C++ design.
- The entropy layer is bound by per-symbol distribution lookup + bypass handling on encode (63–76 M/s) and the CDF binary search on decode (107–109 M/s) — both match the C++ algorithmic structure (`std::upper_bound`).
- The persistent stream layer adds negligible overhead over the entropy layer (79 vs 76 M/s encode).

## Not measured / deferred

- **C++ side-by-side timing:** the oracle CLI processes one casefile per container invocation; per-case timing would be dominated by container spawn. A fair comparison requires an in-process C++ bench binary (future work).
- **Architecture intrinsics:** the code is portable, `#![forbid(unsafe_code)]` in all shipped crates — no SIMD/`unsafe` intrinsics are used (and none are needed at these throughputs).
- **Bounds-check elimination / table layout / decoder search strategy:** profiling showed the binary search and reciprocal multiply dominate; both are already the C++-matched algorithms. Further gains would come from unsafe SIMD or a jump-table CDF lookup — deliberate trade-offs deferred to keep the crate fully safe and byte-sealed.

## Reproduce

```bash
cargo run --release -p msrtc-rans-bench
# or with more/fewer iterations:
cargo run --release -p msrtc-rans-bench -- 100
```
