"""
TL (Type Language) Parser for Telegram's MTProto schema.

Parses .tl files into structured Definition objects, mirroring the
approach of grammers-tl-parser but in idiomatic Python.

A TL definition looks like:
    ns.name#id {X:Type} flags:# field:flags.0?type = ReturnType;

This module yields Definition objects from .tl content, which the
code generator then transforms into Python classes.
"""

from __future__ import annotations

import re
import struct
from dataclasses import dataclass, field
from enum import Enum, auto
from typing import Iterator, Optional


# ---------------------------------------------------------------------------
# Data model
# ---------------------------------------------------------------------------

class Category(Enum):
    TYPES = auto()      # constructors (left side of tl schema)
    FUNCTIONS = auto()  # methods / RPC calls


@dataclass(frozen=True)
class TLType:
    """A TL type reference, e.g. ``Vector<InputPeer>`` or ``!X``."""
    name: str
    namespace: tuple[str, ...] = ()
    bare: bool = True           # lowercase first char → bare
    generic_ref: bool = False   # prefixed with '!'
    generic_arg: Optional[TLType] = None

    def __str__(self) -> str:
        parts = ".".join((*self.namespace, self.name))
        if self.generic_ref:
            parts = "!" + parts
        if self.generic_arg:
            parts = f"{parts}<{self.generic_arg}>"
        return parts

    def full_name(self) -> str:
        return ".".join((*self.namespace, self.name))


@dataclass(frozen=True)
class Flag:
    """A conditional flag reference like ``flags.3``."""
    name: str    # which flags field
    index: int   # which bit


@dataclass(frozen=True)
class Parameter:
    """A single TL parameter, e.g. ``peer:InputPeer`` or ``timeout:flags.1?int``."""
    name: str
    ty: TLType
    flag: Optional[Flag] = None   # None → always present; Flag → optional bit field
    is_flags: bool = False         # True when type is ``#`` (raw flags word)

    def __str__(self) -> str:
        if self.is_flags:
            return f"{self.name}:#"
        if self.flag:
            return f"{self.name}:{self.flag.name}.{self.flag.index}?{self.ty}"
        return f"{self.name}:{self.ty}"


@dataclass
class Definition:
    """A single parsed TL definition (constructor or function)."""
    name: str
    id: int                          # constructor / method ID (CRC32)
    params: list[Parameter]
    ty: TLType                       # return / base type
    category: Category
    namespace: tuple[str, ...] = ()

    def full_name(self) -> str:
        return ".".join((*self.namespace, self.name))

    def __str__(self) -> str:
        parts = [self.full_name() + f"#{self.id:08x}"]
        for p in self.params:
            parts.append(str(p))
        return " ".join(parts) + f" = {self.ty}"


# ---------------------------------------------------------------------------
# CRC32 / ID inference (same algorithm Telegram uses)
# ---------------------------------------------------------------------------

_CRC32_TABLE: list[int] = []

def _build_crc32_table() -> None:
    for i in range(256):
        c = i
        for _ in range(8):
            c = (0xEDB88320 ^ (c >> 1)) if (c & 1) else (c >> 1)
        _CRC32_TABLE.append(c)

_build_crc32_table()


def _crc32(data: bytes) -> int:
    crc = 0xFFFFFFFF
    for byte in data:
        crc = _CRC32_TABLE[(crc ^ byte) & 0xFF] ^ (crc >> 8)
    return crc ^ 0xFFFFFFFF


def infer_id(definition_str: str) -> int:
    """
    Infer the constructor ID from the TL definition string,
    matching Telegram's CRC32-based algorithm.
    """
    # Strip the explicit #id if present, normalise whitespace
    s = re.sub(r'#[0-9a-fA-F]+', '', definition_str)
    s = re.sub(r'\s+', ' ', s).strip()
    # Remove the semicolon
    s = s.rstrip(';')
    return _crc32(s.encode('ascii')) & 0xFFFFFFFF


# ---------------------------------------------------------------------------
# Type parser
# ---------------------------------------------------------------------------

def _parse_type(raw: str) -> TLType:
    """Parse a raw type string into a TLType."""
    raw = raw.strip()
    if not raw:
        raise ValueError("Empty type string")

    generic_ref = raw.startswith('!')
    if generic_ref:
        raw = raw[1:]

    # Handle generic argument: Foo<Bar>
    generic_arg: Optional[TLType] = None
    if '<' in raw:
        idx = raw.index('<')
        if not raw.endswith('>'):
            raise ValueError(f"Invalid generic in type: {raw!r}")
        inner = raw[idx + 1:-1]
        raw = raw[:idx]
        generic_arg = _parse_type(inner)

    # Namespace
    parts = raw.split('.')
    name = parts[-1]
    namespace = tuple(parts[:-1])

    if not name:
        raise ValueError(f"Empty name in type: {raw!r}")

    bare = name[0].islower() if not generic_ref else name[0].islower()

    return TLType(
        name=name,
        namespace=namespace,
        bare=bare,
        generic_ref=generic_ref,
        generic_arg=generic_arg,
    )


# ---------------------------------------------------------------------------
# Parameter parser
# ---------------------------------------------------------------------------

_FLAG_PATTERN = re.compile(r'^(\w+)\.(\d+)\?(.+)$')
_FLAGS_TYPE = re.compile(r'^#$')


