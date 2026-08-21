"""Point-valued properties and bindings."""

from __future__ import annotations

from typing import Self

from ..authoring.signals import ScalarSignal
from ..authoring.values import Point
from .base import (
    InterpolationValue,
    PropertyKeyframe,
    ScalarBindingTarget,
    SignalBinding,
    SignalOperation,
    _Property,
    _number,
)

class PointProperty(_Property[Point]):
    """A typed point value with optional point keyframes."""

    __slots__ = ()

    def _validate(self, value: Point | tuple[int | float, int | float]) -> Point:
        if isinstance(value, tuple) and len(value) == 2:
            value = Point(_number(value[0], "x"), _number(value[1], "y"))
        if not isinstance(value, Point):
            raise TypeError("value must be Point or a pair of real numbers")
        return Point(value.x, value.y)

    @property
    def value(self) -> Point:
        return super().value

    @value.setter
    def value(self, value: Point | tuple[int | float, int | float]) -> None:
        self._value = self._validate(value)

    def keyframe(
        self,
        time: int | float,
        value: Point | tuple[int | float, int | float],
        *,
        interpolation: InterpolationValue | None = None,
    ) -> PropertyKeyframe[Point]:
        return super().keyframe(
            time, self._validate(value), interpolation=interpolation
        )


class BindablePointProperty(PointProperty):
    """A point property with uniform and per-component signal bindings."""

    __slots__ = ("_bindings", "_x", "_y")

    def __init__(
        self, value: Point | tuple[int | float, int | float] = Point(0.0, 0.0)
    ) -> None:
        self._bindings: list[SignalBinding] = []
        self._x = ScalarBindingTarget()
        self._y = ScalarBindingTarget()
        super().__init__(self._validate(value))

    @property
    def bindings(self) -> tuple[SignalBinding, ...]:
        return tuple(self._bindings)

    def bind(self, signal: ScalarSignal, *, operation: SignalOperation = "add") -> Self:
        binding = SignalBinding(signal, operation)
        self._bindings.append(binding)
        return self

    def clear_bindings(self) -> None:
        self._bindings.clear()

    @property
    def x(self) -> ScalarBindingTarget:
        return self._x

    @property
    def y(self) -> ScalarBindingTarget:
        return self._y

    def to_canonical(self) -> dict[str, object]:
        data = super().to_canonical()
        if self._bindings:
            data["modifiers"] = [
                {"operation": item.operation, "signal": item.signal.to_canonical()}
                for item in self._bindings
            ]
        component_modifiers = {
            name: target.to_canonical()
            for name, target in (("x", self._x), ("y", self._y))
            if target.bindings
        }
        if component_modifiers:
            data["component_modifiers"] = component_modifiers
        return data

    def _copy_to(self, other: "_Property[Point]") -> None:
        super()._copy_to(other)
        if isinstance(other, BindablePointProperty):
            other._bindings = list(self._bindings)
            self._x._copy_to(other._x)
            self._y._copy_to(other._y)




__all__ = ["PointProperty", "BindablePointProperty"]
