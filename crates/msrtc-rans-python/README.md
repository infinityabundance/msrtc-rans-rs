# msrtc-rans-python

**Python extension module for msrtc_rans (via PyO3).**

This crate provides Python bindings for the msrtc_rans entropy coder. Built with PyO3, it exposes the `_msrtc_rans` native module backed by maturin mixed project builds.

## Status — Phase 6 Complete ✅

| Component | Status |
|-----------|--------|
| PyO3 module structure | ✅ Full |
| Safe buffer handling | ✅ `PyBuffer`-based, no unsafe pointer casts |
| Persistent multipart streaming | ✅ `RansEncoderStream` holds persistent encoder state |
| Python bindings | ✅ Full API (`EntropyEncoder`, `EntropyDecoder`, streams) |
| Type stubs | ✅ `_msrtc_rans.pyi` |
| Wheel builds | ✅ maturin mixed project layout |
| Upstream tests pass | ✅ All 7 upstream Python tests pass |

The Python extension is fully functional. All 7 upstream `msrtc_rans` Python tests pass against the Rust implementation. The `msrtc.rans` package (`EntropyEncoder`, `EntropyDecoder`, `RansEncoderStream`, `RansDecoderStream`) provides the complete Python API.

## Repository

Full project: [github.com/infinityabundance/msrtc-rans-rs](https://github.com/infinityabundance/msrtc-rans-rs)

## License

MIT — see [LICENSE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/LICENSE) and [NOTICE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/NOTICE) for attribution notices.
