# msrtc-rans-casefile

**Deterministic casefile and residual formats for the msrtc_rans forensic courts.**

This crate defines the structured data formats used for casefiles (deterministic test inputs and expected outputs), residuals (structured mismatch records), receipts (sealed court evidence), and transcripts (human-readable court proceedings). Used by the differential court system in `msrtc-rans-court`.

## Status — Four Courts Sealed ✅

| Component | Status |
|-----------|--------|
| `DifferentialResult` schema | ✅ Stable |
| `CourtReceipt` schema | ✅ Stable |
| `InputHashes` fields | ✅ Stable |
| `ResidualClassification` enum | ✅ 9 variants |
| `ResolutionState` enum | ✅ 6 states |

All schemas are production-stable and used by the four sealed differential courts (raw encoder 8/8, raw decoder 16/16, entropy 6/6, stream 24/24).

## Contents

- **`DifferentialResult`** — Full differential comparison record with input hashes, oracle output, native output, comparison, classification, and resolution
- **`CourtReceipt`** — Sealed court evidence with per-case summaries
- **`InputHashes`** — Per-field SHA-256 hashes for casefile traceability
- **`DockerProvenance`** — Docker environment metadata for reproducibility
- **`sha256()`** — Convenience SHA-256 hashing function

## Repository

Full project: [github.com/infinityabundance/msrtc-rans-rs](https://github.com/infinityabundance/msrtc-rans-rs)

## License

MIT — see [LICENSE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/LICENSE) and [NOTICE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/NOTICE) for attribution notices.
