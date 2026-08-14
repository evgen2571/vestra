from __future__ import annotations

from typing import cast

from ..properties import BindableScalarProperty, ScalarProperty
from .base import (
    Effect,
)

class MotionBlur(Effect):
    __slots__ = ()
    effect_type = "motion_blur"

    def __init__(
        self,
        intensity: int | float | ScalarProperty,
        shutter_angle: int | float | ScalarProperty,
        max_radius: int | float | ScalarProperty,
        samples: int,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init(
            {
                "intensity": intensity,
                "shutter_angle": shutter_angle,
                "max_radius": max_radius,
                "samples": samples,
            },
            id=id,
        )

    @property
    def intensity(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["intensity"])

    @intensity.setter
    def intensity(self, value: int | float | ScalarProperty) -> None:
        self._set_property("intensity", value)

    @property
    def shutter_angle(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["shutter_angle"])

    @shutter_angle.setter
    def shutter_angle(self, value: int | float | ScalarProperty) -> None:
        self._set_property("shutter_angle", value)

    @property
    def max_radius(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["max_radius"])

    @max_radius.setter
    def max_radius(self, value: int | float | ScalarProperty) -> None:
        self._set_property("max_radius", value)

    @property
    def samples(self) -> int:
        return cast(int, self._values["samples"])

    @samples.setter
    def samples(self, value: int) -> None:
        self._set_value("samples", value)




__all__ = ["MotionBlur"]
