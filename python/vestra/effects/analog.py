"""Halftone screen printing, stable segmented sorting and CRT display controls."""
from __future__ import annotations
from typing import cast
from ..authoring.effects import HalftoneMode, PixelSortDirection, PixelSortOrder
from ..authoring.values import Color
from ..properties import BindableScalarProperty, ScalarProperty
from .base import Effect

class Halftone(Effect):
    __slots__ = ()
    effect_type = "halftone"

    def __init__(self, *,
        cell_size: int | float | ScalarProperty = 6,
        angle_degrees: int | float | ScalarProperty = 15,
        softness: int | float | ScalarProperty = 0.5,
        amount: int | float | ScalarProperty = 1,
        mode: HalftoneMode | str = HalftoneMode.LUMINANCE,
        foreground: Color | str = '#ffffff',
        background: Color | str = '#000000',
        invert: bool = False,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init({"cell_size": cell_size, "angle_degrees": angle_degrees, "softness": softness, "amount": amount, "mode": mode, "foreground": foreground, "background": background, "invert": invert}, id=id)

    @property
    def cell_size(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["cell_size"])

    @cell_size.setter
    def cell_size(self, value: int | float | ScalarProperty) -> None:
        self._set_property("cell_size", value)

    @property
    def angle_degrees(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["angle_degrees"])

    @angle_degrees.setter
    def angle_degrees(self, value: int | float | ScalarProperty) -> None:
        self._set_property("angle_degrees", value)

    @property
    def softness(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["softness"])

    @softness.setter
    def softness(self, value: int | float | ScalarProperty) -> None:
        self._set_property("softness", value)

    @property
    def amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["amount"])

    @amount.setter
    def amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("amount", value)

    @property
    def mode(self) -> HalftoneMode:
        return cast(HalftoneMode, self._values["mode"])

    @mode.setter
    def mode(self, value: HalftoneMode | str) -> None:
        self._set_value("mode", value)

    @property
    def foreground(self) -> str:
        return cast(str, self._values["foreground"])

    @foreground.setter
    def foreground(self, value: Color | str) -> None:
        self._set_value("foreground", value)

    @property
    def background(self) -> str:
        return cast(str, self._values["background"])

    @background.setter
    def background(self, value: Color | str) -> None:
        self._set_value("background", value)

    @property
    def invert(self) -> bool:
        return cast(bool, self._values["invert"])

    @invert.setter
    def invert(self, value: bool) -> None:
        self._set_value("invert", value)

class PixelSort(Effect):
    __slots__ = ()
    effect_type = "pixel_sort"

    def __init__(self, *,
        lower_threshold: int | float | ScalarProperty = 0.15,
        upper_threshold: int | float | ScalarProperty = 0.9,
        amount: int | float | ScalarProperty = 1,
        direction: PixelSortDirection | str = PixelSortDirection.HORIZONTAL,
        order: PixelSortOrder | str = PixelSortOrder.ASCENDING,
        segment_length: int = 64,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init({"lower_threshold": lower_threshold, "upper_threshold": upper_threshold, "amount": amount, "direction": direction, "order": order, "segment_length": segment_length}, id=id)

    @property
    def lower_threshold(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["lower_threshold"])

    @lower_threshold.setter
    def lower_threshold(self, value: int | float | ScalarProperty) -> None:
        self._set_property("lower_threshold", value)

    @property
    def upper_threshold(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["upper_threshold"])

    @upper_threshold.setter
    def upper_threshold(self, value: int | float | ScalarProperty) -> None:
        self._set_property("upper_threshold", value)

    @property
    def amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["amount"])

    @amount.setter
    def amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("amount", value)

    @property
    def direction(self) -> PixelSortDirection:
        return cast(PixelSortDirection, self._values["direction"])

    @direction.setter
    def direction(self, value: PixelSortDirection | str) -> None:
        self._set_value("direction", value)

    @property
    def order(self) -> PixelSortOrder:
        return cast(PixelSortOrder, self._values["order"])

    @order.setter
    def order(self, value: PixelSortOrder | str) -> None:
        self._set_value("order", value)

    @property
    def segment_length(self) -> int:
        return cast(int, self._values["segment_length"])

    @segment_length.setter
    def segment_length(self, value: int) -> None:
        self._set_value("segment_length", value)

