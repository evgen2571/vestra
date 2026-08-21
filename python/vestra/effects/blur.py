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


class MotionTile(Effect):
    __slots__ = ()
    effect_type = "motion_tile"

    def __init__(
        self,
        output_width_percent: int | float | ScalarProperty,
        output_height_percent: int | float | ScalarProperty,
        tile_center: Point | tuple[int | float, int | float] = (0.5, 0.5),
        mirror_edges: bool = True,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init(
            {
                "output_width_percent": output_width_percent,
                "output_height_percent": output_height_percent,
                "tile_center": tile_center,
                "mirror_edges": mirror_edges,
            },
            id=id,
        )

    @property
    def output_width_percent(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["output_width_percent"])

    @output_width_percent.setter
    def output_width_percent(self, value: int | float | ScalarProperty) -> None:
        self._set_property("output_width_percent", value)

    @property
    def output_height_percent(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["output_height_percent"])

    @output_height_percent.setter
    def output_height_percent(self, value: int | float | ScalarProperty) -> None:
        self._set_property("output_height_percent", value)

    @property
    def tile_center(self) -> Point:
        return cast(Point, self._values["tile_center"])

    @tile_center.setter
    def tile_center(self, value: Point | tuple[int | float, int | float]) -> None:
        self._set_value("tile_center", value)

    @property
    def mirror_edges(self) -> bool:
        return cast(bool, self._values["mirror_edges"])

    @mirror_edges.setter
    def mirror_edges(self, value: bool) -> None:
        self._set_value("mirror_edges", value)


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


class RadialBlur(Effect):
    __slots__ = ()
    effect_type = "radial_blur"

    def __init__(
        self,
        amount: int | float | ScalarProperty,
        center: Point | tuple[int | float, int | float] = (0.5, 0.5),
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init({"amount": amount, "center": center}, id=id)

    @property
    def amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["amount"])

    @amount.setter
    def amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("amount", value)

    @property
    def center(self) -> Point:
        return cast(Point, self._values["center"])

    @center.setter
    def center(self, value: Point | tuple[int | float, int | float]) -> None:
        self._set_value("center", value)




__all__ = ["DirectionalBlur", "GaussianBlur", "MotionTile", "ZoomBlur", "RadialBlur"]
