# msrtc-rans-python

**Python extension module for msrtc_rans (via PyO3).**

This crate provides Python bindings for the msrtc_rans entropy coder. Built with PyO3, it exposes the `_msrtc_rans` native module, layered with the `msrtc.rans` Python package (`EntropyEncoder`, `EntropyDecoder`, `RansEncoderStream`, `RansDecoderStream`, `RansVariant`).

## Status — Phase 6 + Phase 4 Streams ✅

| Component | Status |
|-----------|--------|
| PyO3 extension module | ✅ Full API |
| Persistent multipart streaming | ✅ **Wire-parity proven** — `RansEncoderStream` keeps one persistent rANS state across `push()`; matches Microsoft `RawRansEncoderStream` |
| `RansDecoderStream` | ✅ Persistent decode cursor (position + state) across `decode()` calls; `decodeEOF` enforces full consumption + EOF |
| Upstream tests pass | ✅ All 7 upstream `test_msrtc_rans.py` tests pass, including `test_encode_decode_multi_part_0` and `test_rans_encoder_stream_0` |
| Bitstream fixtures | ✅ RansByte `0500bd040001a10003000b00` and Rans64 `0500a1bd04000000110a002f03000300` match byte-for-byte |
| Type stubs | ✅ `_msrtc_rans.pyi` |
| Wheel builds | ✅ maturin (built as `_msrtc_rans`; `msrtc.rans` package layered into site-packages) |

### Known hardening item (not a correctness issue)

The FFI buffer helpers (`get_i32_buffer`, `write_i32_buffer`) currently read/write raw `Py_buffer` memory. Output buffers are checked for writability, and `indices`/`values` length mismatch is rejected, but a future hardening pass will validate `Py_buffer` metadata (ndim, format, itemsize, alignment, exact capacity) instead of the current byte-copy approach. The core codec is fully safe Rust; this item is confined to the Python boundary.

## Repository

Full project: [github.com/infinityabundance/msrtc-rans-rs](https://github.com/infinityabundance/msrtc-rans-rs)

## License

MIT — see [LICENSE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/LICENSE) and [NOTICE](https://github.com/infinityabundance/msrtc-rans-rs/blob/main/NOTICE) for attribution notices.
