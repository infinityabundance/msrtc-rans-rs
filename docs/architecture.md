# msrtc-rans-rs Architecture

## Core/Shell Separation

```
                    ┌─────────────────────┐
                    │  msrtc-rans-python   │  (PyO3 shell)
                    │  Python API surface  │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │    msrtc-rans        │  (Safe public API)
                    │  EntropyCoder, etc   │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │  msrtc-rans-core     │  (no_std deterministic)
                    │  Encoder/Decoder     │
                    │  Arithmetic          │
                    └─────────────────────┘
```

## Variant Architecture

| Variant | State Type | Unit Type | STATE_BITS | LOWER_BOUND | UNITS_PER_STATE |
|---------|-----------|-----------|------------|-------------|-----------------|
| RansByte | `u32` | `u8` | 31 | 1 << 23 | 4 |
| Rans64 | `u64` | `u32` | 63 | 1 << 31 | 2 |

Both variants are generated from a single macro (`generate_rans_impl!`) in `raw.rs`,
matching the C++ template pattern.

## Key Design Decisions

1. **Macro-generated concrete types** over generic traits — avoids `as` cast limitations
2. **`#![forbid(unsafe_code)]`** in all production crates
3. **`#![no_std]`** in core crate for embedded compatibility
4. **VecSink (reverse-order)** matches C++ `ResizableBufferSink` behavior
5. **SliceSource** matches C++ `span<const unit_t>` source

## Residual Doctrines

All mismatches are preserved as structured residuals in `courts/residuals/`.
See `docs/residual-ledger.md` for current open residuals.
