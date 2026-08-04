# Copyright (c) Microsoft Corporation.
# Licensed under the MIT license.
# Phase 7 MLVC integration harness — msrtc-rans-rs (Riaan de Beer)
#
# mlvc_harness.py — deterministic end-to-end harness that drives the REAL
# MLVC code paths which consume `msrtc.rans`:
#
#   Section A — video/conversion/_coder.py
#       GaussianEncoder.encode_y/decode_y (index_space True and False)
#       BitEstimator.encode_z/decode_z
#       Multi-frame "video" simulation with per-frame bpp accounting
#       (bpp = bitstream_size_bits / (image_width * image_height), exactly
#       mirroring conversion/_frame_loop.py)
#
#   Section B — video/src/models/entropy_models.py (torch model paths)
#       AEHelper._encode/_decode driven through torch tensors with the real
#       stream helpers from video/src/utils/stream_helper.py
#       (open_encoder_streams / flush_encoder_streams / open_decoder_streams
#       / check_decoder_eof), including the multi-stream tuple branch.
#
# PMF tables are generated with MLVC's own quantize_pmf (entropy_models.py)
# converted to the frequency-table layout the coder consumes (diff of the
# quantized CDF — the exact layout of the sealed upstream fixtures, verified
# empirically against the C++ oracle).
#
# Everything is deterministic (fixed seeds, no wall-clock dependence), so the
# same harness run against the C++ `_msrtc_rans` and the Rust wheel must
# produce byte-identical bitstreams and identical bpp numbers.

import argparse
import importlib.util
import json
import os
import pathlib
import struct
import sys
import types

import numpy as np

# ---------------------------------------------------------------------------
# MLVC source registration
# ---------------------------------------------------------------------------

MLVC_ROOT = pathlib.Path(os.environ.get("MLVC_ROOT", "/workspace/mlvc/video"))

# Register `conversion` as a package WITHOUT executing conversion/__init__.py
# (which pulls the full model/export machinery). This still executes the
# real conversion/_coder.py and conversion/types.py bytes with real relative
# imports.
_conv_dir = MLVC_ROOT / "conversion"
_conv_pkg = types.ModuleType("conversion")
_conv_pkg.__path__ = [str(_conv_dir)]
_conv_pkg.__package__ = "conversion"
sys.modules["conversion"] = _conv_pkg

sys.path.insert(0, str(MLVC_ROOT))

import torch  # noqa: E402
from src.models.entropy_models import quantize_pmf  # noqa: E402

from conversion._coder import BitEstimator, GaussianEncoder  # noqa: E402
from conversion.types import BitEstimatorPmf, GaussianCoderPmf  # noqa: E402

# ---------------------------------------------------------------------------
# Deterministic PMF construction (MLVC quantizer + frequency conversion)
# ---------------------------------------------------------------------------


def quantized_freq_table(pmf, scale_bits=16):
    """Quantize a probability vector to a frequency table summing to
    2^scale_bits with every entry >= 1. Uses a deterministic
    largest-remainder allocation with exact total adjustment."""
    scale = 1 << scale_bits
    pmf = np.asarray(pmf, dtype=np.float64)
    pmf = pmf / pmf.sum()
    scaled = pmf * scale
    base = np.floor(scaled).astype(np.int64)
    base = np.maximum(base, 1)
    delta = int(base.sum()) - scale
    # Remove units from entries whose floor most exceeded their fair share.
    while delta > 0:
        excess = base - scaled
        idx = int(np.argmax(np.where(base > 1, excess, -np.inf)))
        base[idx] -= 1
        delta -= 1
    # Add units to entries with the largest fractional remainder.
    while delta < 0:
        frac = scaled - np.floor(scaled)
        idx = int(np.argmax(frac))
        base[idx] += 1
        delta += 1
    assert base.sum() == scale
    return base.astype(np.int32)


def gaussian_pmf_table(center, sigma, tail_mass=None):
    """Gaussian probability mass over [-center, center] plus tail mass,
    quantized to a frequency table (last entry = out-of-range mass)."""
    x = np.arange(-center, center + 1, dtype=np.float64)
    p = np.exp(-0.5 * (x / sigma) ** 2)
    p = p / p.sum()
    if tail_mass is None:
        tail_mass = p[0]  # mirror MLVC GaussianEncoder tail_mass = 2*lower[0]
    p = np.concatenate((p, [tail_mass]))
    return quantized_freq_table(p)


def build_gaussian_coder_pmf(scale_levels=32, index_space=False):
    """GaussianCoderPmf over `scale_levels` log-spaced scales, exactly like
    MLVC's conversion GaussianEncoder layout."""
    scale_min = 0.11
    scale_max = 16.0
    log_min = float(np.log(scale_min))
    log_max = float(np.log(scale_max))
    log_step = (log_max - log_min) / (scale_levels - 1)
    scales = np.exp(log_min + log_step * np.arange(scale_levels))

    pmf_lengths = []
    pmf_offsets = []
    pmf_table = []
    for s in scales:
        sigma = s / 2.0
        center = max(3, int(round(4.0 * sigma)))
        if center > 64:
            center = 64
        offset = -center
        tbl = gaussian_pmf_table(center, sigma)
        pmf_lengths.append(int(len(tbl)))
        pmf_offsets.append(int(offset))
        pmf_table.extend(tbl.tolist())

    return GaussianCoderPmf(
        scale_min=scale_min,
        scale_max=scale_max,
        scale_levels=scale_levels,
        index_space=index_space,
        pmf_lengths=pmf_lengths,
        pmf_offsets=pmf_offsets,
        pmf_table=pmf_table,
    )


