# msrtc-rans-court

**Differential forensic courts for msrtc_rans parity verification.**

This crate implements the forensic courts that compare the native Rust implementation against the pinned Microsoft C++ oracle. Each court produces machine-readable receipts, human-readable transcripts, and structured residuals for every discovered difference.

## Status — Four Courts Sealed ✅

| Court ID | Cases | Result |
|----------|-------|--------|
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` | 8 | ✅ Sealed |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` | 16 | ✅ Sealed |
| `MSRTC.ENTROPY.DIFFERENTIAL` | 6 | ✅ Sealed |
| `MSRTC.STREAM.DIFFERENTIAL` | 24 | ✅ Sealed |

Raw + entropy courts sealed at `dcfd39e80852`; stream court sealed at `aad4dfce2757`. 43 of 47 court tests pass in the default run; 4 Docker-gated full-differential tests are `#[ignore]`d (they pass when the oracle image is present).

## Court Modules

- `raw_encoder_differential` — Raw rANS encoder vs C++ `raw_oracle_cli`
- `raw_decoder_differential` — Raw rANS decoder cross-validation (both directions) via `decoder_oracle_cli`
- `entropy_differential` — Full entropy coder (PMF, bypass, CDF) vs `oracle_cli`
- `stream_differential` — Multipart persistent streams vs `stream_oracle_cli` (wire parity + cross-decode)
- `raw_ransbyte` — RansByte-specific tests (scaffold)
- `raw_rans64` — Rans64-specific tests (scaffold)
- `reciprocal` — Reciprocal arithmetic three-way comparison (scaffold)
- `pmf` — PMF initialization rules (scaffold)
- `bypass` — Bypass coding differential (scaffold)
- `oracle` — Oracle transport, validation, and residual persistence
- `seal` — Receipt, transcript, and manifest generation

## Usage

```bash
# Seal a court (requires the Docker oracle image; writes receipt/transcript/manifest)
cargo run -p msrtc-rans-court --bin seal -- encoder
cargo run -p msrtc-rans-court --bin seal -- decoder
cargo run -p msrtc-rans-court --bin seal -- entropy
cargo run -p msrtc-rans-court --bin seal -- stream
```

A court is **sealable** only when: status is `Passed`, at least one case ran, every case passed, zero residuals, and zero skips. `seal()` refuses non-sealable results before writing any artifact.

## Repository

Full project: [github.com/infinityabundance/msrtc-rans-rs](https://github.com/infinityabundance/msrtc-rans-rs)

## License

MIT — see [LICENSE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/LICENSE) and [NOTICE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/NOTICE) for attribution notices.
