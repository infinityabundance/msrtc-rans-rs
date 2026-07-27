# Copyright (c) Microsoft Corporation.
# Licensed under the MIT license.

from enum import IntEnum


class RansVariant(IntEnum):
    """
    rANS variant enumeration.

    Maps to the C++ enum used in msrtc_rans:
        Rans64 = 0  (64-bit state, 32-bit units)
        RansByte = 1 (32-bit state, 8-bit units)
    """

    Rans64 = 0
    RansByte = 1
