"""
Code generator for the ``layer`` library.

Reads parsed TL Definitions and generates three Python modules:
  - ``generated/types.py``   — concrete constructor structs
  - ``generated/enums.py``   — boxed type unions (one enum per TL type)
  - ``generated/functions.py`` — RPC function classes

Design mirrors grammers-tl-gen (structs.rs / enums.rs) but outputs
clean, fully-typed Python with dataclasses and __slots__.

Each generated type:
  - Has ``CONSTRUCTOR_ID: int`` class variable
  - Has typed fields matching TL parameters
  - Implements ``_serialize(buf)`` and ``_deserialize(reader)``
  - Has ``__repr__``, ``__eq__``, ``__slots__``

Updating the library is as simple as:
  1. Replace tl/api.tl with the new layer schema
  2. Run ``python -m layer.codegen.generator``
  3. The generated/ files are rewritten automatically
"""

from __future__ import annotations

import textwrap
from collections import defaultdict
from io import StringIO
from pathlib import Path
from typing import Optional

from ..tl.parser import (
    Definition, Parameter, TLType, Category,
    parse_tl_file,
)
from .pythonifier import (
    Names, _to_pascal, _to_snake, _safe_attr,
    type_annotation, param_annotation, enum_members,
    _BUILTIN_TYPES,
)


# ---------------------------------------------------------------------------
# Special-cased types (handled differently by the runtime)
# ---------------------------------------------------------------------------

_SKIP_TYPES: frozenset[str] = frozenset({
    'Bool', 'True', 'False', 'Null',
    'Vector', 'vector',
    'int', 'long', 'double', 'string', 'bytes', 'int128', 'int256',
})


def _should_skip(defn: Definition) -> bool:
    return defn.ty.name in _SKIP_TYPES or defn.name in _SKIP_TYPES


# ---------------------------------------------------------------------------
# Serialization emit helpers
# ---------------------------------------------------------------------------

def _emit_serialize_param(param: Parameter, names: Names, w: StringIO, indent: str) -> None:
    """Emit the _serialize logic for a single parameter."""
    attr = names.param_name(param)
    ty = param.ty

    if param.is_flags:
        # Compute flags word from optional fields
        w.write(f"{indent}# Compute flags\n")
        w.write(f"{indent}flags = 0\n")
        # We'll fix this up when we know sibling params - leave placeholder
        w.write(f"{indent}buf.write_u32(flags)  # placeholder, patched by serialize\n")
        return

    # Optional field - only serialize if present
    if param.flag is not None and ty.name != 'true':
        w.write(f"{indent}if self.{attr} is not None:\n")
        _emit_type_serialize(ty, f"self.{attr}", w, indent + "    ")
        return

    if param.flag is not None and ty.name == 'true':
        return  # bool flags are encoded only in the flags word

    _emit_type_serialize(ty, f"self.{attr}", w, indent)


def _emit_type_serialize(ty: TLType, expr: str, w: StringIO, indent: str) -> None:
    """Emit serialization for a specific type expression."""
    name = ty.name

    if name == '#':
        w.write(f"{indent}buf.write_u32({expr})\n")
    elif name == 'int':
        w.write(f"{indent}buf.write_i32({expr})\n")
    elif name in ('long',):
        w.write(f"{indent}buf.write_i64({expr})\n")
    elif name == 'double':
        w.write(f"{indent}buf.write_f64({expr})\n")
    elif name == 'Bool':
        w.write(f"{indent}buf.write_bool({expr})\n")
    elif name == 'string':
        w.write(f"{indent}buf.write_string({expr})\n")
    elif name in ('bytes',):
        w.write(f"{indent}buf.write_bytes({expr})\n")
    elif name == 'int128':
        w.write(f"{indent}buf.write_int128({expr})\n")
    elif name == 'int256':
        w.write(f"{indent}buf.write_int256({expr})\n")
    elif name == 'Vector':
        if ty.generic_arg:
            inner_ty = ty.generic_arg
            w.write(f"{indent}buf.write_u32(0x1CB5C415)  # Vector constructor\n")
            w.write(f"{indent}buf.write_i32(len({expr}))\n")
            w.write(f"{indent}for _item in {expr}:\n")
            _emit_type_serialize(inner_ty, '_item', w, indent + "    ")
        else:
            w.write(f"{indent}buf.write_u32(0x1CB5C415)\n")
            w.write(f"{indent}buf.write_i32(len({expr}))\n")
    elif name == 'vector':
        # Bare vector — no constructor ID
        w.write(f"{indent}buf.write_i32(len({expr}))\n")
        if ty.generic_arg:
            w.write(f"{indent}for _item in {expr}:\n")
            _emit_type_serialize(ty.generic_arg, '_item', w, indent + "    ")
    else:
        # Custom TL type (types or enums)
        w.write(f"{indent}{expr}._serialize(buf)\n")


