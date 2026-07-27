#!/usr/bin/env python3
"""Generate a binary casefile for the oracle_cli harness."""
import struct
import sys


def write_casefile(path, variant, symbol_bits, bypass_bits,
                   pmf_lengths, pmf_offsets, pmf_table,
                   indices, values):
    """Write a binary casefile in the oracle_cli format."""
    with open(path, 'wb') as f:
        # variant: 0=Rans64, 1=RansByte
        f.write(struct.pack('<I', variant))
        f.write(struct.pack('<I', symbol_bits))
        f.write(struct.pack('<I', bypass_bits))

        # pmf_lengths
        f.write(struct.pack('<I', len(pmf_lengths)))
        for v in pmf_lengths:
            f.write(struct.pack('<i', v))

        # pmf_offsets
        f.write(struct.pack('<I', len(pmf_offsets)))
        for v in pmf_offsets:
            f.write(struct.pack('<i', v))

        # pmf_table
        f.write(struct.pack('<I', len(pmf_table)))
        for v in pmf_table:
            f.write(struct.pack('<i', v))

        # indices
        f.write(struct.pack('<I', len(indices)))
        for v in indices:
            f.write(struct.pack('<i', v))

        # values
        f.write(struct.pack('<I', len(values)))
        for v in values:
            f.write(struct.pack('<i', v))


# Reference test case from upstream test_msrtc_rans.py:
PMF_LENGTHS = [4, 6]
PMF_OFFSETS = [1, 2]
PMF_TABLE   = [1, 3, 1, 1, 1, 3, 5, 3, 1, 1]
INDICES     = [0, 1, 0, 1]
VALUES      = [-2, 1, 0, 1]
SYMBOL_BITS = 16
BYPASS_BITS = 4

if __name__ == '__main__':
    # RansByte casefile
    write_casefile(
        '/workspace/harness/casefile_ransbyte.bin',
        variant=1,         # RansByte
        symbol_bits=SYMBOL_BITS,
        bypass_bits=BYPASS_BITS,
        pmf_lengths=PMF_LENGTHS,
        pmf_offsets=PMF_OFFSETS,
        pmf_table=PMF_TABLE,
        indices=INDICES,
        values=VALUES,
    )

    # Rans64 casefile
    write_casefile(
        '/workspace/harness/casefile_rans64.bin',
        variant=0,         # Rans64
        symbol_bits=SYMBOL_BITS,
        bypass_bits=BYPASS_BITS,
        pmf_lengths=PMF_LENGTHS,
        pmf_offsets=PMF_OFFSETS,
        pmf_table=PMF_TABLE,
        indices=INDICES,
        values=VALUES,
    )

    print("Created casefiles:")
    print("  /workspace/harness/casefile_ransbyte.bin")
    print("  /workspace/harness/casefile_rans64.bin")
