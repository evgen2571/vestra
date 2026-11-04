"""Resolve authoring fonts to ordinary file assets without renderer fallback."""

from __future__ import annotations

from functools import cache
import hashlib
import os
from pathlib import Path
import struct
import sys
import tempfile

from . import _native


def resolve_font(font: str | None, *, explicit_path: bool, base_directory: Path) -> str:
    if font is None:
        return str(Path(__file__).parent / "fonts" / "VestraDefault.ttf")
    if (explicit_path or "/" in font or "\\" in font or Path(font).suffix
            or (base_directory / font).is_file()):
        return font
    path, index = _native._resolve_system_font(font)
    return _system_font_file(Path(path), index)


def _checksum(data: bytes | bytearray) -> int:
    padded = data + b"\0" * (-len(data) % 4)
    return sum(int(word[0]) for word in struct.iter_unpack(">I", padded)) & 0xFFFFFFFF


def _extract_face(data: bytes, index: int) -> bytes:
    """Rebase one TTC/OTC face's shared tables into a standalone sfnt."""
    try:
        if data[:4] != b"ttcf" or not 0 <= index < struct.unpack_from(">I", data, 8)[0]:
            raise ValueError("invalid collection face index")
        offset = struct.unpack_from(">I", data, 12 + index * 4)[0]
        version, count = struct.unpack_from(">4sH", data, offset)
        if version not in (b"\0\1\0\0", b"OTTO", b"true") or count == 0:
            raise ValueError("unsupported collection face")
        tables = []
        for number in range(count):
            tag, _, start, length = struct.unpack_from(">4sIII", data, offset + 12 + number * 16)
            if start + length > len(data):
                raise ValueError("collection table extends beyond the file")
            if tag != b"DSIG":  # Extracting invalidates the collection's digital signature.
                table = data[start:start + length]
                if tag == b"head":
                    if length < 12:
                        raise ValueError("invalid font head table")
                    table = table[:8] + b"\0" * 4 + table[12:]
                tables.append((tag, table))
        tables.sort()
        count = len(tables)
        power = 1 << (count.bit_length() - 1)
        output = bytearray(struct.pack(">4sHHHH", version, count, power * 16,
                                       power.bit_length() - 1, count * 16 - power * 16))
        output.extend(b"\0" * (count * 16))
        head = None
        for number, (tag, table) in enumerate(tables):
            start = len(output)
            struct.pack_into(">4sIII", output, 12 + number * 16, tag, _checksum(table), start, len(table))
            if tag == b"head":
                head = start
            output.extend(table)
            output.extend(b"\0" * (-len(output) % 4))
        if head is None:
            raise ValueError("font has no head table")
        struct.pack_into(">I", output, head + 8, (0xB1B0AFBA - _checksum(output)) & 0xFFFFFFFF)
        return bytes(output)
    except (struct.error, OverflowError) as error:
        raise ValueError("invalid or truncated font collection") from error


def _cache_directory() -> Path:
    if sys.platform == "win32":
        root = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData" / "Local"))
    elif sys.platform == "darwin":
        root = Path.home() / "Library" / "Caches"
    else:
        root = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))
    return root / "vestra" / "fonts"


@cache
def _system_font_file(path: Path, index: int) -> str:
    # Ordinary files remain file-backed; only collections need a standalone copy.
    with path.open("rb") as font:
        if font.read(4) != b"ttcf":
            if index != 0:
                raise ValueError(f"unsupported face {index} in font file {path}")
            return str(path)
        font.seek(0)
        extracted = _extract_face(font.read(), index)
    directory = _cache_directory()
    directory.mkdir(parents=True, exist_ok=True)
    target = directory / (hashlib.sha256(extracted).hexdigest() + ".otf")
    if not target.is_file():
        temporary: str | None = None
        try:
            with tempfile.NamedTemporaryFile(dir=directory, delete=False) as output:
                temporary = output.name
                output.write(extracted)
            os.replace(temporary, target)
        finally:
            if temporary is not None:
                Path(temporary).unlink(missing_ok=True)
    return str(target)