class Crt(Effect):
    __slots__ = ()
    effect_type = "crt"

    def __init__(self, *,
        amount: int | float | ScalarProperty = 1,
        curvature: int | float | ScalarProperty = 0.08,
        scanline_strength: int | float | ScalarProperty = 0.2,
        scanline_spacing: int | float | ScalarProperty = 2,
        mask_strength: int | float | ScalarProperty = 0.15,
        grain: int | float | ScalarProperty = 0.025,
        jitter: int | float | ScalarProperty = 0.35,
        flicker: int | float | ScalarProperty = 0.025,
        rolling_strength: int | float | ScalarProperty = 0.06,
        rolling_width: int | float | ScalarProperty = 0.12,
        phase: int | float | ScalarProperty = 0,
        mask_spacing: int = 1,
        period: int | float | None = None,
        seed: int = 0,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init({"amount": amount, "curvature": curvature, "scanline_strength": scanline_strength, "scanline_spacing": scanline_spacing, "mask_strength": mask_strength, "grain": grain, "jitter": jitter, "flicker": flicker, "rolling_strength": rolling_strength, "rolling_width": rolling_width, "phase": phase, "mask_spacing": mask_spacing, "period": period, "seed": seed}, id=id)

    @property
    def amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["amount"])

    @amount.setter
    def amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("amount", value)

    @property
    def curvature(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["curvature"])

    @curvature.setter
    def curvature(self, value: int | float | ScalarProperty) -> None:
        self._set_property("curvature", value)

    @property
    def scanline_strength(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["scanline_strength"])

    @scanline_strength.setter
    def scanline_strength(self, value: int | float | ScalarProperty) -> None:
        self._set_property("scanline_strength", value)

    @property
    def scanline_spacing(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["scanline_spacing"])

    @scanline_spacing.setter
    def scanline_spacing(self, value: int | float | ScalarProperty) -> None:
        self._set_property("scanline_spacing", value)

    @property
    def mask_strength(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["mask_strength"])

    @mask_strength.setter
    def mask_strength(self, value: int | float | ScalarProperty) -> None:
        self._set_property("mask_strength", value)

    @property
    def grain(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["grain"])

    @grain.setter
    def grain(self, value: int | float | ScalarProperty) -> None:
        self._set_property("grain", value)

    @property
    def jitter(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["jitter"])

    @jitter.setter
    def jitter(self, value: int | float | ScalarProperty) -> None:
        self._set_property("jitter", value)

    @property
    def flicker(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["flicker"])

    @flicker.setter
    def flicker(self, value: int | float | ScalarProperty) -> None:
        self._set_property("flicker", value)

    @property
    def rolling_strength(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["rolling_strength"])

    @rolling_strength.setter
    def rolling_strength(self, value: int | float | ScalarProperty) -> None:
        self._set_property("rolling_strength", value)

    @property
    def rolling_width(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["rolling_width"])

    @rolling_width.setter
    def rolling_width(self, value: int | float | ScalarProperty) -> None:
        self._set_property("rolling_width", value)

    @property
    def phase(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["phase"])

    @phase.setter
    def phase(self, value: int | float | ScalarProperty) -> None:
        self._set_property("phase", value)

    @property
    def mask_spacing(self) -> int:
        return cast(int, self._values["mask_spacing"])

    @mask_spacing.setter
    def mask_spacing(self, value: int) -> None:
        self._set_value("mask_spacing", value)

    @property
    def period(self) -> float | None:
        return cast(float | None, self._values.get("period"))

    @period.setter
    def period(self, value: int | float | None) -> None:
        self._set_value("period", value)

    @property
    def seed(self) -> int:
        return cast(int, self._values["seed"])

    @seed.setter
    def seed(self, value: int) -> None:
        self._set_value("seed", value)
