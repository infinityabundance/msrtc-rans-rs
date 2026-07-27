# msrtc-rans-rs

Native Rust forensic-parity replacement for Microsoft MLVC's `msrtc_rans` entropy coder.

## Status

**Active development — Phase 0/1 complete**

| Component | Status |
|-----------|--------|
| Upstream oracle lock | ✅ Sealed (commit `0500356a`) |
| Workspace scaffold | ✅ Complete |
| RansByte core | ✅ Implemented and tested |
| Rans64 core | ✅ Implemented and tested |
| Arithmetic (Mul64Hi, reciprocal) | ✅ Implemented and tested |
| Sink/Source traits | ✅ Implemented and tested |
| Public Rust API | ✅ Scaffold |
| Python extension | 📋 Scaffold |
| Docker matrix | 📋 Configured |
| Differential courts | 📋 Scaffold |
| MLVC integration | ❌ Not started |
| Performance benchmarking | ❌ Not started |

## Architecture

```
msrtc-rans-core/   → Deterministic no_std rANS primitives (RansByte, Rans64)
msrtc-rans/        → Safe public Rust entropy-coder API
msrtc-rans-python/ → Python extension (_msrtc_rans)
msrtc-rans-oracle/ → Developer-only C++ oracle adapter
msrtc-rans-casefile/ → Deterministic casefile/residual formats
msrtc-rans-court/  → Differential forensic courts
msrtc-rans-bench/  → Matched Rust/C++ benchmark harness
xtask/             → Build orchestration and freshness checks
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

See `oracle/upstream.lock` for details.

## License

MIT — see LICENSE and NOTICE for attribution notices.