def _emit_deserialize_param(ty: TLType, reader_expr: str, w: StringIO, indent: str) -> None:
    """Emit deserialization expression for a type; writes assignment RHS."""
    name = ty.name

    if name == '#' or name == 'int':
        w.write(f"{indent}{reader_expr}.read_i32()")
    elif name == 'long':
        w.write(f"{indent}{reader_expr}.read_i64()")
    elif name == 'double':
        w.write(f"{indent}{reader_expr}.read_f64()")
    elif name == 'Bool':
        w.write(f"{indent}{reader_expr}.read_bool()")
    elif name == 'string':
        w.write(f"{indent}{reader_expr}.read_string()")
    elif name == 'bytes':
        w.write(f"{indent}{reader_expr}.read_bytes()")
    elif name == 'int128':
        w.write(f"{indent}{reader_expr}.read_int128()")
    elif name == 'int256':
        w.write(f"{indent}{reader_expr}.read_int256()")
    elif name == 'true':
        w.write(f"{indent}True")
    elif name == 'Vector':
        if ty.generic_arg:
            inner = _get_deserialize_lambda(ty.generic_arg)
            w.write(f"{indent}_deser_vector({reader_expr}, {inner})")
        else:
            w.write(f"{indent}_deser_vector({reader_expr}, lambda r: r.read_i32())")
    elif name == 'vector':
        if ty.generic_arg:
            inner = _get_deserialize_lambda(ty.generic_arg)
            w.write(f"{indent}_deser_bare_vector({reader_expr}, {inner})")
        else:
            w.write(f"{indent}_deser_bare_vector({reader_expr}, lambda r: r.read_i32())")
    else:
        # Custom TL type
        ns_path = '.'.join([*ty.namespace, _to_pascal(ty.name)])
        if ty.bare:
            w.write(f"{indent}types.{ns_path}._deserialize({reader_expr})")
        else:
            w.write(f"{indent}enums.{ns_path}._deserialize({reader_expr})")


def _get_deserialize_lambda(ty: TLType) -> str:
    """Return a lambda string for deserializing items of the given type."""
    name = ty.name
    if name == 'int':
        return 'lambda r: r.read_i32()'
    if name == 'long':
        return 'lambda r: r.read_i64()'
    if name == 'double':
        return 'lambda r: r.read_f64()'
    if name == 'string':
        return 'lambda r: r.read_string()'
    if name == 'bytes':
        return 'lambda r: r.read_bytes()'
    if name == 'Bool':
        return 'lambda r: r.read_bool()'
    if name == 'int128':
        return 'lambda r: r.read_int128()'
    if name == 'int256':
        return 'lambda r: r.read_int256()'
    ns_path = '.'.join([*ty.namespace, _to_pascal(ty.name)])
    if ty.bare:
        return f'lambda r: types.{ns_path}._deserialize(r)'
    return f'lambda r: enums.{ns_path}._deserialize(r)'


# ---------------------------------------------------------------------------
# Per-definition code generation
# ---------------------------------------------------------------------------

