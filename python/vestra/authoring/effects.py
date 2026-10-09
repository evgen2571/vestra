"""Typed, ordered effect handles for canonical project effects."""

from __future__ import annotations

from dataclasses import dataclass
from collections.abc import Sequence
from enum import Enum
from functools import lru_cache
from types import MappingProxyType
from typing import Callable, Literal, Mapping, Self, TypeVar, cast

from .assets import FontAsset
from ._internal import _IdAllocator, _Owner, _number
from .signals import ScalarSignal
from .tracks import ModulatableScalarTrack, PointTrack, ScalarModifierTarget, ScalarTrack
from .values import Color, CubicBezier, Interpolation, Point, color_to_canonical
from vestra._native import effect_definitions as _native_effect_definitions


@dataclass(frozen=True, slots=True)
class ActiveInterval:
    """A finite half-open interval, measured in clip-local seconds."""

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


class AsciiGlyphStyle(Enum):
    CHARACTERS = "characters"
    GEOMETRIC = "geometric"


class AsciiMode(Enum):
    FILL = "fill"
    EDGES = "edges"
    HYBRID = "hybrid"


class AsciiColorMode(Enum):
    MONOCHROME = "monochrome"
    SOURCE = "source"
    PALETTE = "palette"
    RAINBOW = "rainbow"


class PaletteMode(Enum):
    GRADIENT = "gradient"
    NEAREST = "nearest"
    RAINBOW = "rainbow"
    NEAREST_RGB = "nearest_rgb"
    NEAREST_HUE = "nearest_hue"
    RGB_CHANNELS = "rgb_channels"
    NEAREST_OKLAB = "nearest_oklab"


class HalftoneMode(Enum):
    LUMINANCE = "luminance"
    SOURCE = "source"
    RGB = "rgb"


class PixelSortDirection(Enum):
    HORIZONTAL = "horizontal"
    VERTICAL = "vertical"


class PixelSortOrder(Enum):
    ASCENDING = "ascending"
    DESCENDING = "descending"


class DitherMatrix(Enum):
    BAYER2 = "bayer2"
    BAYER4 = "bayer4"
    BAYER8 = "bayer8"
    BLUE_NOISE = "blue_noise"


@lru_cache(maxsize=1)
def _effect_catalog() -> tuple[Mapping[str, object], ...]:
    return cast(
        tuple[Mapping[str, object], ...],
        tuple(_freeze_metadata(definition) for definition in _native_effect_definitions()),
    )


def _freeze_metadata(value: object) -> object:
    if isinstance(value, dict):
        return MappingProxyType({key: _freeze_metadata(item) for key, item in value.items()})
    if isinstance(value, (list, tuple)):
        return tuple(_freeze_metadata(item) for item in value)
    return value


def available_effects() -> tuple[Mapping[str, object], ...]:
    """Return the registered Rust visual-effect descriptors read-only."""
    return _effect_catalog()


def effect_definition(effect_type: str) -> Mapping[str, object]:
    """Return one registered visual-effect descriptor."""
    for definition in _effect_catalog():
        if definition["id"] == effect_type:
            return definition
    raise ValueError(f"unknown visual effect type: {effect_type!r}")


def _parameter_descriptor(definition: Mapping[str, object], name: str) -> Mapping[str, object]:
    parameters = cast(tuple[Mapping[str, object], ...], definition["parameters"])
    for parameter in parameters:
        if parameter["name"] == name:
            return parameter
    names = ", ".join(str(parameter["name"]) for parameter in parameters)
    raise TypeError(f"unknown parameter {name!r} for effect {definition['id']!r}; expected: {names}")


def _integer_in_range(value: int, name: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{name} must be an integer")
    if not minimum <= value <= maximum:
        raise ValueError(f"{name} must be between {minimum} and {maximum}")
    return value


def _validate_descriptor_value(parameter: Mapping[str, object], value: float) -> None:
    minimum = cast(float | None, parameter["minimum"])
    maximum = cast(float | None, parameter["maximum"])
    minimum_exclusive = cast(bool, parameter["minimum_exclusive"])
    maximum_exclusive = cast(bool, parameter["maximum_exclusive"])
    if minimum is not None and (value <= minimum if minimum_exclusive else value < minimum):
        raise ValueError(f"{parameter['name']} is outside its authored range")
    if maximum is not None and (value >= maximum if maximum_exclusive else value > maximum):
        raise ValueError(f"{parameter['name']} is outside its authored range")


def _palette(parameter: Mapping[str, object], value: object) -> tuple[str, ...]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence):
        raise TypeError("palette must be a sequence of colors")
    minimum = cast(int, parameter["integer_minimum"])
    maximum = cast(int, parameter["integer_maximum"])
    if not minimum <= len(value) <= maximum:
        raise ValueError(f"palette must contain {minimum}..{maximum} colors")
    colors = tuple(color_to_canonical(item) for item in value)
    if any(len(color) == 9 and color[-2:] != "ff" for color in colors):
        raise ValueError("palette colors must be opaque")
    return colors


def _palette_stops(value: object) -> tuple[float, ...] | None:
    if value is None:
        return None
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise TypeError("stops must be a sequence of numbers or None")
    stops = tuple(_number(item, "stops") for item in value)
    if not 2 <= len(stops) <= 16 or stops[0] != 0 or stops[-1] != 1:
        raise ValueError("stops require 2..16 positions with endpoints 0 and 1")
    rounded = tuple(int(stop * 65280 + 0.5) for stop in stops)
    if any(not 0 <= stop <= 1 for stop in stops) or any(a >= b for a, b in zip(rounded, rounded[1:])):
        raise ValueError("stops must increase after rounding to 1/65280")
    return stops


