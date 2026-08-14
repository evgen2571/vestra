"""Typed mutable presentation properties for the high-level editor API.

The editor properties are deliberately independent of a builder.  Lowering
copies their immutable values and keyframes into the owner-bound authoring
tracks, which remain authoritative for canonical projects and rendering.
"""

from __future__ import annotations

from dataclasses import dataclass
from math import isfinite
from typing import Generic, Literal, Self, TypeAlias, TypeVar

from ..authoring.signals import ScalarSignal
from ..authoring.values import CubicBezier, Interpolation

InterpolationValue: TypeAlias = Interpolation | CubicBezier
SignalOperation: TypeAlias = Literal["replace", "add", "multiply"]
T = TypeVar("T")


def _number(value: object, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    number = float(value)
    if not isfinite(number):
        raise ValueError(f"{name} must be finite")
    return number


@dataclass(frozen=True, slots=True)
class PropertyKeyframe(Generic[T]):
    """An immutable typed keyframe authored in layer-local seconds."""

    time: float
    value: T
    interpolation: InterpolationValue = Interpolation.LINEAR

    def __post_init__(self) -> None:
        time = _number(self.time, "time")
        if time < 0:
            raise ValueError("time must be non-negative")
        if not isinstance(self.interpolation, Interpolation | CubicBezier):
            raise TypeError("interpolation must be Interpolation or CubicBezier")
        object.__setattr__(self, "time", time)

    def to_canonical(self, value: object | None = None) -> dict[str, object]:
        candidate = self.value if value is None else value
        if hasattr(candidate, "to_canonical"):
            candidate = candidate.to_canonical()
        return {
            "time": self.time,
            "value": candidate,
            "interpolation": self.interpolation.to_canonical(),
        }


class _Property(Generic[T]):
    __slots__ = ("_value", "_keyframes")

    def __init__(self, value: T) -> None:
        self._value = self._validate(value)
        self._keyframes: list[PropertyKeyframe[T]] = []

    def _validate(self, value: T) -> T:
        return value

    @property
    def value(self) -> T:
        return self._value

    @value.setter
    def value(self, value: T) -> None:
        # Validate before assignment so invalid setters are atomic.
        validated = self._validate(value)
        self._value = validated

    @property
    def base_value(self) -> T:
        return self._value

    @base_value.setter
    def base_value(self, value: T) -> None:
        self.value = value

    @property
    def keyframes(self) -> tuple[PropertyKeyframe[T], ...]:
        return tuple(self._keyframes)

    def keyframe(
        self,
        time: int | float,
        value: T,
        *,
        interpolation: InterpolationValue | None = None,
    ) -> PropertyKeyframe[T]:
        validated = self._validate(value)
        frame = PropertyKeyframe(
            time=time,
            value=validated,
            interpolation=Interpolation.LINEAR
            if interpolation is None
            else interpolation,
        )
        self._keyframes.append(frame)
        return frame

    def clear_keyframes(self) -> None:
        self._keyframes.clear()

    def to_canonical(self) -> dict[str, object]:
        value: object = self.value
        if hasattr(value, "to_canonical"):
            value = value.to_canonical()
        data: dict[str, object] = {"base_value": value}
        if self._keyframes:
            data["keyframes"] = [frame.to_canonical() for frame in self._keyframes]
        return data

    def _copy_to(self, other: "_Property[T]") -> None:
        other.value = self.value
        other._keyframes = list(self._keyframes)


@dataclass(frozen=True, slots=True)
class SignalBinding:
    """An immutable signal modifier attached to a bindable property."""

    signal: ScalarSignal
    operation: SignalOperation = "add"

    def __post_init__(self) -> None:
        if not isinstance(self.signal, ScalarSignal):
            raise TypeError("signal must be ScalarSignal")
        if self.operation not in {"replace", "add", "multiply"}:
            raise ValueError("operation must be replace, add, or multiply")


class ScalarBindingTarget:
    """Modifier-only scalar target for one point-property component.

    The parent point property owns the component's base value and keyframes;
    this target only records ordered audio bindings for that component.
    """

    __slots__ = ("_bindings",)

    def __init__(self) -> None:
        self._bindings: list[SignalBinding] = []

    @property
    def bindings(self) -> tuple[SignalBinding, ...]:
        return tuple(self._bindings)

    def bind(self, signal: ScalarSignal, *, operation: SignalOperation = "add") -> Self:
        self._bindings.append(SignalBinding(signal, operation))
        return self

    def clear_bindings(self) -> None:
        self._bindings.clear()

    def to_canonical(self) -> list[dict[str, object]]:
        return [
            {"operation": item.operation, "signal": item.signal.to_canonical()}
            for item in self._bindings
        ]

    def _copy_to(self, other: "ScalarBindingTarget") -> None:
        other._bindings = list(self._bindings)




__all__ = ["PropertyKeyframe", "SignalBinding", "ScalarBindingTarget"]