_HEADER = '''\
# This file is AUTO-GENERATED by layer/codegen/generator.py
# DO NOT EDIT MANUALLY — run `python -m layer.codegen.generator` to regenerate.
# Layer: {layer}
# Source: {source}

from __future__ import annotations

from typing import Optional, TYPE_CHECKING
from ..mtproto.core import (
    ByteBuffer, ByteReader, TLObject, TLFunction,
    DeserializeError, UnknownConstructorError,
)

def _deser_vector(reader, item_fn):
    cid = reader.read_u32()
    if cid != 0x1CB5C415:
        raise DeserializeError(f"Expected Vector 0x1cb5c415, got 0x{{cid:08x}}")
    n = reader.read_i32()
    return [item_fn(reader) for _ in range(n)]

def _deser_bare_vector(reader, item_fn):
    n = reader.read_i32()
    return [item_fn(reader) for _ in range(n)]

'''

_TYPES_HEADER = _HEADER + "# ruff: noqa\n# isort: skip_file\n"
_ENUMS_HEADER = _HEADER + "from . import types\n# ruff: noqa\n# isort: skip_file\n"
_FUNCS_HEADER = _HEADER + "from . import types, enums\n# ruff: noqa\n# isort: skip_file\n"


def _param_default(p) -> str:
    """Return the default value string for a parameter in __init__."""
    if p.flag is not None:
        if p.ty.name == 'true':
            return ' = False'
        return ' = None'
    return ''


def _sort_params_for_init(params: list) -> list:
    """Sort params so those with defaults come last (required by Python)."""
    required = [p for p in params if p.flag is None]
    optional = [p for p in params if p.flag is not None]
    return required + optional


def _write_type_class(defn: Definition, w: StringIO) -> None:
    """Emit a complete Python class for a TL constructor."""
    names = Names(defn)
    cls = names.class_name
    # Filter out flag-word params for __slots__
    real_params = [p for p in defn.params if not p.is_flags]
    flag_params = [p for p in defn.params if p.is_flags]

    # Docstring
    w.write(f"\n\nclass {cls}(TLObject):\n")
    w.write(f'    """\n')
    w.write(f'    TL constructor ``{defn.full_name()}``.\n\n')
    w.write(f'    Generated from::\n\n')
    w.write(f'        {defn}\n')
    w.write(f'    """\n')
    w.write(f"    CONSTRUCTOR_ID: int = 0x{defn.id:08X}\n")

    # __slots__
    slots = [repr(names.param_name(p)) for p in real_params]
    w.write(f"    __slots__ = ({', '.join(slots)}{', ' if slots else ''})\n\n")

    # __init__
    init_params = _sort_params_for_init(real_params)
    if init_params:
        w.write(f"    def __init__(self")
        for p in init_params:
            attr = names.param_name(p)
            ann = names.param_type(p)
            default = _param_default(p)
            w.write(f", {attr}: {ann}{default}")
        w.write(f") -> None:\n")
        for p in init_params:
            attr = names.param_name(p)
            w.write(f"        self.{attr} = {attr}\n")
    else:
        w.write(f"    def __init__(self) -> None:\n        pass\n")

    # _serialize
    w.write(f"\n    def _serialize(self, buf: ByteBuffer) -> None:\n")
    if not defn.params:
        w.write(f"        pass\n")
    else:
        # Compute flags words
        flag_fields: dict[str, list[Parameter]] = defaultdict(list)
        for p in real_params:
            if p.flag is not None:
                flag_fields[p.flag.name].append(p)

        for p in defn.params:
            attr = names.param_name(p)
            if p.is_flags:
                # Emit computed flags
                w.write(f"        flags = 0\n")
                for fp in flag_fields.get(p.name, []):
                    fa = names.param_name(fp)
                    if fp.ty.name == 'true':
                        w.write(f"        if self.{fa}: flags |= (1 << {fp.flag.index})\n")
                    else:
                        w.write(f"        if self.{fa} is not None: flags |= (1 << {fp.flag.index})\n")
                w.write(f"        buf.write_u32(flags)\n")
            elif p.flag is not None and p.ty.name == 'true':
                pass  # encoded only in flags
            elif p.flag is not None:
                w.write(f"        if self.{attr} is not None:\n")
                inner = StringIO()
                _emit_type_serialize(p.ty, f"self.{attr}", inner, "            ")
                w.write(inner.getvalue())
            else:
                inner = StringIO()
                _emit_type_serialize(p.ty, f"self.{attr}", inner, "        ")
                w.write(inner.getvalue())

    # _deserialize
    w.write(f"\n    @classmethod\n")
    w.write(f"    def _deserialize(cls, reader: ByteReader) -> '{cls}':\n")
    if not defn.params:
        w.write(f"        return cls()\n")
    else:
        w.write(f"        flags = 0\n")
        result_args = []

        for p in defn.params:
            attr = names.param_name(p)
            if p.is_flags:
                w.write(f"        flags = reader.read_u32()\n")
                result_args.append(None)  # skip in constructor
            elif p.flag is not None and p.ty.name == 'true':
                w.write(f"        {attr} = bool(flags & (1 << {p.flag.index}))\n")
                result_args.append(attr)
            elif p.flag is not None:
                w.write(f"        {attr} = None\n")
                w.write(f"        if flags & (1 << {p.flag.index}):\n")
                inner = StringIO()
                _emit_deserialize_param(p.ty, 'reader', inner, "            ")
                w.write(f"            {attr} = {inner.getvalue().strip()}\n")
                result_args.append(attr)
            else:
                inner = StringIO()
                _emit_deserialize_param(p.ty, 'reader', inner, "")
                w.write(f"        {attr} = {inner.getvalue().strip()}\n")
                result_args.append(attr)

        args_str = ', '.join(a for a in result_args if a is not None)
        w.write(f"        return cls({args_str})\n")