def build_bit_estimator_pmf(qp_num=2, channels=8):
    """BitEstimatorPmf with qp_num x channels distributions."""
    pmf_lengths = []
    pmf_offsets = []
    pmf_table = []
    for i in range(qp_num * channels):
        sigma = 1.0 + 0.25 * (i % channels)
        center = 8
        tbl = gaussian_pmf_table(center, sigma)
        pmf_lengths.append(int(len(tbl)))
        pmf_offsets.append(-center)
        pmf_table.extend(tbl.tolist())

    return BitEstimatorPmf(
        qp_num=qp_num,
        channels=channels,
        pmf_lengths=pmf_lengths,
        pmf_offsets=pmf_offsets,
        pmf_table=pmf_table,
    )


# ---------------------------------------------------------------------------
# Section A — conversion/_coder.py (numpy coder paths)
# ---------------------------------------------------------------------------


def run_coder_section(rng, out_dir, frames=4, h=64, w=64):
    """Drive the real conversion.GaussianEncoder / conversion.BitEstimator
    over a synthetic multi-frame sequence; returns report records."""
    records = []

    gauss = build_gaussian_coder_pmf(scale_levels=32, index_space=False)
    gauss_idx = build_gaussian_coder_pmf(scale_levels=32, index_space=True)
    bit_est = build_bit_estimator_pmf(qp_num=2, channels=8)

    enc_y = GaussianEncoder(gauss)
    enc_y_idx = GaussianEncoder(gauss_idx)
    enc_z = BitEstimator(bit_est)
    dec_y = GaussianEncoder(gauss)
    dec_y_idx = GaussianEncoder(gauss_idx)
    dec_z = BitEstimator(bit_est)

    C = 8
    for f in range(frames):
        # Synthetic quantized y latent + scales
        y = rng.integers(-6, 7, size=(1, C, h, w)).astype(np.int32)
        scales = rng.uniform(0.5, 8.0, size=(1, C, h, w)).astype(np.float32)
        z = rng.integers(-8, 9, size=(1, 8, h // 8, w // 8)).astype(np.int32)
        qp = f % 2

        # --- encode (non-index-space scales) ---
        stream = None
        from msrtc.rans import RansEncoderStream, RansDecoderStream

        stream = RansEncoderStream()
        enc_y.encode_y(stream, y, scales)
        enc_z.encode_z(stream, z, qp)
        data = bytes(stream.flush())

        # --- decode (LIFO: decode z first, then y — matches MLVC dmc_61.py
        # decompress_dual_prior after decode_z_hat) ---
        dstream = RansDecoderStream(data)
        z_rec = dec_z.decode_z(dstream, (h // 8, w // 8), qp)
        y_rec = dec_y.decode_y(dstream, scales)
        dstream.decodeEOF()

        payload_bits = 8 * len(data)
        bpp = payload_bits / (h * w)  # mirrors _frame_loop bpp accounting

        records.append(
            {
                "case": f"coder_scales_f{f}",
                "bitstream_sha256": sha256_hex(data),
                "length": len(data),
                "bpp": bpp,
                "y_recon_ok": bool(np.array_equal(y, y_rec)),
                "z_recon_ok": bool(np.array_equal(z, z_rec)),
            }
        )
        write_bitstream(out_dir, f"coder_scales_f{f}.bin", data)

        # --- encode (index-space scales) ---
        scales_idx = rng.integers(0, 32, size=(1, C, h, w)).astype(np.int32)
        stream = RansEncoderStream()
        enc_y_idx.encode_y(stream, y, scales_idx)
        data = bytes(stream.flush())

        dstream = RansDecoderStream(data)
        y_rec = dec_y_idx.decode_y(dstream, scales_idx)
        dstream.decodeEOF()

        bpp = (8 * len(data)) / (h * w)
        records.append(
            {
                "case": f"coder_index_space_f{f}",
                "bitstream_sha256": sha256_hex(data),
                "length": len(data),
                "bpp": bpp,
                "y_recon_ok": bool(np.array_equal(y, y_rec)),
            }
        )
        write_bitstream(out_dir, f"coder_index_space_f{f}.bin", data)

    return records


# ---------------------------------------------------------------------------
# Section B — src/models/entropy_models.py (torch AEHelper paths)
# ---------------------------------------------------------------------------


def run_torch_section(out_dir, frames=2, h=32, w=32):
    """Drive the real torch AEHelper coder through the real stream helpers."""
    records = []
    torch.manual_seed(20260804)

    from src.models.entropy_models import GaussianEncoder as TorchGaussianEncoder

    from msrtc.rans import RansDecoderStream

    from src.utils.stream_helper import (
        check_decoder_eof,
        flush_encoder_streams,
        open_decoder_streams,
        open_encoder_streams,
    )

    # Torch GaussianEncoder with real deterministic build_pmf (no weights).
    enc = TorchGaussianEncoder(distribution="gaussian")
    enc.build_pmf()
    lengths, offsets, cdf_table = enc.get_pmf()
    assert lengths is not None and offsets is not None and cdf_table is not None
    # Convert the quantized CDF to a frequency table with every entry >= 1
    # (the layout the coder requires).
    cdf = np.asarray(cdf_table, dtype=np.float64)
    freq = np.diff(np.concatenate(([0], cdf)))
    freq = np.maximum(freq, 0)
    freq_table = quantized_freq_table(freq, 16)
    enc.set_pmf(np.asarray(lengths), np.asarray(offsets), freq_table)

    dec = TorchGaussianEncoder(distribution="gaussian")
    dec.set_pmf(np.asarray(lengths), np.asarray(offsets), freq_table)

    for f in range(frames):
        y = torch.randint(-6, 7, (1, 4, h, w), dtype=torch.int32)
        scales = torch.empty((1, 4, h, w)).uniform_(0.5, 8.0)

        # Single-stream branch
        streams = open_encoder_streams(1)
        enc._encode(streams, enc.build_indices(scales), y)
        data = flush_encoder_streams(streams)

        dstreams = open_decoder_streams(data)
        out = torch.empty_like(y)
        dec._decode(out, dec.build_indices(scales), dstreams[0])
        check_decoder_eof(dstreams)

        bpp = (8 * len(data)) / (h * w)
        records.append(
            {
                "case": f"torch_aehelper_f{f}",
                "bitstream_sha256": sha256_hex(data),
                "length": len(data),
                "bpp": bpp,
                "y_recon_ok": bool(torch.equal(y, out)),
            }
        )
        write_bitstream(out_dir, f"torch_aehelper_f{f}.bin", data)

    # Multi-stream tuple branch (batch_size=2): verify each reconstruction.
    pairs = []
    streams = open_encoder_streams(2)
    for s in streams:
        y2 = torch.randint(-6, 7, (1, 4, h, w), dtype=torch.int32)
        s2 = torch.empty((1, 4, h, w)).uniform_(0.5, 8.0)
        enc._encode(s, enc.build_indices(s2), y2)
        pairs.append((y2, s2))
    datas = flush_encoder_streams(streams)
    dstreams = open_decoder_streams(datas)
    recon_ok = []
    for i, (ds, (y2, s2)) in enumerate(zip(dstreams, pairs)):
        out = torch.empty_like(y2)
        dec._decode(out, dec.build_indices(s2), ds)
        recon_ok.append(bool(torch.equal(y2, out)))
    check_decoder_eof(dstreams)

    for i, d in enumerate(datas):
        bpp = (8 * len(d)) / (h * w)
        records.append(
            {
                "case": f"torch_multistream_b{i}",
                "bitstream_sha256": sha256_hex(d),
                "length": len(d),
                "bpp": bpp,
                "y_recon_ok": recon_ok[i],
            }
        )
        write_bitstream(out_dir, f"torch_multistream_b{i}.bin", d)

    return records


# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------


def sha256_hex(data: bytes) -> str:
    import hashlib

    return hashlib.sha256(data).hexdigest()


def write_bitstream(out_dir: pathlib.Path, name: str, data: bytes) -> None:
    (out_dir / "bitstreams").mkdir(parents=True, exist_ok=True)
    (out_dir / "bitstreams" / name).write_bytes(data)


def main() -> None:
    parser = argparse.ArgumentParser(description="MLVC msrtc.rans integration harness")
    parser.add_argument("--out", default="/out", help="output directory")
    parser.add_argument("--backend", default="unknown", help="backend tag (cpp|rust)")
    parser.add_argument("--seed", type=int, default=0xC0FFEE)
    args = parser.parse_args()

    out_dir = pathlib.Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    rng = np.random.default_rng(args.seed)

    sections = {}

    # Section A: conversion/_coder.py
    coder_records = run_coder_section(rng, out_dir)
    sections["conversion_coder"] = coder_records

    # Section B: torch entropy_models + stream_helper
    torch_records = run_torch_section(out_dir)
    sections["entropy_models"] = torch_records

    # Aggregate accounting (mirrors _frame_loop aggregate bpp)
    all_records = coder_records + torch_records
    total_bits = sum(8 * r["length"] for r in all_records)

    report = {
        "backend": args.backend,
        "seed": args.seed,
        "total_cases": len(all_records),
        "total_bits": total_bits,
        "sections": sections,
        "all_passed": all(
            r.get("y_recon_ok", False) and r.get("z_recon_ok", True)
            for r in all_records
        ),
    }

    (out_dir / "report.json").write_text(json.dumps(report, indent=2))
    print(f"[mlvc_harness] backend={args.backend} cases={len(all_records)} "
          f"total_bits={total_bits} all_passed={report['all_passed']}")


if __name__ == "__main__":
    main()
