"""Stable mutable authoring tracks with immutable keyframe snapshots."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Generic, Literal, Protocol, Self, TypeVar, cast

from ._internal import _Owner, _number
from .animation import CropKeyframe, InterpolationValue, PointKeyframe, ScalarKeyframe
from .values import Crop, Point

if TYPE_CHECKING:
    from .signals import ScalarSignal

T = TypeVar("T")


class _CanonicalKeyframe(Protocol):
    def to_canonical(self) -> dict[str, object]: ...


class _Track(Generic[T]):
    __slots__ = ("_owner", "_base_value", "_keyframes", "_modifiers")
    _keyframes: list[_CanonicalKeyframe]
    _modifiers: list[dict[str, Any]]

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError(f"{type(self).__name__} objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls: type[Self], owner: _Owner, value: T) -> Self:
        instance = object.__new__(cls)
        instance._owner = owner
        instance._base_value = instance._validate(value)
        instance._keyframes = []
        instance._modifiers = []
        return instance

    def _initialize(self, owner: _Owner, value: T) -> None:
        self._owner = owner
        self._base_value = self._validate(value)
        self._keyframes = []
        self._modifiers = []

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
        if self._modifiers:
            data["modifiers"] = [
                {"operation": item["operation"], "signal": item["signal"].to_canonical()}
                for item in self._modifiers
            ]
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


class ModulatableScalarTrack(ScalarTrack):
    """An authored scalar track that can also receive ordered signal modifiers."""

    def modulate(self, signal: "ScalarSignal", *, mode: Literal["replace", "add", "multiply"] = "add") -> Self:
        from .signals import ScalarSignal
        if not isinstance(signal, ScalarSignal):
            raise TypeError("signal must be ScalarSignal")
        if mode not in {"replace", "add", "multiply"}:
            raise ValueError("mode must be replace, add, or multiply")
        self._modifiers.append({"operation": mode, "signal": signal})
        return self

    react_to = modulate


class ScalarModifierTarget:
    """Modifier-only scalar target for one component of an authored point track.

    Position and scale X/Y animation remains owned by the parent ``PointTrack``;
    this target deliberately exposes no base value or keyframe API.
    """

    __slots__ = ("_owner", "_modifiers")
    _owner: _Owner
    _modifiers: list[dict[str, Any]]

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("ScalarModifierTarget objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls, owner: _Owner) -> "ScalarModifierTarget":
        instance = object.__new__(cls)
        instance._owner = owner
        instance._modifiers = []
        return instance

    def modulate(
        self,
        signal: "ScalarSignal",
        *,
        mode: Literal["replace", "add", "multiply"] = "add",
    ) -> Self:
        from .signals import ScalarSignal
        if not isinstance(signal, ScalarSignal):
            raise TypeError("signal must be ScalarSignal")
        if mode not in {"replace", "add", "multiply"}:
            raise ValueError("mode must be replace, add, or multiply")
        self._modifiers.append({"operation": mode, "signal": signal})
        return self

    react_to = modulate

    def __repr__(self) -> str:
        return f"ScalarModifierTarget(modifiers={len(self._modifiers)})"


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

    __slots__ = ("_owner", "_position", "_anchor", "_scale", "_rotation_degrees", "_position_x", "_position_y", "_scale_x", "_scale_y")

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("Transform objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls, owner: _Owner) -> "Transform":
        instance = object.__new__(cls)
        instance._owner = owner
        instance._position = PointTrack._create(owner, Point(0.5, 0.5))
        instance._anchor = _AnchorTrack._create(owner, Point(0.5, 0.5))
        instance._scale = _ScaleTrack._create(owner, Point(1.0, 1.0))
        instance._rotation_degrees = ModulatableScalarTrack._create(owner, 0.0)
        instance._position_x = ScalarModifierTarget._create(owner)
        instance._position_y = ScalarModifierTarget._create(owner)
        instance._scale_x = ScalarModifierTarget._create(owner)
        instance._scale_y = ScalarModifierTarget._create(owner)
        instance._scale._uniform_target = (instance._scale_x, instance._scale_y)
        return instance

    @property
    def position(self) -> PointTrack:
        return self._position

    @property
    def anchor(self) -> PointTrack:
        return self._anchor

    @property
    def scale(self) -> _ScaleTrack:
        return self._scale

    @property
    def rotation_degrees(self) -> ModulatableScalarTrack:
        """Rotation authoring and modulation values are measured in degrees."""
        return self._rotation_degrees

    @property
    def position_x(self) -> ScalarModifierTarget: return self._position_x
    @property
    def position_y(self) -> ScalarModifierTarget: return self._position_y
    @property
    def scale_x(self) -> ScalarModifierTarget: return self._scale_x
    @property
    def scale_y(self) -> ScalarModifierTarget: return self._scale_y

    def modulate_scale(self, signal: "ScalarSignal", *, mode: Literal["replace", "add", "multiply"] = "multiply") -> None:
        self.scale_x.modulate(signal, mode=mode)
        self.scale_y.modulate(signal, mode=mode)

    def __repr__(self) -> str:
        return ("Transform(position=" f"{self.position!r}, anchor={self.anchor!r}, "
                f"scale={self.scale!r}, rotation_degrees={self.rotation_degrees!r})")

    def _initialize(self, owner: _Owner) -> None:
        self._owner = owner
        self._position = PointTrack._create(owner, Point(0.5, 0.5))
        self._anchor = _AnchorTrack._create(owner, Point(0.5, 0.5))
        self._scale = _ScaleTrack._create(owner, Point(1.0, 1.0))
        self._rotation_degrees = ModulatableScalarTrack._create(owner, 0.0)
        self._position_x = ScalarModifierTarget._create(owner)
        self._position_y = ScalarModifierTarget._create(owner)
        self._scale_x = ScalarModifierTarget._create(owner)
        self._scale_y = ScalarModifierTarget._create(owner)
        self._scale._uniform_target = (self._scale_x, self._scale_y)

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {
            "position": self.position.to_canonical(),
            "anchor": self.anchor.to_canonical(),
            "scale": self.scale.to_canonical(),
            "rotation_degrees": self.rotation_degrees.to_canonical(),
        }
        components = {
            "position_x": self.position_x._modifiers,
            "position_y": self.position_y._modifiers,
            "scale_x": self.scale_x._modifiers,
            "scale_y": self.scale_y._modifiers,
        }
        if any(components.values()):
            data["component_modifiers"] = {
                name: [{"operation": item["operation"], "signal": item["signal"].to_canonical()} for item in modifiers]
                for name, modifiers in components.items() if modifiers
            }
        return data


class _AnchorTrack(PointTrack):
    def _validate(self, value: Point) -> Point:
        point = super()._validate(value)
        if not 0.0 <= point.x <= 1.0 or not 0.0 <= point.y <= 1.0:
            raise ValueError("anchor must be within unit space")
        return point


class _ScaleTrack(PointTrack):
    __slots__ = ("_uniform_target",)
    _uniform_target: tuple[ScalarModifierTarget, ScalarModifierTarget]

    def _validate(self, value: Point) -> Point:
        point = super()._validate(value)
        if point.x <= 0.0 or point.y <= 0.0:
            raise ValueError("scale must be positive")
        return point

    def react_to(self, signal: "ScalarSignal", *, mode: Literal["replace", "add", "multiply"] = "multiply") -> None:
        self._uniform_target[0].modulate(signal, mode=mode)
        self._uniform_target[1].modulate(signal, mode=mode)
