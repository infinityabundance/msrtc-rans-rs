# msrtc-rans-rs Residual Ledger
*Updated: 2026-07-27* — **Phase 3 — Three Courts Sealed**

## Overview

All behavioural mismatches between the Rust implementation and the pinned Microsoft C++ oracle are captured as structured residuals in `courts/residuals/`. Residuals are classified by type, assigned a resolution state, and tracked through the resolution lifecycle.

## Active Residuals

Three active residuals remain, all classified as **intentional safety divergences** — Rust is correct to reject these inputs; the C++ oracle has undefined behaviour.

### MSRTC.RAW.SCALE32 — Intentional Safety Divergence

- **Case:** `scale_bits=32` in `Rans64Encoder::try_new()` / `Rans64Encoder::try_put_raw()`
- **C++ behavior:** The C++ oracle uses `(1u << scale_bits)` which is technically **undefined behaviour** when `scale_bits == 32` (shift equals type width). However, on x86_64 with gcc 12.2.0, the result wraps to 0, causing a division-by-zero at runtime.
- **Rust behavior:** Explicitly rejects `scale_bits >= 32` with `RawRansError::InvalidParameters`. Also rejects `scale_bits < 2`.
- **Classification:** `intentional_safety_divergence` — Rust is correct to reject; C++ has undefined behaviour.
- **Resolution:** `open` — permanently open as a documented safety improvement.

### MSRTC.RAW.SYMBOLBITS32 — Intentional Safety Divergence

- **Case:** `symbol_bits=32` in entropy encoder initialize
- **C++ behavior:** Undefined (shift-related issues)
- **Rust behavior:** Rejected with `EntropyError::InvalidParams`
- **Classification:** `intentional_safety_divergence`
- **Resolution:** `open`

### MSRTC.RAW.BYPASSBITS32 — Intentional Safety Divergence

- **Case:** `bypass_bits=32` in entropy encoder initialize
- **C++ behavior:** Undefined (shift-related issues)
- **Rust behavior:** Rejected with `EntropyError::InvalidParams`
- **Classification:** `intentional_safety_divergence`
- **Resolution:** `open`

## Resolved Residuals

### Entropy Differential — RansByte native mismatch (seed 0)

- **Original issue:** Rust entropy encoder produced 11 bytes vs C++ oracle's 12 bytes for RansByte test case
- **Root cause:** Fixed in commit `db4a5e7` — entropy court wiring, overflow protection fixes
- **Current status:** `MSRTC.ENTROPY.DIFFERENTIAL` passes 6/6, receipt sealed
- **Resolution:** **Resolved** — stale residual file preserved for audit trail

### Entropy Differential — Rans64 native mismatch (seed 1)

- **Original issue:** Rust entropy encoder produced 12 bytes vs C++ oracle's 16 bytes for Rans64 test case
- **Root cause:** Fixed in commit `db4a5e7` — entropy court wiring, overflow protection fixes
- **Current status:** `MSRTC.ENTROPY.DIFFERENTIAL` passes 6/6, receipt sealed
- **Resolution:** **Resolved** — stale residual file preserved for audit trail

## Residual Files

Residuals are stored as JSON files in `courts/residuals/`:

| File | Court | Classification | Resolution |
|------|-------|---------------|------------|
| `MSRTC_RAW_SCALE32.json` | `MSRTC.RAW.SCALE32` | `oracle_undefined_or_assert_only` → `intentional_safety_divergence` | `open` |
| `MSRTC_ENTROPY_DIFFERENTIAL_sha25687cf...native_bug.json` | `MSRTC.ENTROPY.DIFFERENTIAL` | `native_bug` | **resolved** — court now passes |
| `MSRTC_ENTROPY_DIFFERENTIAL_sha2560321...native_bug.json` | `MSRTC.ENTROPY.DIFFERENTIAL` | `native_bug` | **resolved** — court now passes |

## Resolution States

| State | Meaning |
|-------|---------|
| `open` | Newly discovered, awaiting investigation |
| `reproduced` | Successfully reproduced and confirmed |
| `minimized` | Reduced to smallest reproducible case |
| `explained` | Root cause identified |
| `fixed` | Corrective action applied |
| `sealed` | Proved and permanently recorded |
