"""
Core MTProto serialization and deserialization primitives.

Inspired by grammers-tl-types' Serializable / Deserializable traits.
Every generated type and function implements these interfaces.
"""

from __future__ import annotations

import struct
from abc import ABC, abstractmethod
from typing import Any, BinaryIO, ClassVar, Optional, Type, TypeVar

T = TypeVar('T', bound='TLObject')


# ---------------------------------------------------------------------------
# Buffer helpers
# ---------------------------------------------------------------------------

class ByteBuffer:
    """
    A growable byte buffer for serialization — write-only.
    Mirrors grammers' ``impl Extend<u8>`` approach.
    """

    __slots__ = ('_data',)

    def __init__(self) -> None:
        self._data = bytearray()

    # --- primitives ---

    def write_u32(self, value: int) -> None:
        self._data += struct.pack('<I', value & 0xFFFFFFFF)

    def write_i32(self, value: int) -> None:
        self._data += struct.pack('<i', value)

    def write_i64(self, value: int) -> None:
        self._data += struct.pack('<q', value)

    def write_f64(self, value: float) -> None:
        self._data += struct.pack('<d', value)

    def write_bool(self, value: bool) -> None:
        # boolTrue#997275b5, boolFalse#bc799737
        self.write_u32(0x997275B5 if value else 0xBC799737)

    def write_bytes(self, data: bytes) -> None:
        """TL-encoded bytes: length-prefixed with padding to 4-byte alignment."""
        n = len(data)
        if n <= 253:
            self._data.append(n)
            self._data += data
            pad = (4 - (n + 1) % 4) % 4
        else:
            self._data.append(254)
            self._data += struct.pack('<I', n)[:3]
            self._data += data
            pad = (4 - n % 4) % 4
        self._data += b'\x00' * pad

    def write_string(self, value: str) -> None:
        self.write_bytes(value.encode('utf-8'))

    def write_int128(self, value: bytes) -> None:
        assert len(value) == 16, f"int128 must be 16 bytes, got {len(value)}"
        self._data += value

    def write_int256(self, value: bytes) -> None:
        assert len(value) == 32, f"int256 must be 32 bytes, got {len(value)}"
        self._data += value

    def write_raw(self, data: bytes) -> None:
        self._data += data

    def getvalue(self) -> bytes:
        return bytes(self._data)

    def __len__(self) -> int:
        return len(self._data)


class ByteReader:
    """
    A read-cursor over a bytes object for deserialization.
    Mirrors grammers' ``Cursor`` type.
    """

    __slots__ = ('_data', '_pos')

    def __init__(self, data: bytes) -> None:
        self._data = data
        self._pos = 0

    def remaining(self) -> int:
        return len(self._data) - self._pos

    def _require(self, n: int) -> None:
        if self.remaining() < n:
            raise DeserializeError(
                f"Not enough data: need {n} bytes, have {self.remaining()}"
            )

    def read_u32(self) -> int:
        self._require(4)
        val, = struct.unpack_from('<I', self._data, self._pos)
        self._pos += 4
        return val

    def read_i32(self) -> int:
        self._require(4)
        val, = struct.unpack_from('<i', self._data, self._pos)
        self._pos += 4
        return val

    def read_i64(self) -> int:
        self._require(8)
        val, = struct.unpack_from('<q', self._data, self._pos)
        self._pos += 8
        return val

    def read_f64(self) -> float:
        self._require(8)
        val, = struct.unpack_from('<d', self._data, self._pos)
        self._pos += 8
        return val

    def read_bool(self) -> bool:
        cid = self.read_u32()
        if cid == 0x997275B5:
            return True
        if cid == 0xBC799737:
            return False
        raise DeserializeError(f"Expected Bool constructor, got 0x{cid:08x}")

    def read_bytes(self) -> bytes:
        """TL-encoded bytes: reads length-prefixed, padded data."""
        self._require(1)
        first = self._data[self._pos]
        self._pos += 1

        if first <= 253:
            n = first
            self._require(n)
            data = self._data[self._pos:self._pos + n]
            self._pos += n
            pad = (4 - (n + 1) % 4) % 4
        else:
            self._require(3)
            raw = self._data[self._pos:self._pos + 3] + b'\x00'
            n = struct.unpack('<I', raw)[0]
            self._pos += 3
            self._require(n)
            data = self._data[self._pos:self._pos + n]
            self._pos += n
            pad = (4 - n % 4) % 4

        self._pos += pad
        return bytes(data)

    def read_string(self) -> str:
        return self.read_bytes().decode('utf-8')

    def read_int128(self) -> bytes:
        self._require(16)
        data = self._data[self._pos:self._pos + 16]
        self._pos += 16
        return bytes(data)

    def read_int256(self) -> bytes:
        self._require(32)
        data = self._data[self._pos:self._pos + 32]
        self._pos += 32
        return bytes(data)

    def read_raw(self, n: int) -> bytes:
        self._require(n)
        data = self._data[self._pos:self._pos + n]
        self._pos += n
        return bytes(data)

    def peek_u32(self) -> int:
        self._require(4)
        val, = struct.unpack_from('<I', self._data, self._pos)
        return val


# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------

class DeserializeError(Exception):
    """Raised when deserialization fails."""


class UnknownConstructorError(DeserializeError):
    """Raised when an unknown constructor ID is encountered."""
    def __init__(self, cid: int) -> None:
        self.cid = cid
        super().__init__(f"Unknown constructor ID: 0x{cid:08x}")


# ---------------------------------------------------------------------------
# Core protocols / base classes
# ---------------------------------------------------------------------------

class Serializable(ABC):
    """
    Anything that can be written to a ByteBuffer.
    All generated types and functions implement this.
    """

    @abstractmethod
    def _serialize(self, buf: ByteBuffer) -> None:
        """Write this object's TL encoding (WITHOUT constructor ID) into buf."""

    def to_bytes(self) -> bytes:
        """Serialize this object to bytes, including constructor ID if applicable."""
        buf = ByteBuffer()
        self._serialize(buf)
        return buf.getvalue()


class Deserializable(ABC):
    """
    Anything that can be reconstructed from a ByteReader.
    All generated types implement this; functions optionally do.
    """

    @classmethod
    @abstractmethod
    def _deserialize(cls: Type[T], reader: ByteReader) -> T:
        """Read this object from reader. Constructor ID already consumed."""

    @classmethod
    def from_bytes(cls: Type[T], data: bytes) -> T:
        reader = ByteReader(data)
        return cls._deserialize(reader)


class TLObject(Serializable, Deserializable):
    """
    Base class for all generated TL types and functions.
    Provides CONSTRUCTOR_ID and helper methods.
    """
    CONSTRUCTOR_ID: ClassVar[int]

    def __init_subclass__(cls, **kwargs: Any) -> None:
        super().__init_subclass__(**kwargs)

    def __repr__(self) -> str:
        fields = ', '.join(
            f"{k}={v!r}"
            for k, v in self.__dict__.items()
            if not k.startswith('_')
        )
        return f"{self.__class__.__name__}({fields})"

    def __eq__(self, other: object) -> bool:
        if type(self) is not type(other):
            return NotImplemented
        return self.__dict__ == other.__dict__  # type: ignore[union-attr]

    def __hash__(self) -> int:
        return hash((type(self), *self.__dict__.values()))


class TLFunction(TLObject):
    """
    Base class for all generated TL functions (RPC calls).

    The RESPONSE_TYPE class variable tells callers what to expect back.
    """
    RESPONSE_TYPE: ClassVar[type]

    def _serialize(self, buf: ByteBuffer) -> None:
        # Functions always write their constructor ID first
        buf.write_u32(self.CONSTRUCTOR_ID)
        self._serialize_params(buf)

    @abstractmethod
    def _serialize_params(self, buf: ByteBuffer) -> None:
        """Write the function parameters (after constructor ID)."""

    @classmethod
    def _deserialize(cls: Type[T], reader: ByteReader) -> T:
        raise NotImplementedError("Functions are not typically deserialized")


# ---------------------------------------------------------------------------
# Serialization helpers for built-in TL types
# ---------------------------------------------------------------------------

def serialize_vector(items: list, buf: ByteBuffer, item_serializer) -> None:
    """Serialize a Vector<T>: constructor ID + length + items."""
    buf.write_u32(0x1CB5C415)  # vector#1cb5c415
    buf.write_i32(len(items))
    for item in items:
        item_serializer(item, buf)


def deserialize_vector(reader: ByteReader, item_deserializer) -> list:
    """Deserialize a Vector<T>."""
    cid = reader.read_u32()
    if cid != 0x1CB5C415:
        raise DeserializeError(f"Expected Vector constructor 0x1cb5c415, got 0x{cid:08x}")
    count = reader.read_i32()
    return [item_deserializer(reader) for _ in range(count)]


def serialize_bare_vector(items: list, buf: ByteBuffer, item_serializer) -> None:
    """Serialize a bare vector (no constructor ID, just length + items)."""
    buf.write_i32(len(items))
    for item in items:
        item_serializer(item, buf)


def deserialize_bare_vector(reader: ByteReader, item_deserializer) -> list:
    """Deserialize a bare vector."""
    count = reader.read_i32()
    return [item_deserializer(reader) for _ in range(count)]
