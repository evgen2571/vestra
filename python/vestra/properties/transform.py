"""Crop and grouped transform properties."""

from __future__ import annotations

from typing import Self

from ..authoring.signals import ScalarSignal
from ..authoring.values import Crop, Point
from .base import (
    InterpolationValue,
    PropertyKeyframe,
    ScalarBindingTarget,
    SignalOperation,
    _Property,
    _number,
)
from .point import BindablePointProperty, PointProperty
from .scalar import BindableScalarProperty, ScalarProperty

class _AnchorProperty(PointProperty):
    def _validate(self, value: Point | tuple[int | float, int | float]) -> Point:
        point = super()._validate(value)
        if not 0.0 <= point.x <= 1.0 or not 0.0 <= point.y <= 1.0:
            raise ValueError("anchor must be within unit space")
        return point


class CropProperty(_Property[Crop]):
    """A typed crop rectangle with optional crop keyframes."""

    __slots__ = ("_active",)

    def __init__(self, value: Crop | None = None, *, active: bool = False) -> None:
        self._active = active or value is not None
        super().__init__(Crop(0, 0, 1, 1) if value is None else value)

    def _validate(self, value: Crop) -> Crop:
        if not isinstance(value, Crop):
            raise TypeError("value must be Crop")
        return Crop(value.x, value.y, value.width, value.height)

    @property
    def value(self) -> Crop:
        return super().value

    @value.setter
    def value(self, value: Crop) -> None:
        validated = self._validate(value)
        self._value = validated
        self._active = True

    @property
    def active(self) -> bool:
        return self._active or bool(self._keyframes)

    def keyframe(
        self,
        time: int | float,
        value: Crop,
        *,
        interpolation: InterpolationValue | None = None,
    ) -> PropertyKeyframe[Crop]:
        frame = super().keyframe(time, value, interpolation=interpolation)
        self._active = True
        return frame


class _ScaleProperty(BindablePointProperty):
    def _validate(
        self, value: Point | int | float | tuple[int | float, int | float]
    ) -> Point:
        if isinstance(value, bool):
            raise TypeError("scale must be a real number, Point, or pair")
        if isinstance(value, (int, float)):
            number = _number(value, "scale")
            normalized = Point(number, number)
        elif isinstance(value, tuple) and len(value) == 2:
            normalized = Point(
                _number(value[0], "scale x"), _number(value[1], "scale y")
            )
        elif isinstance(value, Point):
            normalized = value
        else:
            raise TypeError("scale must be a real number, Point, or pair")
        point = super()._validate(normalized)
        if point.x <= 0 or point.y <= 0:
            raise ValueError("scale must be positive")
        return point

    @property
    def value(self) -> Point:
        return super().value

    @value.setter
    def value(
        self, value: Point | int | float | tuple[int | float, int | float]
    ) -> None:
        self._value = self._validate(value)

    base_value = value

    def keyframe(
        self,
        time: int | float,
        value: Point | int | float | tuple[int | float, int | float],
        *,
        interpolation: InterpolationValue | None = None,
    ) -> PropertyKeyframe[Point]:
        return super().keyframe(
            time, self._validate(value), interpolation=interpolation
        )

    def bind(
        self, signal: ScalarSignal, *, operation: SignalOperation = "multiply"
    ) -> Self:
        return super().bind(signal, operation=operation)


class Transform:
    """Layer presentation transform in normalized coordinates and degrees."""

    __slots__ = (
        "_position",
        "_anchor",
        "_scale",
        "_rotation_degrees",
        "_position_x",
        "_position_y",
        "_scale_x",
        "_scale_y",
    )

    def __init__(self) -> None:
        self._position = PointProperty(Point(0.5, 0.5))
        self._anchor = _AnchorProperty(Point(0.5, 0.5))
        self._scale = _ScaleProperty(Point(1.0, 1.0))
        self._rotation_degrees = BindableScalarProperty(0.0)
        self._position_x = ScalarBindingTarget()
        self._position_y = ScalarBindingTarget()
        self._scale_x = ScalarBindingTarget()
        self._scale_y = ScalarBindingTarget()

    @property
    def position(self) -> PointProperty:
        return self._position

    @position.setter
    def position(
        self, value: Point | tuple[int | float, int | float] | PointProperty
    ) -> None:
        if isinstance(value, PointProperty):
            value._copy_to(self._position)
        else:
            self._position.value = value

    @property
    def anchor(self) -> PointProperty:
        return self._anchor

    @anchor.setter
    def anchor(
        self, value: Point | tuple[int | float, int | float] | PointProperty
    ) -> None:
        if isinstance(value, PointProperty):
            value._copy_to(self._anchor)
        else:
            self._anchor.value = value

    @property
    def scale(self) -> _ScaleProperty:
        return self._scale

    @scale.setter
    def scale(
        self,
        value: Point | int | float | tuple[int | float, int | float] | _ScaleProperty,
    ) -> None:
        if isinstance(value, _ScaleProperty):
            value._copy_to(self._scale)
        else:
            self._scale.value = value

    @property
    def position_x(self) -> ScalarBindingTarget:
        """Audio bindings applied to the position's X component."""
        return self._position_x

    @property
    def position_y(self) -> ScalarBindingTarget:
        """Audio bindings applied to the position's Y component."""
        return self._position_y

    @property
    def scale_x(self) -> ScalarBindingTarget:
        """Audio bindings applied to the scale's X component."""
        return self._scale_x

    @property
    def scale_y(self) -> ScalarBindingTarget:
        """Audio bindings applied to the scale's Y component."""
        return self._scale_y

    @property
    def rotation_degrees(self) -> BindableScalarProperty:
        return self._rotation_degrees

    @rotation_degrees.setter
    def rotation_degrees(self, value: int | float | ScalarProperty) -> None:
        if isinstance(value, ScalarProperty):
            value._copy_to(self._rotation_degrees)
            if not isinstance(value, BindableScalarProperty):
                self._rotation_degrees.clear_bindings()
        else:
            self._rotation_degrees.value = value

    @property
    def rotation(self) -> BindableScalarProperty:
        return self.rotation_degrees

    @rotation.setter
    def rotation(self, value: int | float | ScalarProperty) -> None:
        self.rotation_degrees = value

    def is_default(self) -> bool:
        return (
            self.position.value == Point(0.5, 0.5)
            and self.anchor.value == Point(0.5, 0.5)
            and self.scale.value == Point(1.0, 1.0)
            and self.rotation_degrees.value == 0.0
            and not self.position.keyframes
            and not self.anchor.keyframes
            and not self.scale.keyframes
            and not self.rotation_degrees.keyframes
            and not self.scale.bindings
            and not self.rotation_degrees.bindings
            and not self.position_x.bindings
            and not self.position_y.bindings
            and not self.scale_x.bindings
            and not self.scale_y.bindings
        )

    def copy(self) -> "Transform":
        result = Transform()
        self.position._copy_to(result.position)
        self.anchor._copy_to(result.anchor)
        self.scale._copy_to(result.scale)
        self.rotation_degrees._copy_to(result.rotation_degrees)
        self.position_x._copy_to(result.position_x)
        self.position_y._copy_to(result.position_y)
        self.scale_x._copy_to(result.scale_x)
        self.scale_y._copy_to(result.scale_y)
        return result




__all__ = ["CropProperty", "Transform"]
