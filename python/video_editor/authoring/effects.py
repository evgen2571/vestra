"""Typed, ordered effect handles for the schema-version 1 project model."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Callable

from ._internal import _IdAllocator, _Owner, _number
from .tracks import ScalarTrack
from .values import Color, Point, color_to_canonical


@dataclass(frozen=True, slots=True)
class ActiveInterval:
    """A finite half-open clip-local interval for transient effects."""

    start: float = 0.0
    duration: float | None = None

    def __post_init__(self) -> None:
        start = _number(self.start, "start")
        if start < 0:
            raise ValueError("start must be non-negative")
        object.__setattr__(self, "start", start)
        if self.duration is not None:
            duration = _number(self.duration, "duration")
            if duration <= 0:
                raise ValueError("duration must be positive")
            object.__setattr__(self, "duration", duration)

    def to_canonical(self) -> dict[str, float]:
        data: dict[str, float] = {"start": self.start}
        if self.duration is not None:
            data["duration"] = self.duration
        return data


class ZoomBlurDirection(Enum):
    INWARD = "inward"
    OUTWARD = "outward"
    CENTERED = "centered"


def _integer(value: int, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{name} must be an integer")
    return value


class Effect:
    __slots__ = ("_owner", "_scope", "_id", "_kind")

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError(f"{type(self).__name__} objects must be created by an effect collection")

    def _initialize(self, owner: _Owner, scope: object, identifier: str, kind: str) -> None:
        self._owner = owner
        self._scope = scope
        self._id = identifier
        self._kind = kind

    @property
    def id(self) -> str:
        return self._id

    @property
    def kind(self) -> str:
        return self._kind

    def __eq__(self, other: object) -> bool:
        return (isinstance(other, Effect) and self._owner is other._owner and self._scope is other._scope
                and self.id == other.id and self.kind == other.kind)

    def __repr__(self) -> str:
        return f"{type(self).__name__}(id={self.id!r})"

    def _canonical(self) -> dict[str, object]:
        return {"id": self.id, "type": self.kind}

    def to_canonical(self) -> dict[str, object]:
        raise NotImplementedError


class _AmountEffect(Effect):
    __slots__ = ("_amount",)
    _amount: ScalarTrack

    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, kind: str, amount: int | float) -> _AmountEffect:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, kind)
        instance._amount = ScalarTrack._create(owner, amount)
        return instance

    @property
    def amount(self) -> ScalarTrack:
        return self._amount

    def to_canonical(self) -> dict[str, object]:
        data = self._canonical()
        data["amount"] = self.amount.to_canonical()
        return data


class BrightnessEffect(_AmountEffect): pass
class ContrastEffect(_AmountEffect): pass
class SaturationEffect(_AmountEffect): pass


class TintEffect(_AmountEffect):
    __slots__ = ("_colour",)
    _colour: str
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, colour: Color | str, amount: int | float) -> TintEffect:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "tint")
        instance._amount = ScalarTrack._create(owner, amount)
        instance._colour = color_to_canonical(colour)
        return instance
    @property
    def colour(self) -> str: return self._colour
    @colour.setter
    def colour(self, value: Color | str) -> None: self._colour = color_to_canonical(value)
    def to_canonical(self) -> dict[str, object]:
        data = super().to_canonical(); data["colour"] = self.colour
        return {"id": data.pop("id"), "type": data.pop("type"), "colour": data.pop("colour"), "amount": data.pop("amount")}


class GaussianBlurEffect(Effect):
    __slots__ = ("_radius",)
    _radius: ScalarTrack
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, radius: int | float) -> GaussianBlurEffect:
        instance = object.__new__(cls); instance._initialize(owner, scope, identifier, "gaussian_blur"); instance._radius = ScalarTrack._create(owner, radius); return instance
    @property
    def radius(self) -> ScalarTrack: return self._radius
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "radius": self.radius.to_canonical()}


class DirectionalBlurEffect(Effect):
    __slots__ = ("_radius", "_angle_degrees")
    _radius: ScalarTrack
    _angle_degrees: ScalarTrack
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, radius: int | float, angle_degrees: int | float) -> DirectionalBlurEffect:
        instance = object.__new__(cls); instance._initialize(owner, scope, identifier, "directional_blur"); instance._radius = ScalarTrack._create(owner, radius); instance._angle_degrees = ScalarTrack._create(owner, angle_degrees); return instance
    @property
    def radius(self) -> ScalarTrack: return self._radius
    @property
    def angle_degrees(self) -> ScalarTrack: return self._angle_degrees
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "radius": self.radius.to_canonical(), "angle_degrees": self.angle_degrees.to_canonical()}


class ZoomBlurEffect(Effect):
    __slots__ = ("_radius", "_samples", "_anchor", "_direction")
    _radius: ScalarTrack
    _samples: int
    _anchor: Point
    _direction: ZoomBlurDirection
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, radius: int | float, samples: int, anchor: Point, direction: ZoomBlurDirection) -> ZoomBlurEffect:
        if not isinstance(anchor, Point): raise TypeError("anchor must be Point")
        if not 0 <= anchor.x <= 1 or not 0 <= anchor.y <= 1: raise ValueError("anchor must be within unit space")
        if not isinstance(direction, ZoomBlurDirection): raise TypeError("direction must be ZoomBlurDirection")
        instance = object.__new__(cls); instance._initialize(owner, scope, identifier, "zoom_blur"); instance._radius = ScalarTrack._create(owner, radius); instance._samples = _integer(samples, "samples"); instance._anchor = Point(anchor.x, anchor.y); instance._direction = direction; return instance
    @property
    def radius(self) -> ScalarTrack: return self._radius
    @property
    def samples(self) -> int: return self._samples
    @samples.setter
    def samples(self, value: int) -> None: self._samples = _integer(value, "samples")
    @property
    def anchor(self) -> Point: return self._anchor
    @anchor.setter
    def anchor(self, value: Point) -> None:
        if not isinstance(value, Point): raise TypeError("anchor must be Point")
        if not 0 <= value.x <= 1 or not 0 <= value.y <= 1: raise ValueError("anchor must be within unit space")
        self._anchor = Point(value.x, value.y)
    @property
    def direction(self) -> ZoomBlurDirection: return self._direction
    @direction.setter
    def direction(self, value: ZoomBlurDirection) -> None:
        if not isinstance(value, ZoomBlurDirection): raise TypeError("direction must be ZoomBlurDirection")
        self._direction = value
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "radius": self.radius.to_canonical(), "samples": self.samples, "anchor": self.anchor.to_canonical(), "direction": self.direction.value}


class GlowEffect(Effect):
    __slots__ = ("_threshold", "_radius", "_intensity", "_colour")
    _threshold: ScalarTrack
    _radius: ScalarTrack
    _intensity: ScalarTrack
    _colour: str
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, threshold: int | float, radius: int | float, intensity: int | float, colour: Color | str) -> GlowEffect:
        instance = object.__new__(cls); instance._initialize(owner, scope, identifier, "glow"); instance._threshold = ScalarTrack._create(owner, threshold); instance._radius = ScalarTrack._create(owner, radius); instance._intensity = ScalarTrack._create(owner, intensity); instance._colour = color_to_canonical(colour); return instance
    @property
    def threshold(self) -> ScalarTrack: return self._threshold
    @property
    def radius(self) -> ScalarTrack: return self._radius
    @property
    def intensity(self) -> ScalarTrack: return self._intensity
    @property
    def colour(self) -> str: return self._colour
    @colour.setter
    def colour(self, value: Color | str) -> None: self._colour = color_to_canonical(value)
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "threshold": self.threshold.to_canonical(), "radius": self.radius.to_canonical(), "intensity": self.intensity.to_canonical(), "colour": self.colour}


class ChromaticAberrationEffect(DirectionalBlurEffect):
    __slots__ = ()
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, amount: int | float, angle_degrees: int | float) -> ChromaticAberrationEffect:
        instance = object.__new__(cls); instance._initialize(owner, scope, identifier, "chromatic_aberration"); instance._radius = ScalarTrack._create(owner, amount); instance._angle_degrees = ScalarTrack._create(owner, angle_degrees); return instance
    @property
    def amount(self) -> ScalarTrack: return self._radius
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "amount": self.amount.to_canonical(), "angle_degrees": self.angle_degrees.to_canonical()}


class VignetteEffect(GlowEffect):
    __slots__ = ("_amount", "_softness")
    _amount: ScalarTrack
    _softness: ScalarTrack
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, amount: int | float, radius: int | float, softness: int | float, colour: Color | str) -> VignetteEffect:
        instance = object.__new__(cls); instance._initialize(owner, scope, identifier, "vignette"); instance._amount = ScalarTrack._create(owner, amount); instance._radius = ScalarTrack._create(owner, radius); instance._softness = ScalarTrack._create(owner, softness); instance._colour = color_to_canonical(colour); return instance
    @property
    def amount(self) -> ScalarTrack: return self._amount
    @property
    def softness(self) -> ScalarTrack: return self._softness
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "amount": self.amount.to_canonical(), "radius": self.radius.to_canonical(), "softness": self.softness.to_canonical(), "colour": self.colour}


class SharpenEffect(DirectionalBlurEffect):
    __slots__ = ()
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, radius: int | float, angle_degrees: int | float) -> SharpenEffect:
        instance = object.__new__(cls); instance._initialize(owner, scope, identifier, "sharpen"); instance._radius = ScalarTrack._create(owner, radius); instance._angle_degrees = ScalarTrack._create(owner, angle_degrees); return instance
    @property
    def amount(self) -> ScalarTrack: return self._radius
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "amount": self.amount.to_canonical(), "radius": self._angle_degrees.to_canonical()}


class ColorAdjustEffect(Effect):
    __slots__ = ("_exposure", "_gamma", "_black_point", "_white_point")
    _exposure: ScalarTrack
    _gamma: ScalarTrack
    _black_point: ScalarTrack
    _white_point: ScalarTrack
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, exposure: int | float, gamma: int | float, black_point: int | float, white_point: int | float) -> ColorAdjustEffect:
        instance = object.__new__(cls); instance._initialize(owner, scope, identifier, "color_adjust"); instance._exposure = ScalarTrack._create(owner, exposure); instance._gamma = ScalarTrack._create(owner, gamma); instance._black_point = ScalarTrack._create(owner, black_point); instance._white_point = ScalarTrack._create(owner, white_point); return instance
    @property
    def exposure(self) -> ScalarTrack: return self._exposure
    @property
    def gamma(self) -> ScalarTrack: return self._gamma
    @property
    def black_point(self) -> ScalarTrack: return self._black_point
    @property
    def white_point(self) -> ScalarTrack: return self._white_point
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "exposure": self.exposure.to_canonical(), "gamma": self.gamma.to_canonical(), "black_point": self.black_point.to_canonical(), "white_point": self.white_point.to_canonical()}


class CameraShakeEffect(Effect):
    __slots__ = ("_active_interval", "_position_amount", "_rotation_degrees", "_scale_amount", "_frequency", "_seed", "_attack", "_decay")
    _active_interval: ActiveInterval
    _position_amount: ScalarTrack
    _rotation_degrees: ScalarTrack
    _scale_amount: ScalarTrack
    _frequency: ScalarTrack
    _seed: int
    _attack: float
    _decay: float
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, active_interval: ActiveInterval, position_amount: int | float, rotation_degrees: int | float, scale_amount: int | float, frequency: int | float, seed: int, attack: int | float, decay: int | float) -> CameraShakeEffect:
        if not isinstance(active_interval, ActiveInterval): raise TypeError("active_interval must be ActiveInterval")
        instance = object.__new__(cls); instance._initialize(owner, scope, identifier, "camera_shake"); instance._active_interval = active_interval; instance._position_amount = ScalarTrack._create(owner, position_amount); instance._rotation_degrees = ScalarTrack._create(owner, rotation_degrees); instance._scale_amount = ScalarTrack._create(owner, scale_amount); instance._frequency = ScalarTrack._create(owner, frequency); instance._seed = _integer(seed, "seed"); instance._attack = _number(attack, "attack"); instance._decay = _number(decay, "decay"); return instance
    @property
    def active_interval(self) -> ActiveInterval: return self._active_interval
    @active_interval.setter
    def active_interval(self, value: ActiveInterval) -> None:
        if not isinstance(value, ActiveInterval): raise TypeError("active_interval must be ActiveInterval")
        self._active_interval = value
    @property
    def position_amount(self) -> ScalarTrack: return self._position_amount
    @property
    def rotation_degrees(self) -> ScalarTrack: return self._rotation_degrees
    @property
    def scale_amount(self) -> ScalarTrack: return self._scale_amount
    @property
    def frequency(self) -> ScalarTrack: return self._frequency
    @property
    def seed(self) -> int: return self._seed
    @seed.setter
    def seed(self, value: int) -> None: self._seed = _integer(value, "seed")
    @property
    def attack(self) -> float: return self._attack
    @attack.setter
    def attack(self, value: int | float) -> None: self._attack = _number(value, "attack")
    @property
    def decay(self) -> float: return self._decay
    @decay.setter
    def decay(self, value: int | float) -> None: self._decay = _number(value, "decay")
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), **self.active_interval.to_canonical(), "position_amount": self.position_amount.to_canonical(), "rotation_degrees": self.rotation_degrees.to_canonical(), "scale_amount": self.scale_amount.to_canonical(), "frequency": self.frequency.to_canonical(), "seed": self.seed, "attack": self.attack, "decay": self.decay}


class MotionBlurEffect(Effect):
    __slots__ = ("_intensity", "_shutter_angle", "_max_radius", "_samples")
    _intensity: ScalarTrack
    _shutter_angle: ScalarTrack
    _max_radius: ScalarTrack
    _samples: int
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, intensity: int | float, shutter_angle: int | float, max_radius: int | float, samples: int) -> MotionBlurEffect:
        instance = object.__new__(cls); instance._initialize(owner, scope, identifier, "motion_blur"); instance._intensity = ScalarTrack._create(owner, intensity); instance._shutter_angle = ScalarTrack._create(owner, shutter_angle); instance._max_radius = ScalarTrack._create(owner, max_radius); instance._samples = _integer(samples, "samples"); return instance
    @property
    def intensity(self) -> ScalarTrack: return self._intensity
    @property
    def shutter_angle(self) -> ScalarTrack: return self._shutter_angle
    @property
    def max_radius(self) -> ScalarTrack: return self._max_radius
    @property
    def samples(self) -> int: return self._samples
    @samples.setter
    def samples(self, value: int) -> None: self._samples = _integer(value, "samples")
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "intensity": self.intensity.to_canonical(), "shutter_angle": self.shutter_angle.to_canonical(), "max_radius": self.max_radius.to_canonical(), "samples": self.samples}


class _EffectCollection:
    __slots__ = ("_owner", "_ids", "_scope", "_items")
    _items: list[Effect]
    def __init__(self, owner: _Owner, ids: _IdAllocator, scope: object) -> None: self._owner, self._ids, self._scope, self._items = owner, ids, scope, []
    @property
    def items(self) -> tuple[Effect, ...]: return tuple(self._items)
    def _identifier(self, identifier: str | None) -> str:
        if identifier is not None: self._ids.validate("effect", identifier, scope=self._scope)
        return self._ids.allocate("effect", scope=self._scope) if identifier is None else self._ids.reserve("effect", identifier, scope=self._scope)
    def _append(self, factory: Callable[..., Effect], identifier: str | None, *args: object) -> Effect:
        # Factories validate all arguments before this allocation, preserving transactionality.
        staged = factory(self._owner, self._scope, "", *args)
        staged._id = self._identifier(identifier)
        self._items.append(staged)
        return staged
    def add_brightness(self, *, amount: int | float, id: str | None = None) -> BrightnessEffect: return self._append(BrightnessEffect._create, id, "brightness", amount)  # type: ignore[return-value]
    def add_contrast(self, *, amount: int | float, id: str | None = None) -> ContrastEffect: return self._append(ContrastEffect._create, id, "contrast", amount)  # type: ignore[return-value]
    def add_saturation(self, *, amount: int | float, id: str | None = None) -> SaturationEffect: return self._append(SaturationEffect._create, id, "saturation", amount)  # type: ignore[return-value]
    def add_tint(self, *, colour: Color | str, amount: int | float, id: str | None = None) -> TintEffect: return self._append(TintEffect._create, id, colour, amount)  # type: ignore[return-value]
    def add_gaussian_blur(self, *, radius: int | float, id: str | None = None) -> GaussianBlurEffect: return self._append(GaussianBlurEffect._create, id, radius)  # type: ignore[return-value]
    def add_directional_blur(self, *, radius: int | float, angle_degrees: int | float, id: str | None = None) -> DirectionalBlurEffect: return self._append(DirectionalBlurEffect._create, id, radius, angle_degrees)  # type: ignore[return-value]
    def add_zoom_blur(self, *, radius: int | float, samples: int, anchor: Point, direction: ZoomBlurDirection = ZoomBlurDirection.CENTERED, id: str | None = None) -> ZoomBlurEffect: return self._append(ZoomBlurEffect._create, id, radius, samples, anchor, direction)  # type: ignore[return-value]
    def add_glow(self, *, threshold: int | float, radius: int | float, intensity: int | float, colour: Color | str, id: str | None = None) -> GlowEffect: return self._append(GlowEffect._create, id, threshold, radius, intensity, colour)  # type: ignore[return-value]
    def add_chromatic_aberration(self, *, amount: int | float, angle_degrees: int | float, id: str | None = None) -> ChromaticAberrationEffect: return self._append(ChromaticAberrationEffect._create, id, amount, angle_degrees)  # type: ignore[return-value]
    def add_vignette(self, *, amount: int | float, radius: int | float, softness: int | float, colour: Color | str, id: str | None = None) -> VignetteEffect: return self._append(VignetteEffect._create, id, amount, radius, softness, colour)  # type: ignore[return-value]
    def add_sharpen(self, *, amount: int | float, radius: int | float, id: str | None = None) -> SharpenEffect: return self._append(SharpenEffect._create, id, amount, radius)  # type: ignore[return-value]
    def add_color_adjust(self, *, exposure: int | float, gamma: int | float, black_point: int | float, white_point: int | float, id: str | None = None) -> ColorAdjustEffect: return self._append(ColorAdjustEffect._create, id, exposure, gamma, black_point, white_point)  # type: ignore[return-value]


class ClipEffectCollection(_EffectCollection):
    def add_camera_shake(self, *, active_interval: ActiveInterval = ActiveInterval(), position_amount: int | float, rotation_degrees: int | float, scale_amount: int | float, frequency: int | float, seed: int, attack: int | float, decay: int | float, id: str | None = None) -> CameraShakeEffect: return self._append(CameraShakeEffect._create, id, active_interval, position_amount, rotation_degrees, scale_amount, frequency, seed, attack, decay)  # type: ignore[return-value]
    def add_motion_blur(self, *, intensity: int | float, shutter_angle: int | float, max_radius: int | float, samples: int, id: str | None = None) -> MotionBlurEffect: return self._append(MotionBlurEffect._create, id, intensity, shutter_angle, max_radius, samples)  # type: ignore[return-value]


class PostEffectCollection(_EffectCollection):
    """Global effects. Transform-aware camera shake and motion blur are clip-only."""
