# Copyright (c) Microsoft Corporation.
# Licensed under the MIT license.
# Phase 7 MLVC integration comparison — msrtc-rans-rs (Riaan de Beer)
#
# compare.py — byte-for-byte differential comparison of the C++-backend and
# Rust-backend MLVC harness runs.
#
# Usage: python compare.py --cpp /out/cpp --rust /out/rust
#
# Verifies, for every case:
#   - bitstream SHA-256 identical (byte-for-byte wire equality)
#   - bitstream length identical
#   - bpp identical (exact float equality)
#   - reconstruction flags identical
# And that aggregate bit accounting matches. Exits non-zero on any mismatch.

import argparse
import hashlib
import json
import pathlib
import sys


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def load_report(root: pathlib.Path) -> dict:
    return json.loads((root / "report.json").read_text())


def main() -> None:
    parser = argparse.ArgumentParser(description="Compare MLVC harness runs")
    parser.add_argument("--cpp", required=True, help="C++ backend output dir")
    parser.add_argument("--rust", required=True, help="Rust backend output dir")
    parser.add_argument("--json", default=None, help="write comparison JSON here")
    args = parser.parse_args()

    cpp_root = pathlib.Path(args.cpp)
    rust_root = pathlib.Path(args.rust)

    cpp = load_report(cpp_root)
    rust = load_report(rust_root)

    errors = []
    checks = []

    if cpp["total_cases"] != rust["total_cases"]:
        errors.append(
            f"case count mismatch: cpp={cpp['total_cases']} rust={rust['total_cases']}"
        )

    cpp_cases = {
        r["case"]: r for section in cpp["sections"].values() for r in section
    }
    rust_cases = {
        r["case"]: r for section in rust["sections"].values() for r in section
    }

    if set(cpp_cases) != set(rust_cases):
        errors.append(
            f"case id sets differ: only_cpp={sorted(set(cpp_cases) - set(rust_cases))} "
            f"only_rust={sorted(set(rust_cases) - set(cpp_cases))}"
        )

    for case_id in sorted(cpp_cases):
        c = cpp_cases[case_id]
        r = rust_cases.get(case_id)
        if r is None:
            continue
        sha_ok = c["bitstream_sha256"] == r["bitstream_sha256"]
        len_ok = c["length"] == r["length"]
        bpp_ok = c["bpp"] == r["bpp"]
        recon_ok = c.get("y_recon_ok") == r.get("y_recon_ok") and c.get(
            "z_recon_ok"
        ) == r.get("z_recon_ok")

        # Independent byte-level check of the preserved bitstreams
        cpp_bytes = (cpp_root / "bitstreams" / f"{case_id}.bin").read_bytes()
        rust_bytes = (rust_root / "bitstreams" / f"{case_id}.bin").read_bytes()
        byte_ok = cpp_bytes == rust_bytes and sha256_hex(cpp_bytes) == c["bitstream_sha256"]

        ok = sha_ok and len_ok and bpp_ok and recon_ok and byte_ok
        checks.append(
            {
                "case": case_id,
                "ok": ok,
                "sha256_identical": sha_ok,
                "length_identical": len_ok,
                "bpp_identical": bpp_ok,
                "recon_identical": recon_ok,
                "preserved_bytes_identical": byte_ok,
                "cpp_sha256": c["bitstream_sha256"],
                "rust_sha256": r["bitstream_sha256"],
                "cpp_length": c["length"],
                "rust_length": r["length"],
                "cpp_bpp": c["bpp"],
                "rust_bpp": r["bpp"],
            }
        )
        if not ok:
            errors.append(f"case {case_id}: mismatch {json.dumps(checks[-1])}")

    total_bits_ok = cpp["total_bits"] == rust["total_bits"]
    checks.append(
        {
            "case": "aggregate",
            "ok": total_bits_ok,
            "cpp_total_bits": cpp["total_bits"],
            "rust_total_bits": rust["total_bits"],
        }
    )
    if not total_bits_ok:
        errors.append(
            f"total bits mismatch: cpp={cpp['total_bits']} rust={rust['total_bits']}"
        )

    result = {
        "ok": len(errors) == 0,
        "cases_compared": len(cpp_cases),
        "errors": errors,
        "checks": checks,
    }

    if args.json:
        pathlib.Path(args.json).write_text(json.dumps(result, indent=2))

    if result["ok"]:
        print(
            f"[compare] PASS: {len(cpp_cases)} cases byte-identical, "
            f"total_bits cpp={cpp['total_bits']} rust={rust['total_bits']}"
        )
        return 0
    else:
        print(f"[compare] FAIL: {len(errors)} error(s)")
        for e in errors:
            print("  -", e)
        return 1


if __name__ == "__main__":
    sys.exit(main())