def _canonical_parameter(
    parameter: Mapping[str, object], value: object, owner: _Owner, *, validate_descriptor_values: bool,
) -> object:
    kind = parameter["kind"]
    name = str(parameter["name"])
    if kind == "scalar_property":
        # Existing tracks are copied into canonical data. The generic effect
        # never retains a live reference to another builder's track.
        track = value if isinstance(value, ScalarTrack) else ModulatableScalarTrack._create(owner, cast(int | float, value))
        if validate_descriptor_values:
            _validate_descriptor_value(parameter, track.base_value)
            for keyframe in track.keyframes:
                _validate_descriptor_value(parameter, keyframe.value)
        return track.to_canonical()
    if kind == "plain_track":
        track = value if isinstance(value, ScalarTrack) else ScalarTrack._create(owner, cast(float, value))
        if validate_descriptor_values:
            _validate_descriptor_value(parameter, track.base_value)
            for keyframe in track.keyframes:
                _validate_descriptor_value(parameter, keyframe.value)
        return track.to_canonical()
    if kind == "string":
        if not isinstance(value, str) or not value or any(ord(char) < 32 or 127 <= ord(char) <= 159 for char in value):
            raise ValueError(f"{name} must be a nonempty string without control characters")
        if name == "characters" and not 1 <= len(value) <= 256 or name == "edge_characters" and len(value) != 4:
            raise ValueError(f"{name} has an invalid character count")
        return value
    if kind == "font":
        from .assets import FontAsset
        if value is None:
            return None
        if not isinstance(value, FontAsset) or value._owner is not owner:
            raise ValueError("font must be a FontAsset belonging to this builder")
        return value.id
    if kind == "colour":
        return color_to_canonical(cast(Color | str, value))
    if kind == "palette_stops":
        stops = _palette_stops(value)
        return None if stops is None else list(stops)
    if kind == "palette":
        return list(_palette(parameter, value))
    if kind == "period":
        if value is None:
            return None
        number = _number(cast(int | float, value), name)
        _validate_descriptor_value(parameter, number)
        return number
    if kind == "integer":
        minimum = cast(int | None, parameter["integer_minimum"])
        maximum = cast(int | None, parameter["integer_maximum"])
        assert minimum is not None and maximum is not None
        return _integer_in_range(cast(int, value), name, minimum, maximum)
    if kind == "number":
        number = _number(cast(int | float, value), name)
        if validate_descriptor_values:
            _validate_descriptor_value(parameter, number)
        return number
    if kind in {"point2d", "point_property"}:
        if not isinstance(value, Point):
            raise TypeError(f"{name} must be Point")
        if not 0 <= value.x <= 1 or not 0 <= value.y <= 1:
            raise ValueError(f"{name} must be within unit space")
        return value.to_canonical()
    if kind == "boolean":
        if not isinstance(value, bool):
            raise TypeError(f"{name} must be a boolean")
        return value
    if kind == "enum":
        candidate = value.value if isinstance(value, Enum) else value
        enum_values = cast(tuple[object, ...], parameter["enum_values"])
        values = tuple(str(item) for item in enum_values)
        if not isinstance(candidate, str):
            raise TypeError(f"{name} must be a string or enum value")
        if candidate not in values:
            raise ValueError(f"{name} must be one of: {', '.join(values)}")
        return candidate
    if kind == "active_interval":
        if not isinstance(value, ActiveInterval):
            raise TypeError(f"{name} must be ActiveInterval")
        return value.to_canonical()
    raise TypeError(f"unsupported authoring parameter kind: {kind!r}")


def _build_registered_effect(
    effect_type: str,
    owner: _Owner,
    scope: object,
    identifier: str,
    supplied: Mapping[str, object],
    *,
    validate_descriptor_values: bool,
) -> dict[str, object]:
    """Build one canonical registered effect from the Rust descriptor."""
    definition = effect_definition(effect_type)
    parameters = cast(tuple[Mapping[str, object], ...], definition["parameters"])
    descriptors = {str(parameter["name"]): parameter for parameter in parameters}
    values: dict[str, object] = {}
    provided_parameters = set(supplied)
    for name, value in supplied.items():
        parameter = _parameter_descriptor(definition, name)
        canonical = _canonical_parameter(
            parameter, value, owner, validate_descriptor_values=validate_descriptor_values,
        )
        if parameter["kind"] == "active_interval":
            values.update(cast(Mapping[str, object], canonical))
        elif parameter["kind"] in {"period", "font", "palette_stops"} and canonical is None:
            continue
        else:
            values[name] = canonical
    for name, parameter in descriptors.items():
        if name in provided_parameters:
            continue
        if parameter["required"]:
            raise TypeError(f"missing required parameter {name!r} for effect {effect_type!r}")
        if parameter["default"] is not None:
            values[name] = parameter["default"]
        elif parameter["kind"] == "active_interval":
            values.update(ActiveInterval().to_canonical())
    return {"id": identifier, "type": effect_type, **values}


def _canonical_typed_effect(effect: Effect, supplied: Mapping[str, object]) -> dict[str, object]:
    return _build_registered_effect(
        effect.kind, effect._owner, effect._scope, effect.id, supplied,
        validate_descriptor_values=False,
    )


class Effect:
    """Base class for factory-created effects. It has no public parameters."""

    __slots__ = ("_owner", "_scope", "_id", "_kind")
    _owner: _Owner
    _scope: object
    _id: str
    _kind: str

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
        return (
            isinstance(other, Effect)
            and self._owner is other._owner
            and self._scope is other._scope
            and self.id == other.id
            and self.kind == other.kind
        )

    def __repr__(self) -> str:
        return f"{type(self).__name__}(id={self.id!r})"

    def _canonical(self) -> dict[str, object]:
        return {"id": self.id, "type": self.kind}

    def to_canonical(self) -> dict[str, object]:
        raise NotImplementedError


class _EffectPointTrack(PointTrack):
    """Private point track used only by generic dynamic effect lowering."""

    __slots__ = ("_x", "_y")

    @classmethod
    def _create(cls, owner: _Owner, value: Point) -> "_EffectPointTrack":
        instance = super()._create(owner, value)
        instance._x = ScalarModifierTarget._create(owner)
        instance._y = ScalarModifierTarget._create(owner)
        return instance

    @property
    def x(self) -> ScalarModifierTarget:
        return self._x

    @property
    def y(self) -> ScalarModifierTarget:
        return self._y

    def modulate(
        self,
        signal: ScalarSignal,
        *,
        mode: Literal["replace", "add", "multiply"] = "add",
    ) -> Self:
        if not isinstance(signal, ScalarSignal):
            raise TypeError("signal must be ScalarSignal")
        if mode not in {"replace", "add", "multiply"}:
            raise ValueError("mode must be replace, add, or multiply")
        self._modifiers.append({"operation": mode, "signal": signal})
        return self

    react_to = modulate


