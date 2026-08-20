"""Particle source descriptors and configuration values."""

from __future__ import annotations

from copy import deepcopy
from dataclasses import replace
from typing import TYPE_CHECKING

from ..authoring.particles import (
    CircleEmitter,
    ColourLifetimeStop,
    ParticleAudioReactive as AuthoringParticleAudioReactive,
    ParticleBlendMode,
    ParticleBurst,
    ParticleLifetimeStyle,
    ParticlePrimitive,
    ParticleSystem as AuthoringParticleSystem,
    PointEmitter,
    RectangleEmitter,
    ScalarLifetimeStop,
    ScalarRange,
)
from ..authoring.tracks import ModulatableScalarTrack
from ..authoring.values import Color as AuthoringColor, Point
from .base import ParticleAudioReactive, Source

if TYPE_CHECKING:
    from ..authoring.builder import ProjectBuilder

class ParticleSystem(Source):
    """A high-level wrapper around the typed authoring particle definition."""

    __slots__ = ("definition", "_audio_reactive")

    definition: AuthoringParticleSystem

    _FIELD_NAMES = frozenset(
        {
            "emitter",
            "rate",
            "bursts",
            "seed",
            "lifetime",
            "lifetime_range",
            "size",
            "size_range",
            "colour",
            "opacity",
            "initial_velocity",
            "speed",
            "speed_range",
            "direction",
            "spread",
            "acceleration",
            "rotation",
            "rotation_range",
            "angular_velocity",
            "angular_velocity_range",
            "primitive",
            "blend_mode",
            "lifetime_style",
            "audio_reactive",
        }
    )

    def __getattr__(self, name: str) -> object:
        if name in self._FIELD_NAMES:
            return getattr(self.definition, name)
        raise AttributeError(name)

    def __setattr__(self, name: str, value: object) -> None:
        if name == "audio_reactive" and hasattr(self, "definition"):
            if value is not None and not isinstance(
                value, (ParticleAudioReactive, AuthoringParticleAudioReactive)
            ):
                raise TypeError("audio_reactive must be ParticleAudioReactive or None")
            self._audio_reactive = (
                None
                if value is None
                else value
                if isinstance(value, ParticleAudioReactive)
                else ParticleAudioReactive.from_authoring(value)
            )
            return
        if name in self._FIELD_NAMES and hasattr(self, "definition"):
            candidate = deepcopy(self.definition)
            setattr(candidate, name, value)
            candidate.__post_init__()
            self.definition = candidate
            return
        object.__setattr__(self, name, value)

    def __init__(
        self,
        definition: AuthoringParticleSystem | None = None,
        *,
        emitter: PointEmitter | RectangleEmitter | CircleEmitter = PointEmitter(),
        rate: float = 0.0,
        bursts: tuple[ParticleBurst, ...] = (),
        seed: int = 0,
        lifetime: float = 1.0,
        lifetime_range: ScalarRange | None = None,
        size: float = 1.0,
        size_range: ScalarRange | None = None,
        colour: str | AuthoringColor = "#ffffff",
        opacity: float = 1.0,
        initial_velocity: Point = Point(0.0, 0.0),
        speed: float = 0.0,
        speed_range: ScalarRange | None = None,
        direction: float = 0.0,
        spread: float = 0.0,
        acceleration: Point = Point(0.0, 0.0),
        rotation: float = 0.0,
        rotation_range: ScalarRange | None = None,
        angular_velocity: float = 0.0,
        angular_velocity_range: ScalarRange | None = None,
        primitive: ParticlePrimitive = ParticlePrimitive.DISC,
        blend_mode: ParticleBlendMode = ParticleBlendMode.NORMAL,
        lifetime_style: ParticleLifetimeStyle | None = None,
        audio_reactive: ParticleAudioReactive | None = None,
    ) -> None:
        if definition is not None:
            if not isinstance(definition, AuthoringParticleSystem):
                raise TypeError("definition must be an authoring ParticleSystem")
            self._audio_reactive = (
                None
                if definition.audio_reactive is None
                else ParticleAudioReactive.from_authoring(definition.audio_reactive)
            )
            # Owner-bound authoring tracks may contain immutable mapping proxies,
            # so detach them before copying the ordinary particle definition.
            # ``replace`` also leaves the caller's advanced value untouched.
            self.definition = deepcopy(replace(definition, audio_reactive=None))
            return
        if audio_reactive is not None and not isinstance(
            audio_reactive, (ParticleAudioReactive, AuthoringParticleAudioReactive)
        ):
            raise TypeError("audio_reactive must be ParticleAudioReactive or None")
        self._audio_reactive = (
            None
            if audio_reactive is None
            else audio_reactive
            if isinstance(audio_reactive, ParticleAudioReactive)
            else ParticleAudioReactive.from_authoring(audio_reactive)
        )
        self.definition = AuthoringParticleSystem(
            emitter=emitter,
            rate=rate,
            bursts=bursts,
            seed=seed,
            lifetime=lifetime,
            lifetime_range=lifetime_range,
            size=size,
            size_range=size_range,
            colour=colour,
            opacity=opacity,
            initial_velocity=initial_velocity,
            speed=speed,
            speed_range=speed_range,
            direction=direction,
            spread=spread,
            acceleration=acceleration,
            rotation=rotation,
            rotation_range=rotation_range,
            angular_velocity=angular_velocity,
            angular_velocity_range=angular_velocity_range,
            primitive=primitive,
            blend_mode=blend_mode,
            lifetime_style=lifetime_style,
            audio_reactive=None,
        )

    @property
    def audio_reactive(self) -> ParticleAudioReactive | None:
        return self._audio_reactive

    def lowering_definition(self, builder: "ProjectBuilder") -> AuthoringParticleSystem:
        """Copy this source into fresh builder-owned native tracks."""
        native = deepcopy(self.definition)
        if self._audio_reactive is None:
            return native
        reactive_values: dict[str, ModulatableScalarTrack | None] = {}
        for name in ("size", "opacity", "intensity"):
            property_value = getattr(self._audio_reactive, name)
            if property_value is None:
                reactive_values[name] = None
                continue
            target = builder.scalar_property(property_value.value)
            for frame in property_value.keyframes:
                target.keyframe(
                    time=frame.time,
                    value=frame.value,
                    interpolation=frame.interpolation,
                )
            for binding in property_value.bindings:
                target.modulate(binding.signal, mode=binding.operation)
            reactive_values[name] = target
        native.audio_reactive = AuthoringParticleAudioReactive(
            size=reactive_values["size"],
            opacity=reactive_values["opacity"],
            intensity=reactive_values["intensity"],
        )
        return native

    def to_canonical(self) -> dict[str, object]:
        return {"type": "particle_system", **self.definition.to_canonical()}

    def snapshot(self) -> "ParticleSystem":
        copied = object.__new__(type(self))
        copied.definition = deepcopy(self.definition)
        copied._audio_reactive = (
            None if self._audio_reactive is None else self._audio_reactive.copy()
        )
        return copied


__all__ = [
    "ParticleSystem", "PointEmitter", "RectangleEmitter", "CircleEmitter",
    "ParticleBurst", "ScalarRange", "ParticlePrimitive", "ParticleBlendMode",
    "ParticleLifetimeStyle", "ScalarLifetimeStop", "ColourLifetimeStop",
    "ParticleAudioReactive",
]
