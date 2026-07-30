"""Constant authoring tracks. Animation is intentionally not exposed here."""

from math import isfinite
from typing import Generic, Self, TypeVar

from ._internal import _Owner
from .values import Crop, Point

T = TypeVar("T")


def _number(value: int | float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    number = float(value)
    if not isfinite(number):
        raise ValueError(f"{name} must be finite")
    return number


class _Track(Generic[T]):
    __slots__ = ("_owner", "_base_value")

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError(f"{type(self).__name__} objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls: type[Self], owner: _Owner, value: T) -> Self:
        instance = object.__new__(cls)
        instance._owner = owner
        instance._base_value = instance._validate(value)
        return instance

    def _initialize(self, owner: _Owner, value: T) -> None:
        self._owner = owner
        self._base_value = self._validate(value)

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

    def to_canonical(self) -> dict[str, object]:
        return {"base_value": self._canonical_value()}

    def __repr__(self) -> str:
        if isinstance(self, CropTrack):
            name = "CropTrack"
        elif isinstance(self, PointTrack):
            name = "PointTrack"
        else:
            name = "ScalarTrack"
        return f"{name}(base_value={self.base_value!r})"


class ScalarTrack(_Track[float]):
    def _validate(self, value: float) -> float:
        return _number(value, "base_value")


class PointTrack(_Track[Point]):
    def _validate(self, value: Point) -> Point:
        if not isinstance(value, Point):
            raise TypeError("base_value must be Point")
        return Point(value.x, value.y)


class CropTrack(_Track[Crop]):
    def _validate(self, value: Crop) -> Crop:
        if not isinstance(value, Crop):
            raise TypeError("base_value must be Crop")
        return Crop(value.x, value.y, value.width, value.height)


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
