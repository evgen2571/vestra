"""Text source descriptors with bundled, system-family or explicit file fonts."""

from __future__ import annotations

import os
from collections.abc import Sequence
from math import isfinite
from typing import Literal

from ..authoring._internal import _number as _finite
from ..authoring.values import Color, color_to_canonical
from ..properties.scalar import BindableScalarProperty, ScalarProperty
from ..properties.transform import Transform, TransformPoint, TransformScale
from .base import Source
from .text_animation import WordHighlight

TextAlignment = Literal["left", "center", "right"]


def _positive(value: int | float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    result = float(value)
    if not isfinite(result) or result <= 0:
        raise ValueError(f"{name} must be finite and positive")
    return result


def _font_selector(value: str | os.PathLike[str], name: str) -> tuple[str, bool]:
    try:
        selector = os.fspath(value)
    except TypeError as error:
        raise TypeError(f"{name} must be str or PathLike[str]") from error
    if not isinstance(selector, str) or not selector.strip():
        raise ValueError(f"{name} must be a non-empty family name or file path")
    return selector, not isinstance(value, str)


class Text(Source):
    """Text using the bundled default, an installed family, or a font file."""

    __slots__ = (
        "_text", "_font", "_font_is_path", "_fallback_fonts", "_font_size", "_fill",
        "_align", "_max_width", "_line_spacing", "_letter_spacing", "_stroke_color",
        "_stroke_width", "_background_color", "_background_padding", "_reveal", "_word_highlight",
    )

    def __init__(
        self,
        text: str,
        *,
        font: str | os.PathLike[str] | None = None,
        fallback_fonts: Sequence[str | os.PathLike[str]] = (),
        font_size: int | float = 48,
        fill: Color | str = "#ffffff",
        align: TextAlignment = "left",
        max_width: int | float | None = None,
        line_spacing: int | float = 1.0,
        letter_spacing: int | float = 0.0,
        stroke_color: Color | str | None = None,
        stroke_width: int | float = 0.0,
        background_color: Color | str | None = None,
        background_padding: int | float = 0.0,
        reveal: int | float | ScalarProperty = 1.0,
        word_highlight: WordHighlight | None = None,
        transform: Transform | None = None,
        position: TransformPoint | None = None,
        anchor: TransformPoint | None = None,
        scale: TransformScale | None = None,
        rotation: int | float | None = None,
        rotation_degrees: int | float | None = None,
    ) -> None:
        super().__init__(
            transform=transform,
            position=position,
            anchor=anchor,
            scale=scale,
            rotation=rotation,
            rotation_degrees=rotation_degrees,
        )
        if not isinstance(text, str):
            raise TypeError("text must be a string")
        font_value, font_is_path = (None, False) if font is None else _font_selector(font, "font")
        if isinstance(fallback_fonts, str | bytes) or not isinstance(fallback_fonts, Sequence):
            raise TypeError("fallback_fonts must be a sequence of font selectors")
        if align not in {"left", "center", "right"}:
            raise ValueError("align must be 'left', 'center', or 'right'")
        self._text = text
        self._font = font_value
        self._font_is_path = font_is_path
        self._fallback_fonts = tuple(
            _font_selector(value, f"fallback_fonts[{index}]") for index, value in enumerate(fallback_fonts)
        )
        self._font_size = _positive(font_size, "font_size")
        self._fill = color_to_canonical(fill)
        self._align = align
        self._max_width = None if max_width is None else _positive(max_width, "max_width")
        self._line_spacing = _positive(line_spacing, "line_spacing")
        self._letter_spacing = _finite(letter_spacing, "letter_spacing")
        self._stroke_color = None if stroke_color is None else color_to_canonical(stroke_color)
        self._stroke_width = _finite(stroke_width, "stroke_width")
        if self._stroke_width < 0 or (self._stroke_width > 0) != (stroke_color is not None):
            raise ValueError("text stroke requires stroke_color and a positive stroke_width")
        self._background_color = None if background_color is None else color_to_canonical(background_color)
        self._background_padding = _finite(background_padding, "background_padding")
        if self._background_padding < 0 or (self._background_padding > 0 and background_color is None):
            raise ValueError("background_padding must be non-negative and requires background_color")
        self._reveal = ScalarProperty(1.0, minimum=0.0, maximum=1.0)
        if isinstance(reveal, ScalarProperty):
            if isinstance(reveal, BindableScalarProperty) and reveal.bindings:
                raise ValueError("text reveal does not support signal bindings")
            reveal._copy_to(self._reveal)
        else:
            self._reveal.value = reveal
        if word_highlight is not None:
            if not isinstance(word_highlight, WordHighlight):
                raise TypeError("word_highlight must be a WordHighlight")
            if any(word.text_end > len(text) for word in word_highlight.words):
                raise ValueError("word text offsets must be within the text")
        self._word_highlight = word_highlight

    @property
    def text(self) -> str: return self._text
    @property
    def font(self) -> str | None: return self._font
    @property
    def fallback_fonts(self) -> tuple[str, ...]:
        return tuple(selector for selector, _ in self._fallback_fonts)
    @property
    def font_size(self) -> float: return self._font_size
    @property
    def fill(self) -> str: return self._fill
    @property
    def align(self) -> TextAlignment: return self._align
    @property
    def max_width(self) -> float | None: return self._max_width
    @property
    def line_spacing(self) -> float:
        """Line-height multiplier relative to ``font_size``."""
        return self._line_spacing
    @property
    def letter_spacing(self) -> float: return self._letter_spacing

    @property
    def stroke_color(self) -> str | None: return self._stroke_color
    @property
    def stroke_width(self) -> float: return self._stroke_width
    @property
    def background_color(self) -> str | None: return self._background_color
    @property
    def background_padding(self) -> float: return self._background_padding

    @property
    def reveal(self) -> ScalarProperty:
        """Fraction of shaped text clusters shown, keyframeable in clip-local seconds."""
        return self._reveal

    @property
    def word_highlight(self) -> WordHighlight | None:
        return self._word_highlight

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {"type": "text", "text": self.text, "font": self.font,
                "font_size": self.font_size, "fill": self.fill, "align": self.align,
                "line_spacing": self.line_spacing, "letter_spacing": self.letter_spacing}
        if self.max_width is not None:
            data["max_width"] = self.max_width
        if self._fallback_fonts:
            data["fallback_fonts"] = list(self.fallback_fonts)
        if self.stroke_color is not None:
            data["stroke_color"] = self.stroke_color
            data["stroke_width"] = self.stroke_width
        if self.background_color is not None:
            data["background_color"] = self.background_color
            data["background_padding"] = self.background_padding
        if self.reveal.value != 1.0 or self.reveal.keyframes:
            data["reveal"] = self.reveal.to_canonical()
        if self.word_highlight is not None:
            data["word_highlight"] = self.word_highlight.to_canonical()
        return data


__all__ = ["Text", "TextAlignment"]
