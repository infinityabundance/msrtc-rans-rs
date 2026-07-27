# msrtc-rans-court

**Differential forensic courts for msrtc_rans parity verification.**

This crate implements the forensic courts that compare the native Rust implementation against the pinned Microsoft C++ oracle. Each court produces machine-readable receipts, human-readable transcripts, and structured residuals for every discovered difference.

## Status — Phase 3 Sealed ✅

| Court ID | Cases | Result |
|----------|-------|--------|
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` | 8 | ✅ Sealed |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` | 16 | ✅ Sealed |
| `MSRTC.ENTROPY.DIFFERENTIAL` | 6 | ✅ Sealed |

All three differential courts sealed at commit `dcfd39e80852`. 39 of 42 court tests pass; 3 scaffold courts are `#[ignore]`d.

## Court Modules

- `encoder_differential` — Raw rANS encoder vs C++ oracle
- `raw_decoder_differential` — Raw rANS decoder cross-validation (both directions)
- `entropy_differential` — Full entropy coder (PMF, bypass, CDF)
- `raw_ransbyte` — RansByte-specific tests (scaffold)
- `raw_rans64` — Rans64-specific tests (scaffold)
- `reciprocal` — Reciprocal arithmetic three-way comparison (scaffold)
- `pmf` — PMF initialization rules (scaffold)
- `bypass` — Bypass coding differential (scaffold)
- `oracle` — Oracle transport, validation, and residual persistence
- `seal` — Receipt, transcript, and manifest generation

## Usage

```bash
# Seal all courts
cargo run -p msrtc-rans-court --bin seal -- --all

# Seal specific court
cargo run -p msrtc-rans-court --bin seal -- --encoder
cargo run -p msrtc-rans-court --bin seal -- --decoder
cargo run -p msrtc-rans-court --bin seal -- --entropy
```

## Repository

Full project: [github.com/infinityabundance/msrtc-rans-rs](https://github.com/infinityabundance/msrtc-rans-rs)

## License

MIT — see [LICENSE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/LICENSE) and [NOTICE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/NOTICE) for attribution notices.
