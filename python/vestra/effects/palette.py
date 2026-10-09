"""Palette coloring and deterministic Bayer/blue-noise dithering."""

from __future__ import annotations

from collections.abc import Sequence
from typing import cast

from ..authoring.effects import DitherMatrix, PaletteMode
from ..authoring.values import Color
from ..properties import BindableScalarProperty, ScalarProperty
from .base import Effect


class PaletteMap(Effect):
    """Map source colors through a palette or quantize RGB channels."""

    __slots__ = ()
    effect_type = "palette_map"

    def __init__(
        self,
        palette: Sequence[Color | str] = ("#000000", "#ffffff"),
        *,
        mode: PaletteMode | str = PaletteMode.GRADIENT,
        levels: int = 4,
        stops: Sequence[int | float] | None = None,
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
                "levels": levels,
                "stops": stops,
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
    def stops(self) -> tuple[float, ...] | None:
        return cast(tuple[float, ...] | None, self._values.get("stops"))

    @stops.setter
    def stops(self, value: Sequence[int | float] | None) -> None:
        self._set_value("stops", value)

    @property
    def levels(self) -> int:
        return cast(int, self._values["levels"])

    @levels.setter
    def levels(self, value: int) -> None:
        self._set_value("levels", value)

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
    """Quantize source colors using a stationary Bayer or blue-noise pattern."""

    __slots__ = ()
    effect_type = "ordered_dither"

    def __init__(
        self,
        palette: Sequence[Color | str] = ("#000000", "#ffffff"),
        *,
        mode: PaletteMode | str = PaletteMode.NEAREST,
        levels: int = 4,
        stops: Sequence[int | float] | None = None,
        amount: int | float | ScalarProperty = 1,
        phase: int | float | ScalarProperty = 0,
        period: int | float | None = None,
        strength: int | float | ScalarProperty = 1,
        matrix: DitherMatrix | str = DitherMatrix.BAYER8,
        scale: int = 1,
        seed: int = 0,
        id: str | None = None,
    ) -> None:
        Effect.__init__(self)
        self._init(
            {
                "palette": palette,
                "mode": mode,
                "levels": levels,
                "stops": stops,
                "amount": amount,
                "phase": phase,
                "period": period,
                "strength": strength,
                "matrix": matrix,
                "scale": scale,
                "seed": seed,
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

    @property
    def seed(self) -> int:
        return cast(int, self._values["seed"])

    @seed.setter
    def seed(self, value: int) -> None:
        self._set_value("seed", value)