class GenericEffect(Effect):
    """Registered effect handle used by the generic authoring escape hatch."""

    __slots__ = ("_data",)
    _data: dict[str, object]

    @classmethod
    def _create(
        cls, owner: _Owner, scope: object, identifier: str, definition: Mapping[str, object], values: Mapping[str, object]
    ) -> "GenericEffect":
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, str(definition["id"]))
        instance._data = {"id": identifier, "type": definition["id"], **values}
        # Generic effects are also used as a lowering seam.  Keep scalar
        # parameters as fresh owner-bound tracks, while retaining the same
        # canonical representation as the historical dictionary path.
        parameters = cast(tuple[Mapping[str, object], ...], definition["parameters"])
        for parameter in parameters:
            name = str(parameter["name"])
            if parameter["kind"] in {"point2d", "point_property"} and name in {
                "tile_center",
                "center",
            }:
                canonical = instance._data.get(name)
                if not isinstance(canonical, Mapping):
                    continue
                if "base_value" in canonical:
                    base = cast(Mapping[str, object], canonical["base_value"])
                    frames = cast(list[Mapping[str, object]], canonical.get("keyframes", []))
                    modifiers = cast(list[Mapping[str, object]], canonical.get("modifiers", []))
                    components = cast(Mapping[str, object], canonical.get("component_modifiers", {}))
                else:
                    base = canonical
                    frames = []
                    modifiers = []
                    components = {}
                track = _EffectPointTrack._create(
                    owner, Point(cast(float, base["x"]), cast(float, base["y"]))
                )
                for frame in frames:
                    interpolation = frame["interpolation"]
                    if isinstance(interpolation, Mapping):
                        interpolation = CubicBezier(
                            cast(float, interpolation["x1"]),
                            cast(float, interpolation["y1"]),
                            cast(float, interpolation["x2"]),
                            cast(float, interpolation["y2"]),
                        )
                    else:
                        interpolation = Interpolation(cast(str, interpolation))
                    value = cast(Mapping[str, object], frame["value"])
                    track.keyframe(
                        time=cast(float, frame["time"]),
                        value=Point(cast(float, value["x"]), cast(float, value["y"])),
                        interpolation=interpolation,
                    )
                for modifier in modifiers:
                    signal = cast(Mapping[str, object], modifier["signal"])
                    source = cast(Mapping[str, object], signal["source"])
                    feature = cast(Mapping[str, object], source["feature"])
                    transforms = cast(
                        tuple[Mapping[str, object], ...],
                        tuple(cast(list[Mapping[str, object]], signal.get("transforms", []))),
                    )
                    track.modulate(
                        ScalarSignal(feature, transforms),
                        mode=cast(Literal["replace", "add", "multiply"], modifier["operation"]),
                    )
                for component_name, target in (("x", track.x), ("y", track.y)):
                    for modifier in cast(list[Mapping[str, object]], components.get(component_name, [])):
                        signal = cast(Mapping[str, object], modifier["signal"])
                        source = cast(Mapping[str, object], signal["source"])
                        feature = cast(Mapping[str, object], source["feature"])
                        transforms = cast(
                            tuple[Mapping[str, object], ...],
                            tuple(cast(list[Mapping[str, object]], signal.get("transforms", []))),
                        )
                        target.modulate(
                            ScalarSignal(feature, transforms),
                            mode=cast(Literal["replace", "add", "multiply"], modifier["operation"]),
                        )
                instance._data[name] = track
                continue
            if parameter["kind"] not in {"scalar_property", "plain_track"}:
                continue
            canonical = instance._data.get(name)
            if not isinstance(canonical, Mapping):
                continue
            track_type = ModulatableScalarTrack if parameter["kind"] == "scalar_property" else ScalarTrack
            track = track_type._create(owner, cast(int | float, canonical["base_value"]))
            for frame in cast(list[Mapping[str, object]], canonical.get("keyframes", [])):
                interpolation = frame["interpolation"]
                if isinstance(interpolation, Mapping):
                    interpolation = CubicBezier(
                        cast(float, interpolation["x1"]),
                        cast(float, interpolation["y1"]),
                        cast(float, interpolation["x2"]),
                        cast(float, interpolation["y2"]),
                    )
                else:
                    interpolation = Interpolation(cast(str, interpolation))
                track.keyframe(
                    time=cast(float, frame["time"]), value=cast(float, frame["value"]),
                    interpolation=interpolation,
                )
            if isinstance(track, ModulatableScalarTrack):
                for modifier in cast(
                    list[Mapping[str, object]], canonical.get("modifiers", [])
                ):
                    signal = cast(Mapping[str, object], modifier["signal"])
                    source = cast(Mapping[str, object], signal["source"])
                    feature = cast(Mapping[str, object], source["feature"])
                    transforms = cast(
                        tuple[Mapping[str, object], ...],
                        tuple(cast(list[Mapping[str, object]], signal.get("transforms", []))),
                    )
                    track.modulate(
                        ScalarSignal(feature, transforms),
                        mode=cast(
                            Literal["replace", "add", "multiply"],
                            modifier["operation"],
                        ),
                    )
            instance._data[name] = track
        return instance

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {}
        for key, value in {**self._data, "id": self.id}.items():
            if isinstance(value, (ScalarTrack, PointTrack)):
                canonical = value.to_canonical()
                if isinstance(value, PointTrack):
                    components = {
                        name: [
                            {"operation": item["operation"], "signal": item["signal"].to_canonical()}
                            for item in target._modifiers
                        ]
                        for name, target in (("x", value.x), ("y", value.y))
                        if target._modifiers
                    }
                    if components:
                        canonical["component_modifiers"] = components
                data[key] = canonical
            else:
                data[key] = list(value) if isinstance(value, list) else value
        return data

    def parameter_track(self, name: str) -> ScalarTrack:
        """Return the builder-owned track for one scalar effect parameter."""
        value = self._data.get(name)
        if not isinstance(value, ScalarTrack):
            raise TypeError(f"effect parameter {name!r} is not a scalar track")
        return value

    def parameter_point_track(self, name: str) -> PointTrack:
        """Return the builder-owned track for one point effect parameter."""
        value = self._data.get(name)
        if not isinstance(value, PointTrack):
            raise TypeError(f"effect parameter {name!r} is not a point track")
        return value

    def parameter_point_component(self, name: str, component: Literal["x", "y"]):
        """Return the modifier target for one point effect component."""
        track = self.parameter_point_track(name)
        return track.x if component == "x" else track.y


class _AmountEffect(Effect):
    """Private implementation shared only by effects with one amount track."""

    __slots__ = ("_amount",)
    _amount: ModulatableScalarTrack

    @classmethod
    def _create(
        cls, owner: _Owner, scope: object, identifier: str, kind: str, amount: int | float
    ) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, kind)
        instance._amount = ModulatableScalarTrack._create(owner, amount)
        return instance

    @property
    def amount(self) -> ModulatableScalarTrack:
        return self._amount

    def to_canonical(self) -> dict[str, object]:
        return {**self._canonical(), "amount": self.amount.to_canonical()}


class BrightnessEffect(_AmountEffect):
    def to_canonical(self) -> dict[str, object]:
        return _canonical_typed_effect(self, {"amount": self.amount})


class ContrastEffect(_AmountEffect):
    pass


class SaturationEffect(_AmountEffect):
    pass


class TintEffect(_AmountEffect):
    __slots__ = ("_colour",)
    _colour: str

    @classmethod
    def _create(
        cls, owner: _Owner, scope: object, identifier: str, colour: Color | str, amount: int | float
    ) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "tint")
        instance._amount = ModulatableScalarTrack._create(owner, amount)
        instance._colour = color_to_canonical(colour)
        return instance

    @property
    def colour(self) -> str:
        return self._colour

    @colour.setter
    def colour(self, value: Color | str) -> None:
        self._colour = color_to_canonical(value)

    def to_canonical(self) -> dict[str, object]:
        return {**self._canonical(), "colour": self.colour, "amount": self.amount.to_canonical()}


class GaussianBlurEffect(Effect):
    __slots__ = ("_radius",)
    _radius: ModulatableScalarTrack

    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, radius: int | float) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "gaussian_blur")
        instance._radius = ModulatableScalarTrack._create(owner, radius)
        return instance

    @property
    def radius(self) -> ModulatableScalarTrack:
        return self._radius

    def to_canonical(self) -> dict[str, object]:
        return {**self._canonical(), "radius": self.radius.to_canonical()}