def _write_enum_class(type_name: str, variants: list[Definition], w: StringIO) -> None:
    """Emit a union enum for a boxed TL type."""
    cls = _to_pascal(type_name.rsplit('.', 1)[-1])
    ns = type_name.rsplit('.', 1)[0] if '.' in type_name else ''

    w.write(f"\n\nclass {cls}(TLObject):\n")
    w.write(f'    """\n')
    w.write(f'    Boxed TL type ``{type_name}`` — union of {len(variants)} constructor(s).\n')
    w.write(f'    """\n')
    w.write(f"    CONSTRUCTOR_ID: int = 0  # not a single constructor\n")
    w.write(f"    __slots__ = ('_inner',)\n\n")
    w.write(f"    def __init__(self, inner: TLObject) -> None:\n")
    w.write(f"        self._inner = inner\n\n")
    w.write(f"    def unwrap(self):\n")
    w.write(f"        return self._inner\n\n")

    # Registry
    w.write(f"    _REGISTRY: dict[int, type] = {{\n")
    for v in variants:
        vn = Names(v)
        w.write(f"        0x{v.id:08X}: types.{'.'.join([*v.namespace, vn.class_name])},\n")
    w.write(f"    }}\n\n")

    w.write(f"    def _serialize(self, buf: ByteBuffer) -> None:\n")
    w.write(f"        buf.write_u32(self._inner.CONSTRUCTOR_ID)\n")
    w.write(f"        self._inner._serialize(buf)\n\n")

    w.write(f"    @classmethod\n")
    w.write(f"    def _deserialize(cls, reader: ByteReader) -> '{cls}':\n")
    w.write(f"        cid = reader.read_u32()\n")
    w.write(f"        if cid not in cls._REGISTRY:\n")
    w.write(f"            raise UnknownConstructorError(cid)\n")
    w.write(f"        inner = cls._REGISTRY[cid]._deserialize(reader)\n")
    w.write(f"        return cls(inner)\n\n")

    # Convenience isinstance checks per variant
    for v in variants:
        vn = Names(v)
        variant = vn.variant_name
        inner_path = '.'.join([*v.namespace, vn.class_name])
        prop_name = _to_snake(variant)
        w.write(f"    def is_{prop_name}(self) -> bool:\n")
        w.write(f"        return isinstance(self._inner, types.{inner_path})\n\n")
        w.write(f"    def as_{prop_name}(self) -> Optional[types.{inner_path}]:\n")
        w.write(f"        return self._inner if self.is_{prop_name}() else None\n\n")

    w.write(f"    def __repr__(self) -> str:\n")
    w.write(f"        return f'{cls}({{self._inner!r}})'\n")


