"""Typed authoring objects for the canonical ParticleSystem source."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum

from ._internal import _number
from .tracks import ModulatableScalarTrack
from .values import Color, Point, color_to_canonical


def _non_negative(value: int | float, name: str) -> float:
    number = _number(value, name)
    if number < 0:
        raise ValueError(f"{name} must be non-negative")
    return number


def _range(value: "ScalarRange | None", name: str) -> "ScalarRange | None":
    if value is not None and not isinstance(value, ScalarRange):
        raise TypeError(f"{name} must be ScalarRange or None")
    return value


class ParticlePrimitive(Enum):
    DISC = "disc"
    SQUARE = "square"

    def to_canonical(self) -> str:
        return self.value


class ParticleBlendMode(Enum):
    NORMAL = "normal"
    ADDITIVE = "additive"

    def to_canonical(self) -> str:
        return self.value


@dataclass(frozen=True, slots=True)
class ScalarRange:
    minimum: float
    maximum: float

    def __post_init__(self) -> None:
        minimum, maximum = (
            _number(self.minimum, "minimum"),
            _number(self.maximum, "maximum"),
        )
        if minimum > maximum:
            raise ValueError("minimum must not exceed maximum")
        object.__setattr__(self, "minimum", minimum)
        object.__setattr__(self, "maximum", maximum)

    def to_canonical(self) -> dict[str, float]:
        return {"min": self.minimum, "max": self.maximum}


@dataclass(frozen=True, slots=True)
class PointEmitter:
    position: Point = field(default_factory=lambda: Point(0.5, 0.5))

    def __post_init__(self) -> None:
        if not isinstance(self.position, Point):
            raise TypeError("position must be Point")

    def to_canonical(self) -> dict[str, object]:
        return {"type": "point", "position": self.position.to_canonical()}


@dataclass(frozen=True, slots=True)
class RectangleEmitter:
    center: Point
    size: Point

    def __post_init__(self) -> None:
        for name in ("center", "size"):
            if not isinstance(getattr(self, name), Point):
                raise TypeError(f"{name} must be Point")

    def to_canonical(self) -> dict[str, object]:
        return {
            "type": "rectangle",
            "center": self.center.to_canonical(),
            "size": self.size.to_canonical(),
        }


@dataclass(frozen=True, slots=True)
class CircleEmitter:
    center: Point
    inner_radius: float = 0.0
    outer_radius: float = 0.5

    def __post_init__(self) -> None:
        if not isinstance(self.center, Point):
            raise TypeError("center must be Point")
        inner, outer = (
            _non_negative(self.inner_radius, "inner_radius"),
            _non_negative(self.outer_radius, "outer_radius"),
        )
        if inner > outer:
            raise ValueError("inner_radius must not exceed outer_radius")
        object.__setattr__(self, "inner_radius", inner)
        object.__setattr__(self, "outer_radius", outer)

    def to_canonical(self) -> dict[str, object]:
        return {
            "type": "circle",
            "center": self.center.to_canonical(),
            "inner_radius": self.inner_radius,
            "outer_radius": self.outer_radius,
        }


@dataclass(frozen=True, slots=True)
class ParticleBurst:
    time: float
    count: int

    def __post_init__(self) -> None:
        object.__setattr__(self, "time", _non_negative(self.time, "time"))
        if isinstance(self.count, bool) or not isinstance(self.count, int):
            raise TypeError("count must be an integer")
        if self.count < 0:
            raise ValueError("count must be non-negative")

    def to_canonical(self) -> dict[str, float | int]:
        return {"time": self.time, "count": self.count}


@dataclass(frozen=True, slots=True)
class ScalarLifetimeStop:
    t: float
    value: float

    def __post_init__(self) -> None:
        t = _number(self.t, "t")
        if not 0.0 <= t <= 1.0:
            raise ValueError("t must be between 0 and 1")
        object.__setattr__(self, "t", t)
        object.__setattr__(self, "value", _number(self.value, "value"))

    def to_canonical(self) -> dict[str, float]:
        return {"t": self.t, "value": self.value}


@dataclass(frozen=True, slots=True)
class ColourLifetimeStop:
    t: float
    colour: Color | str

    def __post_init__(self) -> None:
        t = _number(self.t, "t")
        if not 0.0 <= t <= 1.0:
            raise ValueError("t must be between 0 and 1")
        object.__setattr__(self, "t", t)
        object.__setattr__(self, "colour", color_to_canonical(self.colour))

    def to_canonical(self) -> dict[str, float | str]:
        return {"t": self.t, "colour": color_to_canonical(self.colour)}


@dataclass(frozen=True, slots=True)
class ParticleLifetimeStyle:
    size: tuple[ScalarLifetimeStop, ...] = ()
    opacity: tuple[ScalarLifetimeStop, ...] = ()
    colour: tuple[ColourLifetimeStop, ...] = ()

    def __post_init__(self) -> None:
        for name, item_type in (
            ("size", ScalarLifetimeStop),
            ("opacity", ScalarLifetimeStop),
            ("colour", ColourLifetimeStop),
        ):
            values = getattr(self, name)
            if not isinstance(values, tuple):
                values = tuple(values)
            if any(not isinstance(item, item_type) for item in values):
                raise TypeError(f"{name} must contain typed lifetime stops")
            if any(
                previous.t >= current.t for previous, current in zip(values, values[1:])
            ):
                raise ValueError(f"{name} stop positions must be strictly increasing")
            object.__setattr__(self, name, values)

    def to_canonical(self) -> dict[str, object]:
        return {
            name: [item.to_canonical() for item in getattr(self, name)]
            for name in ("size", "opacity", "colour")
            if getattr(self, name)
        }


@dataclass(frozen=True, slots=True)
class ParticleAudioReactive:
    """Appearance-only modulation; emission and motion are intentionally absent."""

    size: ModulatableScalarTrack | None = None
    opacity: ModulatableScalarTrack | None = None
    intensity: ModulatableScalarTrack | None = None

    def __post_init__(self) -> None:
        for name in ("size", "opacity", "intensity"):
            value = getattr(self, name)
            if value is not None and not isinstance(value, ModulatableScalarTrack):
                raise TypeError(f"{name} must be a ScalarTrack or None")

    def to_canonical(self) -> dict[str, object]:
        return {
            name: getattr(self, name).to_canonical()
            for name in ("size", "opacity", "intensity")
            if getattr(self, name) is not None
        }


@dataclass(slots=True)
class ParticleSystem:
    emitter: PointEmitter | RectangleEmitter | CircleEmitter = field(
        default_factory=PointEmitter
    )
    rate: float = 0.0
    bursts: tuple[ParticleBurst, ...] = ()
    seed: int = 0
    lifetime: float = 1.0
    lifetime_range: ScalarRange | None = None
    size: float = 1.0
    size_range: ScalarRange | None = None
    colour: Color | str = "#ffffff"
    opacity: float = 1.0
    initial_velocity: Point = field(default_factory=lambda: Point(0.0, 0.0))
    speed: float = 0.0
    speed_range: ScalarRange | None = None
    direction: float = 0.0
    spread: float = 0.0
    acceleration: Point = field(default_factory=lambda: Point(0.0, 0.0))
    rotation: float = 0.0
    rotation_range: ScalarRange | None = None
    angular_velocity: float = 0.0
    angular_velocity_range: ScalarRange | None = None
    primitive: ParticlePrimitive = ParticlePrimitive.DISC
    blend_mode: ParticleBlendMode = ParticleBlendMode.NORMAL
    lifetime_style: ParticleLifetimeStyle | None = None
    audio_reactive: ParticleAudioReactive | None = None

    def __post_init__(self) -> None:
        if not isinstance(
            self.emitter, (PointEmitter, RectangleEmitter, CircleEmitter)
        ):
            raise TypeError("emitter must be a particle emitter")
        bursts = self.bursts if isinstance(self.bursts, tuple) else tuple(self.bursts)
        if any(not isinstance(burst, ParticleBurst) for burst in bursts):
            raise TypeError("bursts must contain ParticleBurst objects")
        object.__setattr__(self, "bursts", bursts)
        for name in ("initial_velocity", "acceleration"):
            if not isinstance(getattr(self, name), Point):
                raise TypeError(f"{name} must be Point")
        object.__setattr__(self, "rate", _non_negative(self.rate, "rate"))
        object.__setattr__(self, "lifetime", _number(self.lifetime, "lifetime"))
        if self.lifetime <= 0:
            raise ValueError("lifetime must be positive")
        for name in (
            "lifetime_range",
            "size_range",
            "speed_range",
            "rotation_range",
            "angular_velocity_range",
        ):
            object.__setattr__(self, name, _range(getattr(self, name), name))
        for name in ("direction", "rotation", "angular_velocity"):
            object.__setattr__(self, name, _number(getattr(self, name), name))
        for name in ("size", "speed", "spread", "opacity"):
            value = _number(getattr(self, name), name)
            if name == "opacity" and not 0.0 <= value <= 1.0:
                raise ValueError("opacity must be between 0 and 1")
            if name != "opacity" and value < 0:
                raise ValueError(f"{name} must be non-negative")
            object.__setattr__(self, name, value)
        if self.spread > 360:
            raise ValueError("spread must be at most 360 degrees")
        if (
            isinstance(self.seed, bool)
            or not isinstance(self.seed, int)
            or self.seed < 0
        ):
            raise TypeError("seed must be a non-negative integer")
        if not isinstance(self.primitive, ParticlePrimitive) or not isinstance(
            self.blend_mode, ParticleBlendMode
        ):
            raise TypeError("primitive and blend_mode must use particle enums")
        if self.lifetime_style is not None and not isinstance(
            self.lifetime_style, ParticleLifetimeStyle
        ):
            raise TypeError("lifetime_style must be ParticleLifetimeStyle or None")
        if self.audio_reactive is not None and not isinstance(
            self.audio_reactive, ParticleAudioReactive
        ):
            raise TypeError("audio_reactive must be ParticleAudioReactive or None")
        object.__setattr__(self, "colour", color_to_canonical(self.colour))

    def to_canonical(self) -> dict[str, object]:
        particle: dict[str, object] = {
            "lifetime": self.lifetime,
            "initial_velocity": self.initial_velocity.to_canonical(),
            "acceleration": self.acceleration.to_canonical(),
            "size": self.size,
            "speed": self.speed,
            "direction_degrees": self.direction,
            "direction_spread_degrees": self.spread,
            "opacity": self.opacity,
            "colour": color_to_canonical(self.colour),
            "rotation_degrees": self.rotation,
            "angular_velocity_degrees": self.angular_velocity,
            "primitive": self.primitive.to_canonical(),
            "blend_mode": self.blend_mode.to_canonical(),
        }
        for name in (
            "lifetime_range",
            "size_range",
            "speed_range",
            "rotation_range",
            "angular_velocity_range",
        ):
            value = getattr(self, name)
            if value is not None:
                particle[name] = value.to_canonical()
        if self.lifetime_style is not None:
            particle["lifetime_style"] = self.lifetime_style.to_canonical()
        if self.audio_reactive is not None:
            particle["audio_reactive"] = self.audio_reactive.to_canonical()
        return {
            "type": "particle_system",
            "seed": self.seed,
            "emitter": self.emitter.to_canonical(),
            "emission": {
                "rate": self.rate,
                "bursts": [burst.to_canonical() for burst in self.bursts],
            },
            "particle": particle,
        }