class DirectionalBlurEffect(Effect):
    __slots__ = ("_radius", "_angle_degrees")
    _radius: ModulatableScalarTrack
    _angle_degrees: ModulatableScalarTrack

    @classmethod
    def _create(
        cls, owner: _Owner, scope: object, identifier: str, radius: int | float, angle_degrees: int | float
    ) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "directional_blur")
        instance._radius = ModulatableScalarTrack._create(owner, radius)
        instance._angle_degrees = ModulatableScalarTrack._create(owner, angle_degrees)
        return instance

    @property
    def radius(self) -> ModulatableScalarTrack:
        return self._radius

    @property
    def angle_degrees(self) -> ModulatableScalarTrack:
        return self._angle_degrees

    def to_canonical(self) -> dict[str, object]:
        return {**self._canonical(), "radius": self.radius.to_canonical(), "angle_degrees": self.angle_degrees.to_canonical()}


class ZoomBlurEffect(Effect):
    __slots__ = ("_radius", "_samples", "_anchor", "_direction")
    _radius: ModulatableScalarTrack
    _samples: int
    _anchor: Point
    _direction: ZoomBlurDirection

    @classmethod
    def _create(
        cls, owner: _Owner, scope: object, identifier: str, radius: int | float, samples: int,
        anchor: Point, direction: ZoomBlurDirection,
    ) -> Self:
        if not isinstance(anchor, Point):
            raise TypeError("anchor must be Point")
        if not 0 <= anchor.x <= 1 or not 0 <= anchor.y <= 1:
            raise ValueError("anchor must be within unit space")
        if not isinstance(direction, ZoomBlurDirection):
            raise TypeError("direction must be ZoomBlurDirection")
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "zoom_blur")
        instance._radius = ModulatableScalarTrack._create(owner, radius)
        instance._samples = _integer_in_range(samples, "samples", 2, 32)
        instance._anchor = Point(anchor.x, anchor.y)
        instance._direction = direction
        return instance

    @property
    def radius(self) -> ModulatableScalarTrack: return self._radius
    @property
    def samples(self) -> int: return self._samples
    @samples.setter
    def samples(self, value: int) -> None: self._samples = _integer_in_range(value, "samples", 2, 32)
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
    def to_canonical(self) -> dict[str, object]:
        return _canonical_typed_effect(self, {
            "radius": self.radius, "samples": self.samples, "anchor": self.anchor, "direction": self.direction,
        })


class GlowEffect(Effect):
    __slots__ = ("_threshold", "_radius", "_intensity", "_colour")
    _threshold: ModulatableScalarTrack
    _radius: ModulatableScalarTrack
    _intensity: ModulatableScalarTrack
    _colour: str

    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, threshold: int | float,
                radius: int | float, intensity: int | float, colour: Color | str) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "glow")
        instance._threshold = ModulatableScalarTrack._create(owner, threshold)
        instance._radius = ModulatableScalarTrack._create(owner, radius)
        instance._intensity = ModulatableScalarTrack._create(owner, intensity)
        instance._colour = color_to_canonical(colour)
        return instance

    @property
    def threshold(self) -> ModulatableScalarTrack: return self._threshold
    @property
    def radius(self) -> ModulatableScalarTrack: return self._radius
    @property
    def intensity(self) -> ModulatableScalarTrack: return self._intensity
    @property
    def colour(self) -> str: return self._colour
    @colour.setter
    def colour(self, value: Color | str) -> None: self._colour = color_to_canonical(value)
    def to_canonical(self) -> dict[str, object]:
        return _canonical_typed_effect(self, {
            "threshold": self.threshold, "radius": self.radius, "intensity": self.intensity, "colour": self.colour,
        })


class BloomEffect(Effect):
    __slots__ = ("_threshold", "_radius", "_intensity")
    _threshold: ModulatableScalarTrack
    _radius: ModulatableScalarTrack
    _intensity: ModulatableScalarTrack

    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, threshold: int | float, radius: int | float, intensity: int | float) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "bloom")
        instance._threshold = ModulatableScalarTrack._create(owner, threshold)
        instance._radius = ModulatableScalarTrack._create(owner, radius)
        instance._intensity = ModulatableScalarTrack._create(owner, intensity)
        return instance

    @property
    def threshold(self) -> ModulatableScalarTrack: return self._threshold
    @property
    def radius(self) -> ModulatableScalarTrack: return self._radius
    @property
    def intensity(self) -> ModulatableScalarTrack: return self._intensity

    def to_canonical(self) -> dict[str, object]:
        return _canonical_typed_effect(self, {"threshold": self.threshold, "radius": self.radius, "intensity": self.intensity})


class ChromaticAberrationEffect(Effect):
    __slots__ = ("_amount", "_angle_degrees")
    _amount: ModulatableScalarTrack
    _angle_degrees: ModulatableScalarTrack

    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, amount: int | float,
                angle_degrees: int | float) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "chromatic_aberration")
        instance._amount = ModulatableScalarTrack._create(owner, amount)
        instance._angle_degrees = ModulatableScalarTrack._create(owner, angle_degrees)
        return instance

    @property
    def amount(self) -> ModulatableScalarTrack: return self._amount
    @property
    def angle_degrees(self) -> ModulatableScalarTrack: return self._angle_degrees
    def to_canonical(self) -> dict[str, object]:
        return {**self._canonical(), "amount": self.amount.to_canonical(), "angle_degrees": self.angle_degrees.to_canonical()}


class VignetteEffect(Effect):
    __slots__ = ("_amount", "_radius", "_softness", "_colour")
    _amount: ModulatableScalarTrack
    _radius: ModulatableScalarTrack
    _softness: ScalarTrack
    _colour: str

    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, amount: int | float,
                radius: int | float, softness: int | float, colour: Color | str) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "vignette")
        instance._amount = ModulatableScalarTrack._create(owner, amount)
        instance._radius = ModulatableScalarTrack._create(owner, radius)
        instance._softness = ScalarTrack._create(owner, softness)
        instance._colour = color_to_canonical(colour)
        return instance

    @property
    def amount(self) -> ModulatableScalarTrack: return self._amount
    @property
    def radius(self) -> ModulatableScalarTrack: return self._radius
    @property
    def softness(self) -> ScalarTrack: return self._softness
    @property
    def colour(self) -> str: return self._colour
    @colour.setter
    def colour(self, value: Color | str) -> None: self._colour = color_to_canonical(value)
    def to_canonical(self) -> dict[str, object]:
        return {**self._canonical(), "amount": self.amount.to_canonical(), "radius": self.radius.to_canonical(),
                "softness": self.softness.to_canonical(), "colour": self.colour}


class SharpenEffect(Effect):
    __slots__ = ("_amount", "_radius")
    _amount: ModulatableScalarTrack
    _radius: ModulatableScalarTrack

    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, amount: int | float,
                radius: int | float) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "sharpen")
        instance._amount = ModulatableScalarTrack._create(owner, amount)
        instance._radius = ModulatableScalarTrack._create(owner, radius)
        return instance

    @property
    def amount(self) -> ModulatableScalarTrack: return self._amount
    @property
    def radius(self) -> ModulatableScalarTrack: return self._radius
    def to_canonical(self) -> dict[str, object]:
        return {**self._canonical(), "amount": self.amount.to_canonical(), "radius": self.radius.to_canonical()}


