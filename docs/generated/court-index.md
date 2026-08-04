# msrtc-rans-rs Court Index
*Generated: 2026-08-04* — **Phase 4 — Four Courts Sealed**

## Court Registry

| Court ID | Status | Cases | Passes | Residuals | Receipt |
|----------|--------|-------|--------|-----------|---------|
| `MSRTC.INVENTORY` | 🟡 `partial` | — | — | — | ❌ |
| `MSRTC.ORACLE.BASELINE` | 🟡 `partial` | 7 | 7 | 0 | ❌ (observed, not receipted) |
| `MSRTC.RAW.ENCODER.DIFFERENTIAL` | ✅ `sealed` | 8 | 8 | 0 | ✅ Sealed |
| `MSRTC.RAW.DECODER.DIFFERENTIAL` | ✅ `sealed` | 16 | 16 | 0 | ✅ Sealed |
| `MSRTC.ENTROPY.DIFFERENTIAL` | ✅ `sealed` | 6 | 6 | 0 | ✅ Sealed |
| `MSRTC.STREAM.DIFFERENTIAL` | ✅ `sealed` | 24 | 24 | 0 | ✅ Sealed |
| `MSRTC.RAW.RANSBYTE` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.RAW.RANS64` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.RECIPROCAL` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.PMF` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
| `MSRTC.BYPASS` | 🔲 `scaffold` | 0 | 0 | 0 | ❌ |
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
- **Sealed at commit:** `dcfd39e80852`
- **Receipt:** `courts/receipts/MSRTC_MSRTC_RAW_ENCODER_DIFFERENTIAL_20260727T082625_7b5fc52f91d1.json`

### MSRTC.RAW.DECODER.DIFFERENTIAL
- **Cases:** 16 (Rust encode → C++ decode and C++ encode → Rust decode, both RansByte and Rans64, multiple symbol configurations)
- **Oracle CLI:** `decoder_oracle_cli`
- **Run ID:** `20260727T082628_7b5fc52f91d1`
- **Sealed at commit:** `dcfd39e80852`
- **Receipt:** `courts/receipts/MSRTC_MSRTC_RAW_DECODER_DIFFERENTIAL_20260727T082628_7b5fc52f91d1.json`

### MSRTC.ENTROPY.DIFFERENTIAL
- **Cases:** 6 (encoder differential + roundtrip + C++ encode/Rust decode cross-validation, RansByte + Rans64)
- **Oracle CLI:** `oracle_cli`
- **Run ID:** `20260727T082628_7b5fc52f91d1`
- **Sealed at commit:** `dcfd39e80852`
- **Receipt:** `courts/receipts/MSRTC_MSRTC_ENTROPY_DIFFERENTIAL_20260727T082628_7b5fc52f91d1.json`

### MSRTC.STREAM.DIFFERENTIAL
- **Cases:** 24 (8 multipart stream cases × 3 sub-cases: wire parity, Microsoft stream → Rust decoder, Rust stream → Microsoft decoder; both RansByte and Rans64; 1–3 batches; bypass-bits 2 and 4; 256-symbol batches)
- **Oracle CLI:** `stream_oracle_cli`
- **Run ID:** `20260804T152041_aad4dfce2757`
- **Sealed at commit:** `aad4dfce2757`
- **Receipt:** `courts/receipts/MSRTC_MSRTC_STREAM_DIFFERENTIAL_20260804T152041_aad4dfce2757.json`

## Receipt Files

- `courts/receipts/MSRTC_MSRTC_RAW_ENCODER_DIFFERENTIAL_20260727T082625_7b5fc52f91d1.json`
- `courts/receipts/MSRTC_MSRTC_RAW_DECODER_DIFFERENTIAL_20260727T082628_7b5fc52f91d1.json`
- `courts/receipts/MSRTC_MSRTC_ENTROPY_DIFFERENTIAL_20260727T082628_7b5fc52f91d1.json`
- `courts/receipts/MSRTC_MSRTC_STREAM_DIFFERENTIAL_20260804T152041_aad4dfce2757.json`

## Evidence Chain

```
upstream.lock (pinned commit + fixture hashes)
    │
    ▼
Docker image (build environment + oracle CLIs)
    │
    ▼
Casefiles (deterministic test inputs, content-addressed by SHA-256)
    │
    ▼
Differential Court (Rust vs C++ byte comparison)
    │
    ├── Receipt (JSON: court_id, run_id, per-case hashes)
    ├── Transcript (TXT: human-readable case report)
    └── Manifest (JSON: linked receipt + transcript hash)
```
