"""Source values for the high-level editor API.

The classes in this module are intentionally small authoring handles.  They
hold source-specific values, while ``vestra.authoring`` remains the canonical
implementation used during lowering.
"""

from __future__ import annotations

import os
from copy import deepcopy
from dataclasses import replace
from typing import TYPE_CHECKING, Literal, TypeAlias, cast

from .authoring._internal import _Owner
from .authoring.clips import Spectrum2DClip
from .authoring.particles import (
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
from .authoring.tracks import ModulatableScalarTrack
from .authoring.spectrum2d import (
    Spectrum2DGradient,
    Spectrum2DLayout,
    Spectrum2DLinearLayout,
    Spectrum2DPreset,
    Spectrum2DRadialLayout,
    _UNSET,
    _Unset,
    _resolve_spectrum2d_source,
)
from .authoring.values import (
    Color as AuthoringColor,
    Crop,
    Point,
    Sizing,
    color_to_canonical,
)
from .properties import CropProperty
from .properties import BindableScalarProperty, ScalarProperty, SignalOperation
from .authoring.signals import ScalarSignal

if TYPE_CHECKING:
    from .authoring.builder import ProjectBuilder


def _copy_bindable_property(value: ScalarProperty) -> BindableScalarProperty:
    copied = BindableScalarProperty(value.value)
    for frame in value.keyframes:
        copied.keyframe(frame.time, frame.value, interpolation=frame.interpolation)
    if isinstance(value, BindableScalarProperty):
        for binding in value.bindings:
            copied.bind(binding.signal, operation=binding.operation)
    return copied


def _authoring_property(value: object) -> BindableScalarProperty:
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return BindableScalarProperty(value)
    if isinstance(value, ScalarProperty):
        return _copy_bindable_property(value)
    if isinstance(value, ModulatableScalarTrack):
        copied = BindableScalarProperty(value.base_value)
        for frame in value.keyframes:
            copied.keyframe(frame.time, frame.value, interpolation=frame.interpolation)
        for modifier in value._modifiers:
            copied.bind(
                cast(ScalarSignal, modifier["signal"]),
                operation=cast(SignalOperation, modifier["operation"]),
            )
        return copied
    raise TypeError(
        "particle audio-reactive values must be scalar properties, numbers, or None"
    )


ReactiveProperty: TypeAlias = ScalarProperty | ModulatableScalarTrack | int | float


class ParticleAudioReactive:
    """Builder-independent particle appearance bindings."""

    __slots__ = ("_size", "_opacity", "_intensity")

    def __init__(
        self,
        *,
        size: ReactiveProperty | None = None,
        opacity: ReactiveProperty | None = None,
        intensity: ReactiveProperty | None = None,
    ) -> None:
        self._size = None if size is None else _authoring_property(size)
        self._opacity = None if opacity is None else _authoring_property(opacity)
        self._intensity = None if intensity is None else _authoring_property(intensity)

    @property
    def size(self) -> BindableScalarProperty | None:
        return self._size

    @property
    def opacity(self) -> BindableScalarProperty | None:
        return self._opacity

    @property
    def intensity(self) -> BindableScalarProperty | None:
        return self._intensity

    @classmethod
    def from_authoring(
        cls, value: AuthoringParticleAudioReactive
    ) -> "ParticleAudioReactive":
        return cls(size=value.size, opacity=value.opacity, intensity=value.intensity)

    def to_canonical(self) -> dict[str, object]:
        result: dict[str, object] = {}
        for name in ("size", "opacity", "intensity"):
            value = getattr(self, name)
            if value is not None:
                canonical = value.to_canonical()
                if "bindings" in canonical:
                    canonical["modifiers"] = canonical.pop("bindings")
                result[name] = canonical
        return result

    def copy(self) -> "ParticleAudioReactive":
        return ParticleAudioReactive(
            size=None if self.size is None else _copy_bindable_property(self.size),
            opacity=None
            if self.opacity is None
            else _copy_bindable_property(self.opacity),
            intensity=None
            if self.intensity is None
            else _copy_bindable_property(self.intensity),
        )


class Source:
    """Extensible source value base class.

    Source values are copied when placed in a composition. Subclasses may
    override ``snapshot`` when a normal deep copy is not appropriate.
    """

    __slots__ = ()

    def snapshot(self) -> "Source":
        return deepcopy(self)


SizingValue: TypeAlias = Sizing | Literal["original", "fit", "cover"]


def _sizing(value: SizingValue | None) -> Sizing | None:
    if value is None:
        return None
    if isinstance(value, Sizing):
        return value
    if isinstance(value, str) and value in {"original", "fit", "cover"}:
        return Sizing(value)
    raise TypeError("sizing must be Sizing, 'original', 'fit', 'cover', or None")


class Image(Source):
    """An image file reference with image-only sizing and crop values."""

    __slots__ = ("_path", "_sizing", "_crop")

    _path: str
    _sizing: Sizing | None
    _crop: CropProperty

    def __init__(
        self,
        path: str | os.PathLike[str],
        *,
        sizing: SizingValue | None = None,
        crop: Crop | CropProperty | None = None,
    ) -> None:
        try:
            value = os.fspath(path)
        except TypeError as error:
            raise TypeError("image path must be str or PathLike[str]") from error
        if not isinstance(value, str):
            raise TypeError("image path must be str or PathLike[str]")
        if not value or value.isspace():
            raise ValueError("image path must not be empty")
        if crop is not None and not isinstance(crop, Crop | CropProperty):
            raise TypeError("crop must be Crop, CropProperty, or None")
        self._path = value
        self._sizing = _sizing(sizing)
        self._crop = (
            CropProperty()
            if crop is None
            else (crop if isinstance(crop, CropProperty) else CropProperty(crop))
        )

    @property
    def path(self) -> str:
        return self._path

    @path.setter
    def path(self, value: str | os.PathLike[str]) -> None:
        try:
            candidate = os.fspath(value)
        except TypeError as error:
            raise TypeError("image path must be str or PathLike[str]") from error
        if not isinstance(candidate, str):
            raise TypeError("image path must be str or PathLike[str]")
        if not candidate or candidate.isspace():
            raise ValueError("image path must not be empty")
        self._path = candidate

    @property
    def sizing(self) -> Sizing | None:
        return self._sizing

    @sizing.setter
    def sizing(self, value: SizingValue | None) -> None:
        self._sizing = _sizing(value)

    @property
    def crop(self) -> CropProperty:
        """Mutable crop property; assigning a :class:`Crop` remains supported."""
        return self._crop

    @crop.setter
    def crop(self, value: Crop | CropProperty | None) -> None:
        if value is None:
            self._crop = CropProperty()
        elif isinstance(value, CropProperty):
            self._crop = value
        elif isinstance(value, Crop):
            self._crop = CropProperty(value)
        else:
            raise TypeError("crop must be Crop, CropProperty, or None")


class Color(Source):
    """A solid ``#RRGGBB`` or ``#RRGGBBAA`` source value."""

    __slots__ = ("_value",)

    _value: str

    def __init__(self, value: str | AuthoringColor) -> None:
        canonical = value.to_canonical() if isinstance(value, AuthoringColor) else value
        validated = AuthoringColor(canonical)
        self._value = validated.value

    @property
    def value(self) -> str:
        return self._value

    @value.setter
    def value(self, value: str | AuthoringColor) -> None:
        canonical = value.to_canonical() if isinstance(value, AuthoringColor) else value
        self._value = AuthoringColor(canonical).value

    def to_canonical(self) -> str:
        return self.value


SolidColor = Color


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

    def snapshot(self) -> "ParticleSystem":
        copied = object.__new__(type(self))
        copied.definition = deepcopy(self.definition)
        copied._audio_reactive = (
            None if self._audio_reactive is None else self._audio_reactive.copy()
        )
        return copied


_SPECTRUM_FIELDS = (
    "band_count",
    "min_hz",
    "max_hz",
    "sensitivity",
    "attack_seconds",
    "release_seconds",
    "x",
    "y",
    "width",
    "height",
    "bar_gap_ratio",
    "colour",
    "min_bar_height_ratio",
    "layout",
    "gradient",
)


class Spectrum2D(Source):
    """An audio-reactive Spectrum2D source using the authoring presets."""

    __slots__ = ("_preset", "_overrides", "_values")

    _preset: Spectrum2DPreset | None

    def __init__(
        self,
        *,
        preset: Spectrum2DPreset | None = None,
        band_count: int | None = None,
        min_hz: int | float | None = None,
        max_hz: int | float | None = None,
        sensitivity: int | float | None = None,
        attack_seconds: int | float | None = None,
        release_seconds: int | float | None = None,
        x: int | float | None = None,
        y: int | float | None = None,
        width: int | float | None = None,
        height: int | float | None = None,
        bar_gap_ratio: int | float | None = None,
        colour: str | AuthoringColor | None = None,
        min_bar_height_ratio: int | float | None = None,
        layout: Spectrum2DLayout | None = None,
        gradient: Spectrum2DGradient | None | _Unset = _UNSET,
    ) -> None:
        values = {
            "band_count": band_count,
            "min_hz": min_hz,
            "max_hz": max_hz,
            "sensitivity": sensitivity,
            "attack_seconds": attack_seconds,
            "release_seconds": release_seconds,
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "bar_gap_ratio": bar_gap_ratio,
            "colour": colour,
            "min_bar_height_ratio": min_bar_height_ratio,
            "layout": layout,
            "gradient": gradient,
        }
        self._preset = preset
        overrides_values: dict[str, object] = {
            name: (
                color_to_canonical(cast(str | AuthoringColor, value))
                if name == "colour" and value is not None
                else value
            )
            for name, value in values.items()
            if value is not None and value is not _UNSET
        }
        self._overrides = overrides_values
        overrides = {
            name: value if name in self._overrides else _UNSET
            for name, value in values.items()
        }
        if gradient is None:
            self._overrides["gradient"] = None
            overrides["gradient"] = None
        resolved, _ = _resolve_spectrum2d_source(preset, overrides)
        self._values = dict(resolved)
        self._validate()

    @property
    def preset(self) -> Spectrum2DPreset | None:
        return self._preset

    def _validate(self) -> None:
        values = self._values
        Spectrum2DClip._create(
            _Owner(),
            "spectrum",
            start=0,
            duration=1,
            layer=0,
            visible=True,
            opacity=1,
            band_count=cast(int, values["band_count"]),
            min_hz=cast(float, values["min_hz"]),
            max_hz=cast(float, values["max_hz"]),
            sensitivity=cast(float, values["sensitivity"]),
            attack_seconds=cast(float, values["attack_seconds"]),
            release_seconds=cast(float, values["release_seconds"]),
            x=cast(float, values["x"]),
            y=cast(float, values["y"]),
            width=cast(float, values["width"]),
            height=cast(float, values["height"]),
            bar_gap_ratio=cast(float, values["bar_gap_ratio"]),
            colour=cast(str, values["colour"]),
            min_bar_height_ratio=cast(float, values["min_bar_height_ratio"]),
            layout=values["layout"],
            gradient=values.get("gradient"),
        )

    def value(self, name: str) -> object:
        if name not in _SPECTRUM_FIELDS:
            raise AttributeError(name)
        return self._values[name]

    @property
    def band_count(self) -> int:
        return cast(int, self._values["band_count"])

    @property
    def min_hz(self) -> float:
        return cast(float, self._values["min_hz"])

    @property
    def max_hz(self) -> float:
        return cast(float, self._values["max_hz"])

    @property
    def sensitivity(self) -> float:
        return cast(float, self._values["sensitivity"])

    @property
    def attack_seconds(self) -> float:
        return cast(float, self._values["attack_seconds"])

    @property
    def release_seconds(self) -> float:
        return cast(float, self._values["release_seconds"])

    @property
    def x(self) -> float:
        return cast(float, self._values["x"])

    @property
    def y(self) -> float:
        return cast(float, self._values["y"])

    @property
    def width(self) -> float:
        return cast(float, self._values["width"])

    @property
    def height(self) -> float:
        return cast(float, self._values["height"])

    @property
    def bar_gap_ratio(self) -> float:
        return cast(float, self._values["bar_gap_ratio"])

    @property
    def colour(self) -> str:
        return cast(str, self._values["colour"])

    @property
    def min_bar_height_ratio(self) -> float:
        return cast(float, self._values["min_bar_height_ratio"])

    @property
    def layout(self) -> Spectrum2DLayout:
        return cast(Spectrum2DLayout, self._values["layout"])

    @property
    def gradient(self) -> Spectrum2DGradient | None:
        return cast(Spectrum2DGradient | None, self._values.get("gradient"))

    def lowering_kwargs(self) -> dict[str, object]:
        return dict(self._overrides)


__all__ = [
    "Image",
    "Color",
    "SolidColor",
    "Source",
    "SizingValue",
    "Sizing",
    "Crop",
    "Point",
    "ParticleSystem",
    "PointEmitter",
    "RectangleEmitter",
    "CircleEmitter",
    "ParticleBurst",
    "ScalarRange",
    "ParticlePrimitive",
    "ParticleBlendMode",
    "ParticleLifetimeStyle",
    "ScalarLifetimeStop",
    "ColourLifetimeStop",
    "ParticleAudioReactive",
    "Spectrum2D",
    "Spectrum2DPreset",
    "Spectrum2DGradient",
    "Spectrum2DLayout",
    "Spectrum2DLinearLayout",
    "Spectrum2DRadialLayout",
]
