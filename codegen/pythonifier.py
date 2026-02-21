"""
Pythonifier: converts TL names to idiomatic Python identifiers.

Inspired by grammers' rustifier.rs but adapted for Python conventions:
  - Types become PascalCase classes
  - Parameters become snake_case attributes
  - Functions become snake_case in function modules
  - Namespaces become Python sub-packages

Examples
--------
    TL name            →  Python name
    -----------------------------------------
    inputPeerUser      →  InputPeerUser
    access_hash        →  access_hash
    getMessages        →  get_messages  (function)
    upload.getFile     →  upload.GetFile (function class)
    int                →  int
    long               →  int (i64)
    double             →  float
    bytes              →  bytes
    string             →  str
    Bool               →  bool
    Vector<T>          →  list[T]
    int128             →  bytes (16)
    int256             →  bytes (32)
"""

from __future__ import annotations

import re
import keyword
from typing import Optional

from ..tl.parser import Definition, Parameter, TLType, Category


# ---------------------------------------------------------------------------
# Name-conversion core
# ---------------------------------------------------------------------------

def _to_pascal(name: str) -> str:
    """
    Convert a TL name to PascalCase, preserving consecutive uppercase sequences.

    Examples::

        userEmpty          → UserEmpty
        some_OK_name       → SomeOkName
        inputPeerUserSelf  → InputPeerUserSelf
        HTML               → Html
    """
    # Take only the last segment after any final dot
    if '.' in name:
        name = name.rsplit('.', 1)[-1]

    result = []
    # State machine matching grammers' casing logic
    force_upper = True
    prev_upper = False

    for ch in name:
        if ch == '_':
            force_upper = True
            prev_upper = False
            continue
        if force_upper:
            result.append(ch.upper())
            force_upper = False
            prev_upper = ch.isupper()
        elif ch.isupper():
            if not prev_upper:
                result.append(ch)
            else:
                result.append(ch.lower())
            prev_upper = True
        else:
            result.append(ch)
            prev_upper = False

    return ''.join(result)


def _to_snake(name: str) -> str:
    """
    Convert a TL parameter name to snake_case.

    Examples::

        accessHash   → access_hash
        userId       → user_id
        already_fine → already_fine
    """
    # Insert underscore before runs of uppercase followed by lowercase
    s1 = re.sub(r'([A-Z]+)([A-Z][a-z])', r'\1_\2', name)
    s2 = re.sub(r'([a-z\d])([A-Z])', r'\1_\2', s1)
    return s2.lower()


# Python reserved words that need escaping
_RESERVED = frozenset(keyword.kwlist) | {'type', 'id', 'hash'}

def _safe_attr(name: str) -> str:
    """Return a Python-safe attribute name for a TL parameter."""
    snake = _to_snake(name)
    if snake in _RESERVED or keyword.iskeyword(snake):
        return snake + '_'
    # Special Telegram fields
    if name == 'self':
        return 'is_self'
    return snake


# ---------------------------------------------------------------------------
# TL type → Python type annotation
# ---------------------------------------------------------------------------

_BUILTIN_TYPES: dict[str, str] = {
    'int':    'int',
    'long':   'int',
    'int128': 'bytes',
    'int256': 'bytes',
    'double': 'float',
    'string': 'str',
    'bytes':  'bytes',
    'Bool':   'bool',
    'true':   'bool',
    'False':  'bool',
    'True':   'bool',
    '#':      'int',      # flags word
    'vector': 'list',     # bare vector
    'Vector': 'list',
}

# Constructor IDs for built-in bool values (for reference)
BOOL_TRUE_ID  = 0x997275B5
BOOL_FALSE_ID = 0xBC799737


def type_annotation(ty: TLType, *, optional: bool = False) -> str:
    """
    Convert a TLType to a Python type annotation string.

    Parameters
    ----------
    ty:
        The TL type to convert.
    optional:
        If True, wrap the result in ``Optional[...]``.
    """
    result = _type_annotation_inner(ty)
    if optional:
        return f'Optional[{result}]'
    return result


def _type_annotation_inner(ty: TLType) -> str:
    if ty.generic_ref:
        return ty.name  # Generic type variable

    builtin = _BUILTIN_TYPES.get(ty.name)
    if builtin:
        if ty.name in ('vector', 'Vector') and ty.generic_arg:
            inner = _type_annotation_inner(ty.generic_arg)
            return f'list[{inner}]'
        return builtin

    # Custom TL type
    parts = [*ty.namespace, _to_pascal(ty.name)]
    qualified = '.'.join(parts)

    if ty.bare:
        return f"'types.{qualified}'"
    else:
        return f"'enums.{qualified}'"


def param_annotation(param: Parameter) -> str:
    """Return the Python type annotation for a TL parameter."""
    if param.is_flags:
        return 'int'
    return type_annotation(param.ty, optional=param.flag is not None and param.ty.name != 'true')


# ---------------------------------------------------------------------------
# Definition-level names
# ---------------------------------------------------------------------------

class Names:
    """All Python names derived from a single TL Definition."""

    def __init__(self, defn: Definition) -> None:
        self.defn = defn

    @property
    def class_name(self) -> str:
        """PascalCase class name, e.g. ``InputPeerUser``."""
        return _to_pascal(self.defn.name)

    @property
    def namespace_path(self) -> list[str]:
        """Namespace as a list of module path segments."""
        return list(self.defn.namespace)

    @property
    def module_path(self) -> str:
        """Dotted module path for the namespace."""
        return '.'.join(self.defn.namespace) if self.defn.namespace else ''

    @property
    def variant_name(self) -> str:
        """
        Enum variant name when this definition is a member of a boxed type enum.

        Strips the type prefix to avoid repetition, similar to grammers' variant_name.
        Example: ``InputPeerUser`` in ``InputPeer`` enum → variant ``User``.
        """
        cls = self.class_name
        ty_name = _to_pascal(self.defn.ty.name)

        if cls.startswith(ty_name) and len(cls) > len(ty_name):
            suffix = cls[len(ty_name):]
            # Avoid totally-empty or all-numeric variants
            if suffix and not suffix.isdigit():
                return suffix

        return cls

    @property
    def function_name(self) -> str:
        """snake_case name for use as a Python function (for functions category)."""
        return _to_snake(self.defn.name)

    def param_name(self, param: Parameter) -> str:
        return _safe_attr(param.name)

    def param_type(self, param: Parameter) -> str:
        return param_annotation(param)


# ---------------------------------------------------------------------------
# Module-level utilities
# ---------------------------------------------------------------------------

def definitions_to_groups(definitions: list[Definition]) -> dict[str, list[Definition]]:
    """
    Group definitions by their namespace for module generation.
    Returns a dict of ``{namespace_str: [Definition, ...]}``.
    """
    groups: dict[str, list[Definition]] = {}
    for defn in definitions:
        ns = '.'.join(defn.namespace) if defn.namespace else ''
        groups.setdefault(ns, []).append(defn)
    return groups


def enum_members(definitions: list[Definition]) -> dict[str, list[Definition]]:
    """
    Group constructor definitions by their return type name.
    Used to build enum (boxed type) classes.

    Returns ``{TypeName: [Definition, ...]}``.
    """
    groups: dict[str, list[Definition]] = {}
    for defn in definitions:
        if defn.category == Category.TYPES:
            key = defn.ty.full_name()
            groups.setdefault(key, []).append(defn)
    return groups
