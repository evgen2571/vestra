"""Typed mutable presentation properties for the high-level editor API.

The editor properties are deliberately independent of a builder.  Lowering
copies their immutable values and keyframes into the owner-bound authoring
tracks, which remain authoritative for canonical projects and rendering.
"""

from __future__ import annotations

from dataclasses import dataclass
from math import isfinite
from typing import Generic, Literal, Self, TypeAlias, TypeVar

from .authoring.signals import ScalarSignal
from .authoring.values import BlendMode, Crop, CubicBezier, Interpolation, Point

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


class ScalarProperty(_Property[float]):
    """A finite scalar value with optional scalar keyframes."""

    __slots__ = ("_minimum", "_maximum", "_minimum_exclusive", "_maximum_exclusive")

    def __init__(
        self,
        value: int | float = 0.0,
        *,
        minimum: float | None = None,
        maximum: float | None = None,
        minimum_exclusive: bool = False,
        maximum_exclusive: bool = False,
    ) -> None:
        self._minimum = minimum
        self._maximum = maximum
        self._minimum_exclusive = minimum_exclusive
        self._maximum_exclusive = maximum_exclusive
        super().__init__(value)

    def _validate(self, value: float | int) -> float:
        result = _number(value, "value")
        if self._minimum is not None and (
            result <= self._minimum
            if self._minimum_exclusive
            else result < self._minimum
        ):
            raise ValueError("value is below its minimum")
        if self._maximum is not None and (
            result >= self._maximum
            if self._maximum_exclusive
            else result > self._maximum
        ):
            raise ValueError("value is above its maximum")
        return result


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


class BindableScalarProperty(ScalarProperty):
    """A scalar property that accepts ordered immutable audio bindings."""

    __slots__ = ("_bindings",)

    def __init__(
        self,
        value: int | float = 0.0,
        *,
        minimum: float | None = None,
        maximum: float | None = None,
        minimum_exclusive: bool = False,
        maximum_exclusive: bool = False,
    ) -> None:
        self._bindings: list[SignalBinding] = []
        super().__init__(
            value,
            minimum=minimum,
            maximum=maximum,
            minimum_exclusive=minimum_exclusive,
            maximum_exclusive=maximum_exclusive,
        )

    @property
    def bindings(self) -> tuple[SignalBinding, ...]:
        return tuple(self._bindings)

    def bind(self, signal: ScalarSignal, *, operation: SignalOperation = "add") -> Self:
        binding = SignalBinding(signal, operation)
        self._bindings.append(binding)
        return self

    def clear_bindings(self) -> None:
        self._bindings.clear()

    def to_canonical(self) -> dict[str, object]:
        data = super().to_canonical()
        if self._bindings:
            data["bindings"] = [
                {"operation": item.operation, "signal": item.signal.to_canonical()}
                for item in self._bindings
            ]
        return data

    def _copy_to(self, other: "_Property[float]") -> None:
        super()._copy_to(other)
        if isinstance(other, BindableScalarProperty):
            other._bindings = list(self._bindings)


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
    """A point property with ordered scalar bindings for uniform components."""

    __slots__ = ("_bindings",)

    def __init__(
        self, value: Point | tuple[int | float, int | float] = Point(0.0, 0.0)
    ) -> None:
        self._bindings: list[SignalBinding] = []
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

    def to_canonical(self) -> dict[str, object]:
        data = super().to_canonical()
        if self._bindings:
            data["bindings"] = [
                {"operation": item.operation, "signal": item.signal.to_canonical()}
                for item in self._bindings
            ]
        return data

    def _copy_to(self, other: "_Property[Point]") -> None:
        super()._copy_to(other)
        if isinstance(other, BindablePointProperty):
            other._bindings = list(self._bindings)


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


__all__ = [
    "ScalarProperty",
    "PointProperty",
    "CropProperty",
    "PropertyKeyframe",
    "Transform",
    "BindableScalarProperty",
    "BindablePointProperty",
    "ScalarBindingTarget",
    "SignalBinding",
    "SignalOperation",
    "Interpolation",
    "CubicBezier",
    "Point",
    "Crop",
    "BlendMode",
]
