"""Area-filtered cinematic ASCII and antialiased geometric pseudo-ASCII."""
from __future__ import annotations
from collections.abc import Sequence
from os import PathLike
from typing import cast
from ..authoring.effects import AsciiColorMode, AsciiGlyphStyle, AsciiMode
from ..authoring.values import Color
from ..properties import BindableScalarProperty, ScalarProperty
from .base import Effect

CHARACTER_SETS = {
    "standard": " .:-=+*#%@",
    "dense": ' .\'`^",:;Il!i~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$',
    "blocks": " ░▒▓█",
}

class Ascii(Effect):
    """Analyze output-anchored cells using alpha-aware area means and gradients.

    Character order is authored dark-to-light; scalar properties accept keyframes
    and audio signals. Each Unicode scalar is independent (no shaping/fallback).
    Fonts are explicit paths; omission uses the bundled licensed DejaVu face.
    """
    __slots__ = ()
    effect_type = "ascii"

    def __init__(self, characters: str = CHARACTER_SETS["standard"], *,
        font: str | PathLike[str] | None = None, edge_characters: str = "-|/\\",
        cell_width: int | float | ScalarProperty = 8, cell_height: int | float | ScalarProperty = 12,
        mode: AsciiMode | str = AsciiMode.HYBRID, glyph_style: AsciiGlyphStyle | str = AsciiGlyphStyle.CHARACTERS,
        color_mode: AsciiColorMode | str = AsciiColorMode.MONOCHROME,
        foreground: Color | str = "#ffffff", background: Color | str = "#000000",
        palette: Sequence[Color | str] = ("#000000", "#ffffff"),
        phase: int | float | ScalarProperty = 0, period: int | float | None = None,
        invert: bool = False, edge_threshold: int | float | ScalarProperty = .15,
        edge_strength: int | float | ScalarProperty = 1, source_mix: int | float | ScalarProperty = 0,
        amount: int | float | ScalarProperty = 1, id: str | None = None,
    ) -> None:
        super().__init__()
        self._init({
            "characters": characters, "font": font, "edge_characters": edge_characters,
            "cell_width": cell_width, "cell_height": cell_height, "mode": mode,
            "glyph_style": glyph_style, "color_mode": color_mode, "foreground": foreground,
            "background": background, "palette": palette, "phase": phase, "period": period,
            "invert": invert, "edge_threshold": edge_threshold, "edge_strength": edge_strength,
            "source_mix": source_mix, "amount": amount,
        }, id=id)

    @property
    def cell_width(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["cell_width"])
    @cell_width.setter
    def cell_width(self, value: int | float | ScalarProperty) -> None:
        self._set_property("cell_width", value)

    @property
    def cell_height(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["cell_height"])
    @cell_height.setter
    def cell_height(self, value: int | float | ScalarProperty) -> None:
        self._set_property("cell_height", value)

    @property
    def edge_threshold(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["edge_threshold"])
    @edge_threshold.setter
    def edge_threshold(self, value: int | float | ScalarProperty) -> None:
        self._set_property("edge_threshold", value)

    @property
    def edge_strength(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["edge_strength"])
    @edge_strength.setter
    def edge_strength(self, value: int | float | ScalarProperty) -> None:
        self._set_property("edge_strength", value)

    @property
    def source_mix(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["source_mix"])
    @source_mix.setter
    def source_mix(self, value: int | float | ScalarProperty) -> None:
        self._set_property("source_mix", value)

    @property
    def amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["amount"])
    @amount.setter
    def amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("amount", value)

    @property
    def phase(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["phase"])
    @phase.setter
    def phase(self, value: int | float | ScalarProperty) -> None:
        self._set_property("phase", value)

    @property
    def characters(self) -> str:
        return cast(str, self._values.get("characters"))
    @characters.setter
    def characters(self, value: str) -> None:
        self._set_value("characters", value)

    @property
    def edge_characters(self) -> str:
        return cast(str, self._values.get("edge_characters"))
    @edge_characters.setter
    def edge_characters(self, value: str) -> None:
        self._set_value("edge_characters", value)

    @property
    def font(self) -> str | PathLike[str] | None:
        return cast(str | PathLike[str] | None, self._values.get("font"))
    @font.setter
    def font(self, value: str | PathLike[str] | None) -> None:
        self._set_value("font", value)

    @property
    def glyph_style(self) -> AsciiGlyphStyle:
        return cast(AsciiGlyphStyle, self._values.get("glyph_style"))
    @glyph_style.setter
    def glyph_style(self, value: AsciiGlyphStyle | str) -> None:
        self._set_value("glyph_style", value)

    @property
    def mode(self) -> AsciiMode:
        return cast(AsciiMode, self._values.get("mode"))
    @mode.setter
    def mode(self, value: AsciiMode | str) -> None:
        self._set_value("mode", value)

    @property
    def color_mode(self) -> AsciiColorMode:
        return cast(AsciiColorMode, self._values.get("color_mode"))
    @color_mode.setter
    def color_mode(self, value: AsciiColorMode | str) -> None:
        self._set_value("color_mode", value)

    @property
    def foreground(self) -> str:
        return cast(str, self._values.get("foreground"))
    @foreground.setter
    def foreground(self, value: Color | str) -> None:
        self._set_value("foreground", value)

    @property
    def background(self) -> str:
        return cast(str, self._values.get("background"))
    @background.setter
    def background(self, value: Color | str) -> None:
        self._set_value("background", value)

    @property
    def palette(self) -> tuple[str, ...]:
        return cast(tuple[str, ...], self._values.get("palette"))
    @palette.setter
    def palette(self, value: Sequence[Color | str]) -> None:
        self._set_value("palette", value)

    @property
    def invert(self) -> bool:
        return cast(bool, self._values.get("invert"))
    @invert.setter
    def invert(self, value: bool) -> None:
        self._set_value("invert", value)

    @property
    def period(self) -> float | None:
        return cast(float | None, self._values.get("period"))
    @period.setter
    def period(self, value: int | float | None) -> None:
        self._set_value("period", value)

class PseudoAscii(Ascii):
    """Geometric dot/line/cross ASCII with the same analysis and color controls."""
    __slots__ = ()

    def __init__(self, characters: str = CHARACTER_SETS["standard"], *,
        font: str | PathLike[str] | None = None, edge_characters: str = "-|/\\",
        cell_width: int | float | ScalarProperty = 8, cell_height: int | float | ScalarProperty = 12,
        mode: AsciiMode | str = AsciiMode.HYBRID,
        color_mode: AsciiColorMode | str = AsciiColorMode.MONOCHROME,
        foreground: Color | str = "#ffffff", background: Color | str = "#000000",
        palette: Sequence[Color | str] = ("#000000", "#ffffff"),
        phase: int | float | ScalarProperty = 0, period: int | float | None = None,
        invert: bool = False, edge_threshold: int | float | ScalarProperty = .15,
        edge_strength: int | float | ScalarProperty = 1, source_mix: int | float | ScalarProperty = 0,
        amount: int | float | ScalarProperty = 1, id: str | None = None,
    ) -> None:
        super().__init__(characters, font=font, edge_characters=edge_characters,
            cell_width=cell_width, cell_height=cell_height, mode=mode,
            glyph_style=AsciiGlyphStyle.GEOMETRIC, color_mode=color_mode,
            foreground=foreground, background=background, palette=palette,
            phase=phase, period=period, invert=invert, edge_threshold=edge_threshold,
            edge_strength=edge_strength, source_mix=source_mix, amount=amount, id=id)
