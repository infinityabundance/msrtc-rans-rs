# msrtc-rans-rs Court Index
*Generated: 2026-07-27* — **Phase 3 — Three Courts Sealed**

## Court Registry

| Court ID | Status | Cases | Passes | Residuals | Receipt |
|----------|--------|-------|--------|-----------|---------|
| `MSRTC.INVENTORY` | 🟡 `partial` | — | — | — | ❌ |
| `MSRTC.ORACLE.BASELINE` | 🟡 `partial` | 7 | 7 | 0 | ❌ (observed, not receipted) |
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` | ✅ `sealed` | 8 | 8 | 0 | ✅ Sealed |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` | ✅ `sealed` | 16 | 16 | 0 | ✅ Sealed |
| `MSRTC.ENTROPY.DIFFERENTIAL` | ✅ `sealed` | 6 | 6 | 0 | ✅ Sealed |
| `MSRTC.RAW.RANSBYTE` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.RAW.RANS64` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.RECIPROCAL` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.PMF` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.BYPASS` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.STREAM` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.BUFFER` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.CROSS` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.INVALID` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.PYTHON.API` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.PLATFORM` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |

## Legend

- ✅ `sealed` — Receipt exists, all cases pass
- 🟡 `partial` — Cases exist but no formal receipt
- 🔲 `scaffold` — Not yet implemented

## Sealed Court Summaries

### MSRTC.RAW.ENCODER.DIFFERENTIAL
- **Cases:** 8 (4 RansByte + 4 Rans64, covering raw + prepared modes, single + multiple symbols, freq=1 edge case)
- **Oracle CLI:** `raw_oracle_cli`
- **Run ID:** `20260727T082625_7b5fc52f91d1`

### MSRTC.RAW.DECODER.DIFFERENTIAL
- **Cases:** 16 (Rust encode → C++ decode and C++ encode → Rust decode, both RansByte and Rans64, multiple symbol configurations)
- **Oracle CLI:** `decoder_oracle_cli`
- **Run ID:** `20260727T082628_7b5fc52f91d1`

### MSRTC.ENTROPY.DIFFERENTIAL
- **Cases:** 6 (encoder differential + roundtrip + C++ encode/Rust decode cross-validation, RansByte + Rans64)
- **Oracle CLI:** `oracle_cli`
- **Run ID:** `20260727T082628_7b5fc52f91d1`

## Receipt Files

- `courts/receipts/MSRTC_MSRTC_RAW_ENCODER_DIFFERENTIAL_20260727T082625_7b5fc52f91d1.json`
- `courts/receipts/MSRTC_MSRTC_RAW_DECODER_DIFFERENTIAL_20260727T082628_7b5fc52f91d1.json`
- `courts/receipts/MSRTC_MSRTC_ENTROPY_DIFFERENTIAL_20260727T082628_7b5fc52f91d1.json`
