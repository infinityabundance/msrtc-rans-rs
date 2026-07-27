# Copyright (c) Microsoft Corporation.
# Licensed under the MIT license.

from typing import Any

RansByte: int
Rans64: int

class RansEncoderStream:
    def __init__(self, variant: int = 1, *, initialSize: int = 4096, maxSizeStep: int = 1048576) -> None: ...
    def flush(self) -> bytes: ...
    def reset(self) -> None: ...

class RansDecoderStream:
    def __init__(self, data: Any = None, *, variant: int = 1) -> None: ...
    def open(self, data: Any) -> None: ...
    def close(self) -> None: ...
    def isOpen(self) -> bool: ...
    def decodeEOF(self) -> None: ...

class EntropyEncoder:
    def __init__(
        self,
        *,
        pmfLengths: Any,
        pmfOffsets: Any,
        pmfTable: Any,
        variant: int = 1,
        symbolBits: int = 16,
        bypassBits: int = 4,
    ) -> None: ...
    def encode(self, stream: RansEncoderStream, indices: Any, values: Any) -> None: ...

class EntropyDecoder:
    def __init__(
        self,
        *,
        pmfLengths: Any,
        pmfOffsets: Any,
        pmfTable: Any,
        variant: int = 1,
        symbolBits: int = 16,
        bypassBits: int = 4,
    ) -> None: ...
    def decode(self, values: Any, indices: Any, data: Any) -> None: ...