class ColorAdjustEffect(Effect):
    __slots__ = ("_exposure", "_gamma", "_black_point", "_white_point")
    _exposure: ModulatableScalarTrack
    _gamma: ModulatableScalarTrack
    _black_point: ScalarTrack
    _white_point: ScalarTrack

    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, exposure: int | float,
                gamma: int | float, black_point: int | float, white_point: int | float) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "color_adjust")
        instance._exposure = ModulatableScalarTrack._create(owner, exposure)
        instance._gamma = ModulatableScalarTrack._create(owner, gamma)
        instance._black_point = ScalarTrack._create(owner, black_point)
        instance._white_point = ScalarTrack._create(owner, white_point)
        return instance

    @property
    def exposure(self) -> ModulatableScalarTrack: return self._exposure
    @property
    def gamma(self) -> ModulatableScalarTrack: return self._gamma
    @property
    def black_point(self) -> ScalarTrack: return self._black_point
    @property
    def white_point(self) -> ScalarTrack: return self._white_point
    def to_canonical(self) -> dict[str, object]:
        return {**self._canonical(), "exposure": self.exposure.to_canonical(), "gamma": self.gamma.to_canonical(),
                "black_point": self.black_point.to_canonical(), "white_point": self.white_point.to_canonical()}


class CameraShakeEffect(Effect):
    __slots__ = ("_active_interval", "_position_amount", "_rotation_degrees", "_scale_amount", "_frequency", "_seed", "_attack", "_decay")
    _active_interval: ActiveInterval
    _position_amount: ModulatableScalarTrack
    _rotation_degrees: ModulatableScalarTrack
    _scale_amount: ModulatableScalarTrack
    _frequency: ModulatableScalarTrack
    _seed: int
    _attack: float
    _decay: float

    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, active_interval: ActiveInterval,
                position_amount: int | float, rotation_degrees: int | float, scale_amount: int | float,
                frequency: int | float, seed: int, attack: int | float, decay: int | float) -> Self:
        if not isinstance(active_interval, ActiveInterval): raise TypeError("active_interval must be ActiveInterval")
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "camera_shake")
        instance._active_interval = active_interval
        instance._position_amount = ModulatableScalarTrack._create(owner, position_amount)
        instance._rotation_degrees = ModulatableScalarTrack._create(owner, rotation_degrees)
        instance._scale_amount = ModulatableScalarTrack._create(owner, scale_amount)
        instance._frequency = ModulatableScalarTrack._create(owner, frequency)
        instance._seed = _integer_in_range(seed, "seed", 0, 2**64 - 1)
        instance._attack = _number(attack, "attack")
        instance._decay = _number(decay, "decay")
        return instance

    @property
    def active_interval(self) -> ActiveInterval: return self._active_interval
    @active_interval.setter
    def active_interval(self, value: ActiveInterval) -> None:
        if not isinstance(value, ActiveInterval): raise TypeError("active_interval must be ActiveInterval")
        self._active_interval = value
    @property
    def position_amount(self) -> ModulatableScalarTrack: return self._position_amount
    @property
    def rotation_degrees(self) -> ModulatableScalarTrack: return self._rotation_degrees
    @property
    def scale_amount(self) -> ModulatableScalarTrack: return self._scale_amount
    @property
    def frequency(self) -> ModulatableScalarTrack: return self._frequency
    @property
    def seed(self) -> int: return self._seed
    @seed.setter
    def seed(self, value: int) -> None: self._seed = _integer_in_range(value, "seed", 0, 2**64 - 1)
    @property
    def attack(self) -> float: return self._attack
    @attack.setter
    def attack(self, value: int | float) -> None: self._attack = _number(value, "attack")
    @property
    def decay(self) -> float: return self._decay
    @decay.setter
    def decay(self, value: int | float) -> None: self._decay = _number(value, "decay")
    def to_canonical(self) -> dict[str, object]:
        return _canonical_typed_effect(self, {
            "active_interval": self.active_interval, "position_amount": self.position_amount,
            "rotation_degrees": self.rotation_degrees, "scale_amount": self.scale_amount,
            "frequency": self.frequency, "seed": self.seed, "attack": self.attack, "decay": self.decay,
        })


class MotionBlurEffect(Effect):
    __slots__ = ("_intensity", "_shutter_angle", "_max_radius", "_samples")
    _intensity: ModulatableScalarTrack
    _shutter_angle: ModulatableScalarTrack
    _max_radius: ModulatableScalarTrack
    _samples: int

    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, intensity: int | float,
                shutter_angle: int | float, max_radius: int | float, samples: int) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "motion_blur")
        instance._intensity = ModulatableScalarTrack._create(owner, intensity)
        instance._shutter_angle = ModulatableScalarTrack._create(owner, shutter_angle)
        instance._max_radius = ModulatableScalarTrack._create(owner, max_radius)
        instance._samples = _integer_in_range(samples, "samples", 2, 32)
        return instance

    @property
    def intensity(self) -> ModulatableScalarTrack: return self._intensity
    @property
    def shutter_angle(self) -> ModulatableScalarTrack: return self._shutter_angle
    @property
    def max_radius(self) -> ModulatableScalarTrack: return self._max_radius
    @property
    def samples(self) -> int: return self._samples
    @samples.setter
    def samples(self, value: int) -> None: self._samples = _integer_in_range(value, "samples", 2, 32)
    def to_canonical(self) -> dict[str, object]:
        return {**self._canonical(), "intensity": self.intensity.to_canonical(), "shutter_angle": self.shutter_angle.to_canonical(),
                "max_radius": self.max_radius.to_canonical(), "samples": self.samples}


EffectType = TypeVar("EffectType", bound=Effect)


class HalftoneEffect(GenericEffect):
    __slots__ = ()
    _effect_type = "halftone"

    @classmethod
    def _create_stylized(cls, owner: _Owner, scope: object, identifier: str, parameters: Mapping[str, object]) -> Self:
        definition = effect_definition(cls._effect_type)
        values = _build_registered_effect(cls._effect_type, owner, scope, identifier, parameters, validate_descriptor_values=True)
        values.pop("id")
        values.pop("type")
        return cast(Self, super()._create(owner, scope, identifier, definition, values))

    @property
    def cell_size(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("cell_size"))

    @property
    def angle_degrees(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("angle_degrees"))

    @property
    def softness(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("softness"))

    @property
    def amount(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("amount"))

    @property
    def mode(self) -> HalftoneMode:
        return HalftoneMode(self._data["mode"])

    @mode.setter
    def mode(self, value: HalftoneMode | str) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), "mode")
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if canonical is None:
            self._data.pop("mode", None)
        else:
            self._data["mode"] = canonical

    @property
    def foreground(self) -> str:
        return cast(str, self._data["foreground"])

    @foreground.setter
    def foreground(self, value: Color | str) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), "foreground")
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if canonical is None:
            self._data.pop("foreground", None)
        else:
            self._data["foreground"] = canonical

    @property
    def background(self) -> str:
        return cast(str, self._data["background"])

    @background.setter
    def background(self, value: Color | str) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), "background")
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if canonical is None:
            self._data.pop("background", None)
        else:
            self._data["background"] = canonical

    @property
    def invert(self) -> bool:
        return cast(bool, self._data["invert"])

    @invert.setter
    def invert(self, value: bool) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), "invert")
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if canonical is None:
            self._data.pop("invert", None)
        else:
            self._data["invert"] = canonical