def _write_function_class(defn: Definition, w: StringIO) -> None:
    """Emit a TLFunction class for an RPC call."""
    names = Names(defn)
    cls = names.class_name
    real_params = [p for p in defn.params if not p.is_flags]
    flag_params = [p for p in defn.params if p.is_flags]

    ret_type = type_annotation(defn.ty)

    w.write(f"\n\nclass {cls}(TLFunction):\n")
    w.write(f'    """\n')
    w.write(f'    TL function ``{defn.full_name()}``.\n\n')
    w.write(f'    Returns: {ret_type}\n\n')
    w.write(f'    Generated from::\n\n')
    w.write(f'        {defn}\n')
    w.write(f'    """\n')
    w.write(f"    CONSTRUCTOR_ID: int = 0x{defn.id:08X}\n")

    # Return type hint
    ret_cls = _to_pascal(defn.ty.name)
    ret_ns = '.'.join(defn.ty.namespace)
    if defn.ty.name in _BUILTIN_TYPES:
        ret_ann = _BUILTIN_TYPES[defn.ty.name]
    elif defn.ty.bare:
        ret_ann = f"types.{(ret_ns + '.' if ret_ns else '') + ret_cls}"
    else:
        ret_ann = f"enums.{(ret_ns + '.' if ret_ns else '') + ret_cls}"
    w.write(f"    RESPONSE_TYPE = None  # {ret_ann}\n")

    slots = [repr(names.param_name(p)) for p in real_params]
    w.write(f"    __slots__ = ({', '.join(slots)}{', ' if slots else ''})\n\n")

    # __init__
    init_params = _sort_params_for_init(real_params)
    if init_params:
        w.write(f"    def __init__(self")
        for p in init_params:
            attr = names.param_name(p)
            ann = names.param_type(p)
            default = _param_default(p)
            w.write(f", {attr}: {ann}{default}")
        w.write(f") -> None:\n")
        for p in init_params:
            attr = names.param_name(p)
            w.write(f"        self.{attr} = {attr}\n")
    else:
        w.write(f"    def __init__(self) -> None:\n        pass\n")

    # _serialize_params
    w.write(f"\n    def _serialize_params(self, buf: ByteBuffer) -> None:\n")
    if not defn.params:
        w.write(f"        pass\n")
    else:
        flag_fields: dict[str, list[Parameter]] = defaultdict(list)
        for p in real_params:
            if p.flag is not None:
                flag_fields[p.flag.name].append(p)

        for p in defn.params:
            attr = names.param_name(p)
            if p.is_flags:
                w.write(f"        flags = 0\n")
                for fp in flag_fields.get(p.name, []):
                    fa = names.param_name(fp)
                    if fp.ty.name == 'true':
                        w.write(f"        if self.{fa}: flags |= (1 << {fp.flag.index})\n")
                    else:
                        w.write(f"        if self.{fa} is not None: flags |= (1 << {fp.flag.index})\n")
                w.write(f"        buf.write_u32(flags)\n")
            elif p.flag is not None and p.ty.name == 'true':
                pass
            elif p.flag is not None:
                w.write(f"        if self.{attr} is not None:\n")
                inner = StringIO()
                _emit_type_serialize(p.ty, f"self.{attr}", inner, "            ")
                w.write(inner.getvalue())
            else:
                inner = StringIO()
                _emit_type_serialize(p.ty, f"self.{attr}", inner, "        ")
                w.write(inner.getvalue())


# ---------------------------------------------------------------------------
# Generator entry point
# ---------------------------------------------------------------------------

class Config:
    """Configuration for the code generator."""
    def __init__(
        self,
        *,
        include_mtproto: bool = False,
        include_api: bool = True,
    ) -> None:
        self.include_mtproto = include_mtproto
        self.include_api = include_api


