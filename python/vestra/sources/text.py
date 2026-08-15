"""Deterministic file-backed text source descriptors."""

from __future__ import annotations

import os
from math import isfinite
from typing import Literal

from ..authoring.values import Color, color_to_canonical
from .base import Source

TextAlignment = Literal["left", "center", "right"]


def _positive(value: int | float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    result = float(value)
    if not isfinite(result) or result <= 0:
        raise ValueError(f"{name} must be finite and positive")
    return result


def _finite(value: int | float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    result = float(value)
    if not isfinite(result):
        raise ValueError(f"{name} must be finite")
    return result


class Text(Source):
    """A static text source using one explicit font file."""

    __slots__ = ("_text", "_font", "_font_size", "_fill", "_align", "_max_width", "_line_spacing", "_letter_spacing")

    def __init__(self, text: str, *, font: str | os.PathLike[str], font_size: int | float,
                 fill: Color | str = "#ffffff", align: TextAlignment = "left",
                 max_width: int | float | None = None, line_spacing: int | float = 1.0,
                 letter_spacing: int | float = 0.0) -> None:
        if not isinstance(text, str):
            raise TypeError("text must be a string")
        try:
            font_value = os.fspath(font)
        except TypeError as error:
            raise TypeError("font must be str or PathLike[str]") from error
        if not isinstance(font_value, str) or not font_value or font_value.isspace():
            raise ValueError("font must be a non-empty file path")
        if align not in {"left", "center", "right"}:
            raise ValueError("align must be 'left', 'center', or 'right'")
        self._text = text
        self._font = font_value
        self._font_size = _positive(font_size, "font_size")
        self._fill = color_to_canonical(fill)
        self._align = align
        self._max_width = None if max_width is None else _positive(max_width, "max_width")
        self._line_spacing = _positive(line_spacing, "line_spacing")
        self._letter_spacing = _finite(letter_spacing, "letter_spacing")

    @property
    def text(self) -> str: return self._text
    @property
    def font(self) -> str: return self._font
    @property
    def font_size(self) -> float: return self._font_size
    @property
    def fill(self) -> str: return self._fill
    @property
    def align(self) -> TextAlignment: return self._align
    @property
    def max_width(self) -> float | None: return self._max_width
    @property
    def line_spacing(self) -> float: return self._line_spacing
    @property
    def letter_spacing(self) -> float: return self._letter_spacing

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {"type": "text", "text": self.text, "font": self.font,
                "font_size": self.font_size, "fill": self.fill, "align": self.align,
                "line_spacing": self.line_spacing, "letter_spacing": self.letter_spacing}
        if self.max_width is not None:
            data["max_width"] = self.max_width
        return data


__all__ = ["Text", "TextAlignment"]