class PixelSortEffect(GenericEffect):
    __slots__ = ()
    _effect_type = "pixel_sort"

    @classmethod
    def _create_stylized(cls, owner: _Owner, scope: object, identifier: str, parameters: Mapping[str, object]) -> Self:
        definition = effect_definition(cls._effect_type)
        values = _build_registered_effect(cls._effect_type, owner, scope, identifier, parameters, validate_descriptor_values=True)
        values.pop("id")
        values.pop("type")
        return cast(Self, super()._create(owner, scope, identifier, definition, values))

    @property
    def lower_threshold(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("lower_threshold"))

    @property
    def upper_threshold(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("upper_threshold"))

    @property
    def amount(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("amount"))

    @property
    def direction(self) -> PixelSortDirection:
        return PixelSortDirection(self._data["direction"])

    @direction.setter
    def direction(self, value: PixelSortDirection | str) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), "direction")
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if canonical is None:
            self._data.pop("direction", None)
        else:
            self._data["direction"] = canonical

    @property
    def order(self) -> PixelSortOrder:
        return PixelSortOrder(self._data["order"])

    @order.setter
    def order(self, value: PixelSortOrder | str) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), "order")
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if canonical is None:
            self._data.pop("order", None)
        else:
            self._data["order"] = canonical

    @property
    def segment_length(self) -> int:
        return cast(int, self._data["segment_length"])

    @segment_length.setter
    def segment_length(self, value: int) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), "segment_length")
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if canonical is None:
            self._data.pop("segment_length", None)
        else:
            self._data["segment_length"] = canonical

class CrtEffect(GenericEffect):
    __slots__ = ()
    _effect_type = "crt"

    @classmethod
    def _create_stylized(cls, owner: _Owner, scope: object, identifier: str, parameters: Mapping[str, object]) -> Self:
        definition = effect_definition(cls._effect_type)
        values = _build_registered_effect(cls._effect_type, owner, scope, identifier, parameters, validate_descriptor_values=True)
        values.pop("id")
        values.pop("type")
        return cast(Self, super()._create(owner, scope, identifier, definition, values))

    @property
    def amount(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("amount"))

    @property
    def curvature(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("curvature"))

    @property
    def scanline_strength(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("scanline_strength"))

    @property
    def scanline_spacing(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("scanline_spacing"))

    @property
    def mask_strength(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("mask_strength"))

    @property
    def grain(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("grain"))

    @property
    def jitter(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("jitter"))

    @property
    def flicker(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("flicker"))

    @property
    def rolling_strength(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("rolling_strength"))

    @property
    def rolling_width(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("rolling_width"))

    @property
    def phase(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("phase"))

    @property
    def mask_spacing(self) -> int:
        return cast(int, self._data["mask_spacing"])

    @mask_spacing.setter
    def mask_spacing(self, value: int) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), "mask_spacing")
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if canonical is None:
            self._data.pop("mask_spacing", None)
        else:
            self._data["mask_spacing"] = canonical

    @property
    def period(self) -> float | None:
        return cast(float | None, self._data.get("period"))

    @period.setter
    def period(self, value: int | float | None) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), "period")
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if canonical is None:
            self._data.pop("period", None)
        else:
            self._data["period"] = canonical

    @property
    def seed(self) -> int:
        return cast(int, self._data["seed"])

    @seed.setter
    def seed(self, value: int) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), "seed")
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if canonical is None:
            self._data.pop("seed", None)
        else:
            self._data["seed"] = canonical

class PaletteMapEffect(GenericEffect):
    """Palette coloring with builder-owned animatable amount and phase tracks."""

    __slots__ = ()
    _effect_type = "palette_map"

    @classmethod
    def _create_palette(
        cls, owner: _Owner, scope: object, identifier: str, parameters: Mapping[str, object],
    ) -> Self:
        definition = effect_definition(cls._effect_type)
        values = _build_registered_effect(
            cls._effect_type, owner, scope, identifier, parameters,
            validate_descriptor_values=True,
        )
        values.pop("id")
        values.pop("type")
        return cast(Self, super()._create(owner, scope, identifier, definition, values))

    def _set_parameter(self, name: str, value: object) -> None:
        parameter = _parameter_descriptor(effect_definition(self.kind), name)
        canonical = _canonical_parameter(parameter, value, self._owner, validate_descriptor_values=True)
        if parameter["kind"] in {"period", "font", "palette_stops"} and canonical is None:
            self._data.pop(name, None)
        else:
            self._data[name] = canonical

    @property
    def palette(self) -> tuple[str, ...]: return tuple(cast(list[str], self._data["palette"]))
    @palette.setter
    def palette(self, value: Sequence[Color | str]) -> None: self._set_parameter("palette", value)
    @property
    def amount(self) -> ModulatableScalarTrack: return cast(ModulatableScalarTrack, self.parameter_track("amount"))
    @property
    def phase(self) -> ModulatableScalarTrack: return cast(ModulatableScalarTrack, self.parameter_track("phase"))
    @property
    def period(self) -> float | None: return cast(float | None, self._data.get("period"))
    @period.setter
    def period(self, value: int | float | None) -> None: self._set_parameter("period", value)
    @property
    def mode(self) -> PaletteMode: return PaletteMode(self._data["mode"])
    @mode.setter
    def mode(self, value: PaletteMode | str) -> None: self._set_parameter("mode", value)

    @property
    def stops(self) -> tuple[float, ...] | None:
        value = self._data.get("stops")
        return None if value is None else tuple(cast(list[float], value))
    @stops.setter
    def stops(self, value: Sequence[int | float] | None) -> None: self._set_parameter("stops", value)

    @property
    def levels(self) -> int: return cast(int, self._data["levels"])
    @levels.setter
    def levels(self, value: int) -> None: self._set_parameter("levels", value)


class OrderedDitherEffect(PaletteMapEffect):
    """Bayer/blue-noise quantization with a palette-independent threshold pattern."""

    __slots__ = ()
    _effect_type = "ordered_dither"

    @property
    def strength(self) -> ModulatableScalarTrack: return cast(ModulatableScalarTrack, self.parameter_track("strength"))
    @property
    def matrix(self) -> DitherMatrix: return DitherMatrix(self._data["matrix"])
    @matrix.setter
    def matrix(self, value: DitherMatrix | str) -> None: self._set_parameter("matrix", value)
    @property
    def scale(self) -> int: return cast(int, self._data["scale"])
    @scale.setter
    def scale(self, value: int) -> None: self._set_parameter("scale", value)

    @property
    def seed(self) -> int: return cast(int, self._data["seed"])
    @seed.setter
    def seed(self, value: int) -> None: self._set_parameter("seed", value)


