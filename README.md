# msrtc-rans-rs

Native Rust forensic-parity replacement for Microsoft MLVC's `msrtc_rans` entropy coder.

## Current Status — Phase 0/1 Partial

| Component | Status |
|-----------|--------|
| Upstream oracle source pin | ✅ Complete (commit `0500356a`) |
| Oracle baseline observation | 🟡 Partial — no formal receipt yet |
| Docker Debian oracle cell | 🟡 Observed externally |
| Docker matrix infrastructure | 🔲 Pending/incomplete |
| Raw rANS engine (RansByte/Rans64) | 🟡 Partial — internally tested, no oracle differential court |
| VecSink growth policy | 🟡 Partial — matches Microsoft formula, no oracle verification |
| `scale_bits == 32` rejection | 🟡 Implemented, residual created |
| Decoder transactional state | ✅ Fixed and tested |
| Python extension | 🔲 Scaffold |
| MLVC integration | ❌ Not started |
| Performance benchmarking | ❌ Not started |

## Architecture

```
msrtc-rans-core/   → Deterministic no_std rANS primitives (RansByte, Rans64)
msrtc-rans/        → Safe public Rust entropy-coder API
msrtc-rans-python/ → Python extension (_msrtc_rans) — not published on crates.io
msrtc-rans-oracle/ → Developer-only C++ oracle adapter — not published on crates.io
msrtc-rans-casefile/ → Deterministic casefile/residual formats
msrtc-rans-court/  → Differential forensic courts
msrtc-rans-bench/  → Matched Rust/C++ benchmark harness
xtask/             → Build orchestration and freshness checks — not published
```

## Quick Start

```bash
cargo build --workspace --exclude msrtc-rans-python
cargo test --workspace --exclude msrtc-rans-python
```

## Oracle

The Microsoft C++ oracle is pinned at:
- Repository: https://github.com/microsoft/mlvc
- Commit: `0500356a8d6146dd8dc8911022cbeca19675614f`
- Subdirectory: `packages/msrtc_rans`

See `oracle/upstream.lock` for details. A formal oracle baseline receipt is pending.

## Published Crates (crates.io)

| Crate | Version | Description |
|-------|---------|-------------|
| `msrtc-rans-core` | 0.1.0 | Deterministic no_std rANS primitives |
| `msrtc-rans` | 0.1.0 | Safe public Rust entropy-coder API |
| `msrtc-rans-casefile` | 0.1.0 | Casefile/residual formats |
| `msrtc-rans-court` | 0.1.0 | Differential forensic courts |
| `msrtc-rans-bench` | 0.1.0 | Benchmark harness |

## License

MIT — see LICENSE and NOTICE for attribution notices.

Author: Riaan de Beer — github.com/infinityabundance — rdebeer.infinityabundance@gmail.com
