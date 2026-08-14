from __future__ import annotations

from typing import cast

from ..properties import BindableScalarProperty, ScalarProperty
from ..authoring.effects import ZoomBlurDirection
from ..authoring.values import Point
from .base import (
    Effect,
)

class GaussianBlur(Effect):
    __slots__ = ()
    effect_type = "gaussian_blur"

    def __init__(
        self, radius: int | float | ScalarProperty, *, id: str | None = None
    ) -> None:
        super().__init__()
        self._init({"radius": radius}, id=id)

    @property
    def radius(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["radius"])

    @radius.setter
    def radius(self, value: int | float | ScalarProperty) -> None:
        self._set_property("radius", value)


class DirectionalBlur(Effect):
    __slots__ = ()
    effect_type = "directional_blur"

    def __init__(
        self,
        radius: int | float | ScalarProperty,
        angle_degrees: int | float | ScalarProperty,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init({"radius": radius, "angle_degrees": angle_degrees}, id=id)

    @property
    def radius(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["radius"])

    @radius.setter
    def radius(self, value: int | float | ScalarProperty) -> None:
        self._set_property("radius", value)

    @property
    def angle_degrees(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["angle_degrees"])

    @angle_degrees.setter
    def angle_degrees(self, value: int | float | ScalarProperty) -> None:
        self._set_property("angle_degrees", value)


class ZoomBlur(Effect):
    __slots__ = ()
    effect_type = "zoom_blur"

    def __init__(
        self,
        radius: int | float | ScalarProperty,
        samples: int,
        anchor: Point | tuple[int | float, int | float],
        direction: ZoomBlurDirection = ZoomBlurDirection.CENTERED,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init(
            {
                "radius": radius,
                "samples": samples,
                "anchor": anchor,
                "direction": direction,
            },
            id=id,
        )

    @property
    def radius(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["radius"])

    @radius.setter
    def radius(self, value: int | float | ScalarProperty) -> None:
        self._set_property("radius", value)

    @property
    def samples(self) -> int:
        return cast(int, self._values["samples"])

    @samples.setter
    def samples(self, value: int) -> None:
        self._set_value("samples", value)

    @property
    def anchor(self) -> Point:
        return cast(Point, self._values["anchor"])

    @anchor.setter
    def anchor(self, value: Point | tuple[int | float, int | float]) -> None:
        self._set_value("anchor", value)

    @property
    def direction(self) -> ZoomBlurDirection:
        return cast(ZoomBlurDirection, self._values["direction"])

    @direction.setter
    def direction(self, value: ZoomBlurDirection | str) -> None:
        self._set_value("direction", value)




__all__ = ["DirectionalBlur", "GaussianBlur", "ZoomBlur"]