class AsciiEffect(PaletteMapEffect):
    """Builder-owned ASCII controls and reusable prepared glyph resources."""
    __slots__ = ()
    _effect_type = "ascii"

    @property
    def cell_width(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("cell_width"))
    @property
    def cell_height(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("cell_height"))
    @property
    def edge_threshold(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("edge_threshold"))
    @property
    def edge_strength(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("edge_strength"))
    @property
    def source_mix(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("source_mix"))
    @property
    def amount(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("amount"))
    @property
    def phase(self) -> ModulatableScalarTrack:
        return cast(ModulatableScalarTrack, self.parameter_track("phase"))
    @property
    def characters(self) -> str:
        return cast(str, self._data.get("characters"))
    @characters.setter
    def characters(self, value: str) -> None:
        self._set_parameter("characters", value)
    @property
    def edge_characters(self) -> str:
        return cast(str, self._data.get("edge_characters"))
    @edge_characters.setter
    def edge_characters(self, value: str) -> None:
        self._set_parameter("edge_characters", value)
    @property
    def glyph_style(self) -> AsciiGlyphStyle:
        return AsciiGlyphStyle(self._data["glyph_style"])
    @glyph_style.setter
    def glyph_style(self, value: AsciiGlyphStyle | str) -> None:
        self._set_parameter("glyph_style", value)
    @property
    def mode(self) -> AsciiMode:
        return AsciiMode(self._data["mode"])
    @mode.setter
    def mode(self, value: AsciiMode | str) -> None:
        self._set_parameter("mode", value)
    @property
    def color_mode(self) -> AsciiColorMode:
        return AsciiColorMode(self._data["color_mode"])
    @color_mode.setter
    def color_mode(self, value: AsciiColorMode | str) -> None:
        self._set_parameter("color_mode", value)
    @property
    def foreground(self) -> str:
        return cast(str, self._data.get("foreground"))
    @foreground.setter
    def foreground(self, value: Color | str) -> None:
        self._set_parameter("foreground", value)
    @property
    def background(self) -> str:
        return cast(str, self._data.get("background"))
    @background.setter
    def background(self, value: Color | str) -> None:
        self._set_parameter("background", value)
    @property
    def palette(self) -> tuple[str, ...]:
        return tuple(cast(list[str], self._data["palette"]))
    @palette.setter
    def palette(self, value: Sequence[Color | str]) -> None:
        self._set_parameter("palette", value)
    @property
    def invert(self) -> bool:
        return cast(bool, self._data.get("invert"))
    @invert.setter
    def invert(self, value: bool) -> None:
        self._set_parameter("invert", value)
    @property
    def period(self) -> float | None:
        return cast(float | None, self._data.get("period"))
    @period.setter
    def period(self, value: int | float | None) -> None:
        self._set_parameter("period", value)
    @property
    def font(self) -> str | None:
        return cast(str | None, self._data.get("font"))
    @font.setter
    def font(self, value: FontAsset | None) -> None:
        self._set_parameter("font", value)


