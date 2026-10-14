"""Bothy: serverless, local-first sync over your own Google Drive.

Early development. This release only contains the on-remote format version and helpers for
the hybrid logical clock (HLC) timestamps that order every change. The sync engine will ship
as native bindings to the Rust core: https://github.com/bothy-sync/bothy
"""

from typing import NamedTuple

__all__ = ["FORMAT_VERSION", "Hlc", "__version__"]

__version__ = "0.0.1"

#: Version of the on-remote file format (``bothy_format`` in ``manifest.json``). Draft.
FORMAT_VERSION = 1

_LOGICAL_BITS = 16
_MAX_PHYSICAL_MS = (1 << (64 - _LOGICAL_BITS)) - 1
_HEX_DIGITS = frozenset("0123456789abcdef")


class Hlc(NamedTuple):
    """Hybrid logical clock timestamp: 48-bit physical milliseconds + 16-bit logical counter."""

    physical_ms: int
    logical: int

    @classmethod
    def from_u64(cls, raw: int) -> "Hlc":
        if not 0 <= raw < 1 << 64:
            raise ValueError("HLC must fit in 64 bits")
        return cls(raw >> _LOGICAL_BITS, raw & 0xFFFF)

    @classmethod
    def from_hex(cls, text: str) -> "Hlc":
        """Parse the fixed-width, lowercase 16-character hex form used in file names."""
        if len(text) != 16 or not _HEX_DIGITS.issuperset(text):
            raise ValueError("HLC must be exactly 16 lowercase hex characters")
        return cls.from_u64(int(text, 16))

    def to_u64(self) -> int:
        if not 0 <= self.physical_ms <= _MAX_PHYSICAL_MS:
            raise ValueError("physical_ms must fit in 48 bits")
        if not 0 <= self.logical <= 0xFFFF:
            raise ValueError("logical must fit in 16 bits")
        return (self.physical_ms << _LOGICAL_BITS) | self.logical

    def to_hex(self) -> str:
        return format(self.to_u64(), "016x")