def generate(
    tl_dir: Path,
    output_dir: Path,
    config: Optional[Config] = None,
) -> None:
    """
    Generate Python source files from .tl schemas.

    Parameters
    ----------
    tl_dir:
        Directory containing ``api.tl`` and optionally ``mtproto.tl``.
    output_dir:
        Directory to write ``types.py``, ``enums.py``, ``functions.py``.
    config:
        Optional configuration object.
    """
    config = config or Config()
    output_dir.mkdir(parents=True, exist_ok=True)

    all_defs: list[Definition] = []

    # Parse schema files
    schemas = []
    if config.include_api:
        schemas.append(('api.tl', tl_dir / 'api.tl'))
    if config.include_mtproto:
        schemas.append(('mtproto.tl', tl_dir / 'mtproto.tl'))

    layer_num = 0
    for source_name, path in schemas:
        content = path.read_text(encoding='utf-8')
        # Extract layer constant
        for line in content.splitlines():
            if line.strip().startswith('// LAYER'):
                try:
                    layer_num = int(line.split()[-1])
                except ValueError:
                    pass
                break
        defs = list(parse_tl_file(content))
        all_defs.extend(defs)

    types_defs = [d for d in all_defs if d.category == Category.TYPES and not _should_skip(d)]
    func_defs  = [d for d in all_defs if d.category == Category.FUNCTIONS and not _should_skip(d)]

    # --- Write types.py ---
    types_out = StringIO()
    types_out.write(_TYPES_HEADER.format(layer=layer_num, source='api.tl'))
    for defn in types_defs:
        _write_type_class(defn, types_out)
    # Emit namespace proxy objects so types.account.Foo works
    namespaces: dict[str, list[str]] = {}
    for defn in types_defs:
        ns = '.'.join(defn.namespace)
        if ns:
            cls_name = Names(defn).class_name
            namespaces.setdefault(ns, []).append(cls_name)

    if namespaces:
        types_out.write("\n\n# Namespace proxy objects\n")
        types_out.write("class _NS:\n")
        types_out.write("    pass\n\n")
        for ns, clslist in namespaces.items():
            # Create one proxy per top-level namespace segment
            parts = ns.split('.')
            obj_name = parts[0]
        # Emit one _NS object per unique top-level namespace
        top_ns: dict[str, list[tuple[str, str]]] = {}
        for defn in types_defs:
            if defn.namespace:
                top = defn.namespace[0]
                cls_name = Names(defn).class_name
                top_ns.setdefault(top, []).append((cls_name, cls_name))
        for ns_name, members in top_ns.items():
            types_out.write(f"{ns_name} = _NS()\n")
            for _, cls_name in members:
                types_out.write(f"{ns_name}.{cls_name} = {cls_name}\n")
        types_out.write("\n")

    (output_dir / 'types.py').write_text(types_out.getvalue(), encoding='utf-8')
    print(f"  ✓ types.py  ({len(types_defs)} constructors)")

    # --- Write enums.py ---
    enums_out = StringIO()
    enums_out.write(_ENUMS_HEADER.format(layer=layer_num, source='api.tl'))

    boxed_types = enum_members(types_defs)
    # Only emit enums with more than one variant (or where type ≠ constructor name)
    emitted = 0
    for type_full_name, variants in sorted(boxed_types.items()):
        _write_enum_class(type_full_name, variants, enums_out)
        emitted += 1
    (output_dir / 'enums.py').write_text(enums_out.getvalue(), encoding='utf-8')
    print(f"  ✓ enums.py  ({emitted} boxed types)")

    # --- Write functions.py ---
    funcs_out = StringIO()
    funcs_out.write(_FUNCS_HEADER.format(layer=layer_num, source='api.tl'))
    for defn in func_defs:
        _write_function_class(defn, funcs_out)
    (output_dir / 'functions.py').write_text(funcs_out.getvalue(), encoding='utf-8')
    print(f"  ✓ functions.py ({len(func_defs)} functions)")

    # --- Write __init__.py ---
    init_code = f"""\
# Auto-generated. Layer {layer_num}.
LAYER = {layer_num}

from . import types, enums, functions
from .types import *
"""
    (output_dir / '__init__.py').write_text(init_code, encoding='utf-8')
    print(f"  ✓ __init__.py (LAYER={layer_num})")