class _EffectCollection:
    __slots__ = ("_owner", "_ids", "_scope", "_items")
    _owner: _Owner
    _ids: _IdAllocator
    _scope: object
    _items: list[Effect]

    def __init__(self) -> None:
        raise TypeError(f"{type(self).__name__} objects must be obtained from a clip or ProjectBuilder")

    @classmethod
    def _create(cls, owner: _Owner, ids: _IdAllocator, scope: object) -> Self:
        instance = object.__new__(cls)
        instance._owner = owner
        instance._ids = ids
        instance._scope = scope
        instance._items = []
        return instance

    @property
    def items(self) -> tuple[Effect, ...]: return tuple(self._items)
    def _identifier(self, identifier: str | None) -> str:
        if identifier is not None: self._ids.validate("effect", identifier, scope=self._scope)
        return self._ids.allocate("effect", scope=self._scope) if identifier is None else self._ids.reserve("effect", identifier, scope=self._scope)
    def _append(self, factory: Callable[..., EffectType], identifier: str | None, *args: object) -> EffectType:
        staged = factory(self._owner, self._scope, "", *args)
        staged._id = self._identifier(identifier)
        self._items.append(staged)
        return staged

    def add_effect(self, effect_type: str, /, *, id: str | None = None, **parameters: object) -> GenericEffect:
        """Author a registered visual effect through the Rust catalog."""
        definition = effect_definition(effect_type)
        if definition["scope"] == "clip_only" and isinstance(self, PostEffectCollection):
            raise ValueError(f"effect {effect_type!r} is only valid on clips")
        values = _build_registered_effect(
            effect_type, self._owner, self._scope, "", parameters,
            validate_descriptor_values=True,
        )
        values.pop("id")
        values.pop("type")
        return self._append(GenericEffect._create, id, definition, values)
    def add_brightness(self, *, amount: int | float, id: str | None = None) -> BrightnessEffect: return self._append(BrightnessEffect._create, id, "brightness", amount)
    def add_contrast(self, *, amount: int | float, id: str | None = None) -> ContrastEffect: return self._append(ContrastEffect._create, id, "contrast", amount)
    def add_saturation(self, *, amount: int | float, id: str | None = None) -> SaturationEffect: return self._append(SaturationEffect._create, id, "saturation", amount)
    def add_tint(self, *, colour: Color | str, amount: int | float, id: str | None = None) -> TintEffect: return self._append(TintEffect._create, id, colour, amount)
    def add_gaussian_blur(self, *, radius: int | float, id: str | None = None) -> GaussianBlurEffect: return self._append(GaussianBlurEffect._create, id, radius)
    def add_directional_blur(self, *, radius: int | float, angle_degrees: int | float, id: str | None = None) -> DirectionalBlurEffect: return self._append(DirectionalBlurEffect._create, id, radius, angle_degrees)
    def add_zoom_blur(self, *, radius: int | float, samples: int, anchor: Point, direction: ZoomBlurDirection = ZoomBlurDirection.CENTERED, id: str | None = None) -> ZoomBlurEffect: return self._append(ZoomBlurEffect._create, id, radius, samples, anchor, direction)
    def add_glow(self, *, threshold: int | float, radius: int | float, intensity: int | float, colour: Color | str, id: str | None = None) -> GlowEffect: return self._append(GlowEffect._create, id, threshold, radius, intensity, colour)
    def add_bloom(self, *, threshold: int | float, radius: int | float, intensity: int | float, id: str | None = None) -> BloomEffect: return self._append(BloomEffect._create, id, threshold, radius, intensity)
    def add_chromatic_aberration(self, *, amount: int | float, angle_degrees: int | float, id: str | None = None) -> ChromaticAberrationEffect: return self._append(ChromaticAberrationEffect._create, id, amount, angle_degrees)
    def add_vignette(self, *, amount: int | float, radius: int | float, softness: int | float, colour: Color | str, id: str | None = None) -> VignetteEffect: return self._append(VignetteEffect._create, id, amount, radius, softness, colour)
    def add_sharpen(self, *, amount: int | float, radius: int | float, id: str | None = None) -> SharpenEffect: return self._append(SharpenEffect._create, id, amount, radius)
    def add_color_adjust(self, *, exposure: int | float, gamma: int | float, black_point: int | float, white_point: int | float, id: str | None = None) -> ColorAdjustEffect: return self._append(ColorAdjustEffect._create, id, exposure, gamma, black_point, white_point)

    def add_halftone(self, *,
        cell_size: int | float | ScalarTrack = 6,
        angle_degrees: int | float | ScalarTrack = 15,
        softness: int | float | ScalarTrack = 0.5,
        amount: int | float | ScalarTrack = 1,
        mode: HalftoneMode | str = HalftoneMode.LUMINANCE,
        foreground: Color | str = '#ffffff',
        background: Color | str = '#000000',
        invert: bool = False,
        id: str | None = None,
    ) -> HalftoneEffect:
        return self._append(HalftoneEffect._create_stylized, id, {"cell_size": cell_size, "angle_degrees": angle_degrees, "softness": softness, "amount": amount, "mode": mode, "foreground": foreground, "background": background, "invert": invert})

    def add_pixel_sort(self, *,
        lower_threshold: int | float | ScalarTrack = 0.15,
        upper_threshold: int | float | ScalarTrack = 0.9,
        amount: int | float | ScalarTrack = 1,
        direction: PixelSortDirection | str = PixelSortDirection.HORIZONTAL,
        order: PixelSortOrder | str = PixelSortOrder.ASCENDING,
        segment_length: int = 64,
        id: str | None = None,
    ) -> PixelSortEffect:
        return self._append(PixelSortEffect._create_stylized, id, {"lower_threshold": lower_threshold, "upper_threshold": upper_threshold, "amount": amount, "direction": direction, "order": order, "segment_length": segment_length})

    def add_crt(self, *,
        amount: int | float | ScalarTrack = 1,
        curvature: int | float | ScalarTrack = 0.08,
        scanline_strength: int | float | ScalarTrack = 0.2,
        scanline_spacing: int | float | ScalarTrack = 2,
        mask_strength: int | float | ScalarTrack = 0.15,
        grain: int | float | ScalarTrack = 0.025,
        jitter: int | float | ScalarTrack = 0.35,
        flicker: int | float | ScalarTrack = 0.025,
        rolling_strength: int | float | ScalarTrack = 0.06,
        rolling_width: int | float | ScalarTrack = 0.12,
        phase: int | float | ScalarTrack = 0,
        mask_spacing: int = 1,
        period: int | float | None = None,
        seed: int = 0,
        id: str | None = None,
    ) -> CrtEffect:
        return self._append(CrtEffect._create_stylized, id, {"amount": amount, "curvature": curvature, "scanline_strength": scanline_strength, "scanline_spacing": scanline_spacing, "mask_strength": mask_strength, "grain": grain, "jitter": jitter, "flicker": flicker, "rolling_strength": rolling_strength, "rolling_width": rolling_width, "phase": phase, "mask_spacing": mask_spacing, "period": period, "seed": seed})

    def add_ascii(self, *, characters: str = " .:-=+*#%@", edge_characters: str = "-|/\\",
        font: FontAsset | None = None, glyph_style: AsciiGlyphStyle | str = AsciiGlyphStyle.CHARACTERS,
        mode: AsciiMode | str = AsciiMode.HYBRID, color_mode: AsciiColorMode | str = AsciiColorMode.MONOCHROME,
        foreground: Color | str = "#ffffff", background: Color | str = "#000000",
        palette: Sequence[Color | str] = ("#000000", "#ffffff"), invert: bool = False,
        cell_width: int | float | ScalarTrack = 8, cell_height: int | float | ScalarTrack = 12,
        edge_threshold: int | float | ScalarTrack = .15, edge_strength: int | float | ScalarTrack = 1,
        source_mix: int | float | ScalarTrack = 0, amount: int | float | ScalarTrack = 1,
        phase: int | float | ScalarTrack = 0, period: int | float | None = None, id: str | None = None,
    ) -> AsciiEffect:
        return self._append(AsciiEffect._create_palette, id, {
            "characters": characters, "edge_characters": edge_characters, "font": font,
            "glyph_style": glyph_style, "mode": mode, "color_mode": color_mode,
            "foreground": foreground, "background": background, "palette": palette, "invert": invert,
            "cell_width": cell_width, "cell_height": cell_height, "edge_threshold": edge_threshold,
            "edge_strength": edge_strength, "source_mix": source_mix, "amount": amount,
            "phase": phase, "period": period,
        })

    def add_palette_map(
        self, *, palette: Sequence[Color | str] = ("#000000", "#ffffff"),
        amount: int | float | ScalarTrack = 1, phase: int | float | ScalarTrack = 0,
        period: int | float | None = None, mode: PaletteMode | str = PaletteMode.GRADIENT,
        levels: int = 4, stops: Sequence[int | float] | None = None,
        id: str | None = None,
    ) -> PaletteMapEffect:
        return self._append(PaletteMapEffect._create_palette, id, {
            "palette": palette, "amount": amount, "phase": phase, "period": period, "mode": mode, "levels": levels, "stops": stops,
        })

    def add_ordered_dither(
        self, *, palette: Sequence[Color | str] = ("#000000", "#ffffff"),
        amount: int | float | ScalarTrack = 1, phase: int | float | ScalarTrack = 0,
        period: int | float | None = None, mode: PaletteMode | str = PaletteMode.NEAREST,
        levels: int = 4, stops: Sequence[int | float] | None = None,
        strength: int | float | ScalarTrack = 1, matrix: DitherMatrix | str = DitherMatrix.BAYER8,
        scale: int = 1, seed: int = 0, id: str | None = None,
    ) -> OrderedDitherEffect:
        return self._append(OrderedDitherEffect._create_palette, id, {
            "palette": palette, "amount": amount, "phase": phase, "period": period, "mode": mode, "levels": levels, "stops": stops,
            "strength": strength, "matrix": matrix, "scale": scale, "seed": seed,
        })


class ClipEffectCollection(_EffectCollection):
    def add_camera_shake(self, *, active_interval: ActiveInterval = ActiveInterval(), position_amount: int | float, rotation_degrees: int | float, scale_amount: int | float, frequency: int | float, seed: int, attack: int | float, decay: int | float, id: str | None = None) -> CameraShakeEffect: return self._append(CameraShakeEffect._create, id, active_interval, position_amount, rotation_degrees, scale_amount, frequency, seed, attack, decay)
    def add_motion_blur(self, *, intensity: int | float, shutter_angle: int | float, max_radius: int | float, samples: int, id: str | None = None) -> MotionBlurEffect: return self._append(MotionBlurEffect._create, id, intensity, shutter_angle, max_radius, samples)


class PostEffectCollection(_EffectCollection):
    """Global effects. Transform-aware camera shake and motion blur are clip-only."""