def _parse_parameter(raw: str, known_flags: set[str], known_generics: set[str]) -> Optional[Parameter]:
    """
    Parse a single parameter token. Returns None for type-defs ``{X:Type}``.
    Raises ValueError on invalid input.
    """
    raw = raw.strip()
    if not raw:
        return None

    # Type generic definition {X:Type} - skip, used only for validation
    if raw.startswith('{') and raw.endswith('}'):
        inner = raw[1:-1]
        if ':' in inner:
            gname, gty = inner.split(':', 1)
            if gty.strip() == 'Type':
                known_generics.add(gname.strip())
                return None
        raise ValueError(f"Unknown type-def syntax: {raw!r}")

    if ':' not in raw:
        raise ValueError(f"No colon in parameter: {raw!r}")

    name, ty_str = raw.split(':', 1)
    name = name.strip()
    ty_str = ty_str.strip()

    if not name or not ty_str:
        raise ValueError(f"Empty name or type in parameter: {raw!r}")

    # Flags word: name:#
    if _FLAGS_TYPE.match(ty_str):
        known_flags.add(name)
        return Parameter(name=name, ty=TLType(name='#', bare=True), is_flags=True)

    # Conditional: flags.N?Type
    m = _FLAG_PATTERN.match(ty_str)
    if m:
        flag_name, flag_idx, actual_ty = m.group(1), int(m.group(2)), m.group(3)
        if flag_name not in known_flags:
            raise ValueError(f"Unknown flags field {flag_name!r} in parameter {name!r}")
        ty = _parse_type(actual_ty)
        return Parameter(name=name, ty=ty, flag=Flag(name=flag_name, index=flag_idx))

    # Validate potential generic refs
    ty = _parse_type(ty_str)
    if ty.generic_ref and ty.name not in known_generics:
        raise ValueError(f"Unknown generic ref {ty.name!r} in parameter {name!r}")

    return Parameter(name=name, ty=ty)


# ---------------------------------------------------------------------------
# Definition parser
# ---------------------------------------------------------------------------

def _parse_definition(line: str, category: Category) -> Definition:
    """Parse a single TL definition line into a Definition object."""
    line = line.strip().rstrip(';')

    # Split on '=' to get left side (name + params) and return type
    if '=' not in line:
        raise ValueError(f"No '=' in definition: {line!r}")

    left, right = line.rsplit('=', 1)
    left = left.strip()
    right = right.strip()

    if not right:
        raise ValueError(f"Missing return type in: {line!r}")

    ty = _parse_type(right)

    # Split name#id from parameters
    tokens = left.split()
    if not tokens:
        raise ValueError(f"Empty left side in: {line!r}")

    name_token = tokens[0]
    param_tokens = tokens[1:]

    # Extract explicit id
    explicit_id: Optional[int] = None
    if '#' in name_token:
        name_part, id_str = name_token.split('#', 1)
        if id_str:
            explicit_id = int(id_str, 16)
        name_token = name_part

    # Namespace from name
    name_parts = name_token.split('.')
    name = name_parts[-1]
    namespace = tuple(name_parts[:-1])

    if not name:
        raise ValueError(f"Empty name in: {line!r}")

    # Infer ID if not explicit
    constructor_id = explicit_id if explicit_id is not None else infer_id(line)

    # Parse params
    known_flags: set[str] = set()
    known_generics: set[str] = set()
    params: list[Parameter] = []

    for token in param_tokens:
        param = _parse_parameter(token, known_flags, known_generics)
        if param is not None:
            params.append(param)

    return Definition(
        name=name,
        id=constructor_id,
        params=params,
        ty=ty,
        category=category,
        namespace=namespace,
    )


# ---------------------------------------------------------------------------
# File-level parser
# ---------------------------------------------------------------------------

def parse_tl_file(content: str) -> Iterator[Definition]:
    """
    Parse a complete .tl file, yielding Definition objects.

    Handles:
    - Comments (// ...)
    - Category switches (---types--- / ---functions---)
    - Multi-line definitions joined by semicolons
    - Blank lines and whitespace
    """
    category = Category.TYPES

    # Join logical lines (a definition may span multiple lines before the ';')
    content = re.sub(r'//[^\n]*', '', content)   # strip comments

    # Split into logical statements by semicolons or newlines
    # We accumulate until we see a full definition (contains '=')
    buffer = ''

    for raw_line in content.splitlines():
        line = raw_line.strip()

        if not line:
            if buffer.strip() and '=' in buffer:
                # Flush buffer
                try:
                    yield _parse_definition(buffer, category)
                except Exception:
                    pass
                buffer = ''
            continue

        # Category markers
        if line == '---types---':
            category = Category.TYPES
            buffer = ''
            continue
        if line == '---functions---':
            category = Category.FUNCTIONS
            buffer = ''
            continue

        # Accumulate
        if line.endswith(';'):
            buffer += ' ' + line.rstrip(';')
            if '=' in buffer:
                try:
                    yield _parse_definition(buffer.strip(), category)
                except Exception:
                    pass
            buffer = ''
        else:
            buffer += ' ' + line

    # Flush remaining
    if buffer.strip() and '=' in buffer:
        try:
            yield _parse_definition(buffer.strip(), category)
        except Exception:
            pass
