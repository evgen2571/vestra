"""Immutable animation values for typed authoring tracks."""

from dataclasses import dataclass

from .tracks import _number
from .values import Crop, CubicBezier, Interpolation, Point

InterpolationValue = Interpolation | CubicBezier


def _interpolation(value: InterpolationValue) -> InterpolationValue:
    if not isinstance(value, Interpolation | CubicBezier):
        raise TypeError("interpolation must be Interpolation or CubicBezier")
    return value


def interpolation_to_canonical(value: InterpolationValue) -> str | dict[str, float | str]:
    return value.to_canonical()


@dataclass(frozen=True, slots=True)
class ScalarKeyframe:
    time: float
    value: float
    interpolation: InterpolationValue = Interpolation.LINEAR

    def __post_init__(self) -> None:
        time = _number(self.time, "time")
        if time < 0.0:
            raise ValueError("time must be non-negative")
        object.__setattr__(self, "time", time)
        object.__setattr__(self, "value", _number(self.value, "value"))
        object.__setattr__(self, "interpolation", _interpolation(self.interpolation))

    def to_canonical(self) -> dict[str, object]:
        return {
            "time": self.time,
            "value": self.value,
            "interpolation": interpolation_to_canonical(self.interpolation),
        }


@dataclass(frozen=True, slots=True)
class PointKeyframe:
    time: float
    value: Point
    interpolation: InterpolationValue = Interpolation.LINEAR

    def __post_init__(self) -> None:
        time = _number(self.time, "time")
        if time < 0.0:
            raise ValueError("time must be non-negative")
        if not isinstance(self.value, Point):
            raise TypeError("value must be Point")
        object.__setattr__(self, "time", time)
        object.__setattr__(self, "value", Point(self.value.x, self.value.y))
        object.__setattr__(self, "interpolation", _interpolation(self.interpolation))

    def to_canonical(self) -> dict[str, object]:
        return {
            "time": self.time,
            "value": self.value.to_canonical(),
            "interpolation": interpolation_to_canonical(self.interpolation),
        }


@dataclass(frozen=True, slots=True)
class CropKeyframe:
    time: float
    value: Crop
    interpolation: InterpolationValue = Interpolation.LINEAR

    def __post_init__(self) -> None:
        time = _number(self.time, "time")
        if time < 0.0:
            raise ValueError("time must be non-negative")
        if not isinstance(self.value, Crop):
            raise TypeError("value must be Crop")
        object.__setattr__(self, "time", time)
        object.__setattr__(self, "value", Crop(
            self.value.x, self.value.y, self.value.width, self.value.height,
        ))
        object.__setattr__(self, "interpolation", _interpolation(self.interpolation))

    def to_canonical(self) -> dict[str, object]:
        return {
            "time": self.time,
            "value": self.value.to_canonical(),
            "interpolation": interpolation_to_canonical(self.interpolation),
        }
