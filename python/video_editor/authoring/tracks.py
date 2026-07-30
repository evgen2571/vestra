"""Stable mutable authoring tracks with immutable keyframe snapshots."""

from __future__ import annotations

from typing import Generic, Protocol, Self, TypeVar, cast

from ._internal import _Owner, _number
from .animation import CropKeyframe, InterpolationValue, PointKeyframe, ScalarKeyframe
from .values import Crop, Point

T = TypeVar("T")


class _CanonicalKeyframe(Protocol):
    def to_canonical(self) -> dict[str, object]: ...


class _Track(Generic[T]):
    __slots__ = ("_owner", "_base_value", "_keyframes")
    _keyframes: list[_CanonicalKeyframe]

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError(f"{type(self).__name__} objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls: type[Self], owner: _Owner, value: T) -> Self:
        instance = object.__new__(cls)
        instance._owner = owner
        instance._base_value = instance._validate(value)
        instance._keyframes = []
        return instance

    def _initialize(self, owner: _Owner, value: T) -> None:
        self._owner = owner
        self._base_value = self._validate(value)
        self._keyframes = []

    @property
    def base_value(self) -> T:
        return self._base_value

    @base_value.setter
    def base_value(self, value: T) -> None:
        self._base_value = self._validate(value)

    def _validate(self, value: T) -> T:
        raise NotImplementedError

    def _canonical_value(self) -> object:
        value = self._base_value
        return value.to_canonical() if isinstance(value, Point | Crop) else value

    @property
    def keyframes(self) -> tuple[object, ...]:
        return tuple(self._keyframes)

    def clear_keyframes(self) -> None:
        self._keyframes.clear()

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {"base_value": self._canonical_value()}
        if self._keyframes:
            data["keyframes"] = [keyframe.to_canonical() for keyframe in self._keyframes]
        return data

    def __repr__(self) -> str:
        if isinstance(self, CropTrack):
            name = "CropTrack"
        elif isinstance(self, PointTrack):
            name = "PointTrack"
        else:
            name = "ScalarTrack"
        return f"{name}(base_value={self.base_value!r}, keyframes={self.keyframes!r})"


class ScalarTrack(_Track[float]):
    def _validate(self, value: float) -> float:
        return _number(value, "base_value")

    @property
    def keyframes(self) -> tuple[ScalarKeyframe, ...]:
        return tuple(cast(ScalarKeyframe, keyframe) for keyframe in self._keyframes)

    def keyframe(self, *, time: int | float, value: int | float,
                 interpolation: InterpolationValue | None = None) -> ScalarKeyframe:
        from .values import Interpolation
        selected: InterpolationValue = Interpolation.LINEAR if interpolation is None else interpolation
        keyframe = ScalarKeyframe(time=time, value=self._validate(value), interpolation=selected)
        self._keyframes.append(keyframe)
        return keyframe


class PointTrack(_Track[Point]):
    def _validate(self, value: Point) -> Point:
        if not isinstance(value, Point):
            raise TypeError("base_value must be Point")
        return Point(value.x, value.y)

    @property
    def keyframes(self) -> tuple[PointKeyframe, ...]:
        return tuple(cast(PointKeyframe, keyframe) for keyframe in self._keyframes)

    def keyframe(self, *, time: int | float, value: Point,
                 interpolation: InterpolationValue | None = None) -> PointKeyframe:
        from .values import Interpolation
        selected: InterpolationValue = Interpolation.LINEAR if interpolation is None else interpolation
        keyframe = PointKeyframe(time=time, value=self._validate(value), interpolation=selected)
        self._keyframes.append(keyframe)
        return keyframe


class CropTrack(_Track[Crop]):
    def _validate(self, value: Crop) -> Crop:
        if not isinstance(value, Crop):
            raise TypeError("base_value must be Crop")
        return Crop(value.x, value.y, value.width, value.height)

    @property
    def keyframes(self) -> tuple[CropKeyframe, ...]:
        return tuple(cast(CropKeyframe, keyframe) for keyframe in self._keyframes)

    def keyframe(self, *, time: int | float, value: Crop,
                 interpolation: InterpolationValue | None = None) -> CropKeyframe:
        from .values import Interpolation
        selected: InterpolationValue = Interpolation.LINEAR if interpolation is None else interpolation
        keyframe = CropKeyframe(time=time, value=self._validate(value), interpolation=selected)
        self._keyframes.append(keyframe)
        return keyframe


class Transform:
    """The static image transform attached to one image clip."""

    __slots__ = ("_owner", "_position", "_anchor", "_scale", "_rotation_degrees")

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("Transform objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls, owner: _Owner) -> "Transform":
        instance = object.__new__(cls)
        instance._owner = owner
        instance._position = PointTrack._create(owner, Point(0.5, 0.5))
        instance._anchor = _AnchorTrack._create(owner, Point(0.5, 0.5))
        instance._scale = _ScaleTrack._create(owner, Point(1.0, 1.0))
        instance._rotation_degrees = ScalarTrack._create(owner, 0.0)
        return instance

    @property
    def position(self) -> PointTrack:
        return self._position

    @property
    def anchor(self) -> PointTrack:
        return self._anchor

    @property
    def scale(self) -> PointTrack:
        return self._scale

    @property
    def rotation_degrees(self) -> ScalarTrack:
        return self._rotation_degrees

    def __repr__(self) -> str:
        return ("Transform(position=" f"{self.position!r}, anchor={self.anchor!r}, "
                f"scale={self.scale!r}, rotation_degrees={self.rotation_degrees!r})")

    def _initialize(self, owner: _Owner) -> None:
        self._owner = owner
        self._position = PointTrack._create(owner, Point(0.5, 0.5))
        self._anchor = _AnchorTrack._create(owner, Point(0.5, 0.5))
        self._scale = _ScaleTrack._create(owner, Point(1.0, 1.0))
        self._rotation_degrees = ScalarTrack._create(owner, 0.0)

    def to_canonical(self) -> dict[str, object]:
        return {
            "position": self.position.to_canonical(),
            "anchor": self.anchor.to_canonical(),
            "scale": self.scale.to_canonical(),
            "rotation_degrees": self.rotation_degrees.to_canonical(),
        }


class _AnchorTrack(PointTrack):
    def _validate(self, value: Point) -> Point:
        point = super()._validate(value)
        if not 0.0 <= point.x <= 1.0 or not 0.0 <= point.y <= 1.0:
            raise ValueError("anchor must be within unit space")
        return point


class _ScaleTrack(PointTrack):
    def _validate(self, value: Point) -> Point:
        point = super()._validate(value)
        if point.x <= 0.0 or point.y <= 0.0:
            raise ValueError("scale must be positive")
        return point
