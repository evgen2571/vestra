"""Palette coloring and deterministic ordered Bayer dithering."""

from __future__ import annotations

from collections.abc import Sequence
from typing import cast

from ..authoring.effects import DitherMatrix, PaletteMode
from ..authoring.values import Color
from ..properties import BindableScalarProperty, ScalarProperty
from .base import Effect


class PaletteMap(Effect):
    """Map source luminance through a dark-to-light palette or rainbow ramp."""

    __slots__ = ()
    effect_type = "palette_map"

    def __init__(
        self,
        palette: Sequence[Color | str] = ("#000000", "#ffffff"),
        *,
        mode: PaletteMode | str = PaletteMode.GRADIENT,
        amount: int | float | ScalarProperty = 1,
        phase: int | float | ScalarProperty = 0,
        period: int | float | None = None,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init(
            {
                "palette": palette,
                "mode": mode,
                "amount": amount,
                "phase": phase,
                "period": period,
            },
            id=id,
        )

    @property
    def palette(self) -> tuple[str, ...]:
        return cast(tuple[str, ...], self._values["palette"])

    @palette.setter
    def palette(self, value: Sequence[Color | str]) -> None:
        self._set_value("palette", value)

    @property
    def mode(self) -> PaletteMode:
        return cast(PaletteMode, self._values["mode"])

    @mode.setter
    def mode(self, value: PaletteMode | str) -> None:
        self._set_value("mode", value)

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
    def period(self) -> float | None:
        return cast(float | None, self._values.get("period"))

    @period.setter
    def period(self, value: int | float | None) -> None:
        self._set_value("period", value)


class OrderedDither(PaletteMap):
    """Quantize source luminance using a stationary ordered Bayer pattern."""

    __slots__ = ()
    effect_type = "ordered_dither"

    def __init__(
        self,
        palette: Sequence[Color | str] = ("#000000", "#ffffff"),
        *,
        mode: PaletteMode | str = PaletteMode.NEAREST,
        amount: int | float | ScalarProperty = 1,
        phase: int | float | ScalarProperty = 0,
        period: int | float | None = None,
        strength: int | float | ScalarProperty = 1,
        matrix: DitherMatrix | str = DitherMatrix.BAYER8,
        scale: int = 1,
        id: str | None = None,
    ) -> None:
        Effect.__init__(self)
        self._init(
            {
                "palette": palette,
                "mode": mode,
                "amount": amount,
                "phase": phase,
                "period": period,
                "strength": strength,
                "matrix": matrix,
                "scale": scale,
            },
            id=id,
        )

    @property
    def strength(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["strength"])

    @strength.setter
    def strength(self, value: int | float | ScalarProperty) -> None:
        self._set_property("strength", value)

    @property
    def matrix(self) -> DitherMatrix:
        return cast(DitherMatrix, self._values["matrix"])

    @matrix.setter
    def matrix(self, value: DitherMatrix | str) -> None:
        self._set_value("matrix", value)

    @property
    def scale(self) -> int:
        return cast(int, self._values["scale"])

    @scale.setter
    def scale(self, value: int) -> None:
        self._set_value("scale", value)
