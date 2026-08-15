"""Reusable transition definitions and composition-owned placements."""

from __future__ import annotations

from dataclasses import dataclass
from copy import deepcopy
import math
from typing import TYPE_CHECKING, Generic, TypeVar

from .authoring.animation import interpolation_to_canonical
from .authoring.values import CubicBezier, Interpolation, Point
from .effects import DirectionalBlur, Effect, GaussianBlur, ZoomBlur

if TYPE_CHECKING:
    from .editor import Composition, Layer

InterpolationValue = Interpolation | CubicBezier
AnimationValue = TypeVar("AnimationValue", covariant=True)


def _number(value: int | float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    result = float(value)
    if not math.isfinite(result):
        raise ValueError(f"{name} must be finite")
    return result


def _nonnegative(value: int | float, name: str) -> float:
    result = _number(value, name)
    if result < 0:
        raise ValueError(f"{name} must be non-negative")
    return result


def _positive(value: int | float, name: str) -> float:
    result = _number(value, name)
    if result <= 0:
        raise ValueError(f"{name} must be positive")
    return result


def _interpolation(value: InterpolationValue) -> InterpolationValue:
    if not isinstance(value, Interpolation | CubicBezier):
        raise TypeError("easing must be Interpolation or CubicBezier")
    return value


@dataclass(frozen=True, slots=True)
class _NormalizedKeyframe(Generic[AnimationValue]):
    progress: float
    value: AnimationValue
    interpolation: InterpolationValue | None


@dataclass(frozen=True, slots=True, init=False)
class Animate(Generic[AnimationValue]):
    """An immutable animation track expressed in normalized transition progress."""

    _keyframes: tuple[_NormalizedKeyframe[AnimationValue], ...]
    _easing: InterpolationValue | None

    def __init__(
        self,
        start_value: AnimationValue,
        end_value: AnimationValue,
        *,
        easing: InterpolationValue | None = None,
    ) -> None:
        if easing is not None:
            easing = _interpolation(easing)
        object.__setattr__(
            self,
            "_keyframes",
            (
                _NormalizedKeyframe(0.0, start_value, easing),
                _NormalizedKeyframe(1.0, end_value, easing),
            ),
        )
        object.__setattr__(self, "_easing", easing)

    @classmethod
    def keyframes(
        cls,
        *keyframes: tuple[float, AnimationValue]
        | tuple[float, AnimationValue, InterpolationValue],
        easing: InterpolationValue | None = None,
    ) -> "Animate[AnimationValue]":
        """Create a track from normalized keyframes.

        Interpolation on a keyframe controls the segment ending at that
        keyframe, matching Vestra's native keyframe convention.
        """
        if easing is not None:
            easing = _interpolation(easing)
        if len(keyframes) < 2:
            raise ValueError("Animate requires at least 2 keyframes")
        normalized: list[_NormalizedKeyframe[AnimationValue]] = []
        previous = -1.0
        for keyframe in keyframes:
            if not isinstance(keyframe, tuple) or len(keyframe) not in (2, 3):
                raise TypeError("keyframes must be (progress, value[, easing]) tuples")
            progress = _number(keyframe[0], "progress")
            if not 0.0 <= progress <= 1.0:
                raise ValueError("progress must be between 0 and 1")
            if progress <= previous:
                raise ValueError("keyframe progress must be strictly increasing")
            interpolation = None if len(keyframe) == 2 else _interpolation(keyframe[2])
            normalized.append(_NormalizedKeyframe(progress, keyframe[1], interpolation))
            previous = progress
        if normalized[0].progress != 0.0:
            raise ValueError("first keyframe progress must be 0")
        if normalized[-1].progress != 1.0:
            raise ValueError("last keyframe progress must be 1")
        instance = object.__new__(cls)
        object.__setattr__(instance, "_keyframes", tuple(normalized))
        object.__setattr__(instance, "_easing", easing)
        return instance

    @property
    def keyframe_values(self) -> tuple[_NormalizedKeyframe[AnimationValue], ...]:
        return self._keyframes


@dataclass(frozen=True, slots=True)
class TransitionLayer:
    """Normalized presentation channels for one transition endpoint."""

    opacity: Animate[float] | None = None
    position: Animate[object] | None = None
    scale: Animate[object] | None = None
    rotation: Animate[float] | None = None
    effects: tuple[Effect, ...] = ()

    def __post_init__(self) -> None:
        for name in ("opacity", "position", "scale", "rotation"):
            value = getattr(self, name)
            if value is not None and not isinstance(value, Animate):
                raise TypeError(f"{name} must be an Animate instance or None")
        if not isinstance(self.effects, (list, tuple)):
            raise TypeError("effects must be a list or tuple of Effect objects")
        effects = tuple(self.effects)
        if any(not isinstance(effect, Effect) for effect in effects):
            raise TypeError("effects must contain only Effect objects")
        object.__setattr__(self, "effects", effects)


def _track(
    values: tuple[tuple[float, object, InterpolationValue], ...],
) -> dict[str, object]:
    return {
        "keyframes": [
            {
                "progress": progress,
                "value": value,
                "interpolation": interpolation_to_canonical(easing),
            }
            for progress, value, easing in values
        ]
    }


def _definition(
    outgoing: dict[str, object], incoming: dict[str, object]
) -> dict[str, object]:
    return {"outgoing": outgoing, "incoming": incoming}


def _opacity(easing: InterpolationValue) -> tuple[dict[str, object], dict[str, object]]:
    return (
        {"opacity": _track(((0.0, 1.0, easing), (1.0, 0.0, easing)))},
        {"opacity": _track(((0.0, 0.0, easing), (1.0, 1.0, easing)))},
    )


def _scale(value: float) -> dict[str, float]:
    return {"x": value, "y": value}


@dataclass(frozen=True, slots=True)
class TransitionDefinition:
    """Immutable, reusable behavior with no layer, timing, or placement ID."""

    _canonical: dict[str, object]

    def to_canonical(self) -> dict[str, object]:
        return deepcopy(self._canonical)


def _canonical_channel_value(channel: str, value: object) -> object:
    if channel == "opacity":
        if isinstance(value, bool) or not isinstance(value, int | float):
            raise TypeError("opacity must be a real number")
        number = _number(value, "opacity")
        if not 0.0 <= number <= 1.0:
            raise ValueError("opacity must be between 0 and 1")
        return number
    if channel == "rotation_offset_degrees":
        if isinstance(value, bool) or not isinstance(value, int | float):
            raise TypeError("rotation must be a real number")
        return _number(value, "rotation")
    if channel == "position_offset":
        if isinstance(value, Point):
            return value.to_canonical()
        if isinstance(value, tuple) and len(value) == 2:
            return Point(value[0], value[1]).to_canonical()
        raise TypeError("position values must be Point or (x, y) tuples")
    if channel == "scale_multiplier":
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            scale = _number(value, "scale")
            if scale <= 0:
                raise ValueError("scale must be positive")
            return {"x": scale, "y": scale}
        if isinstance(value, Point):
            x, y = value.x, value.y
        elif isinstance(value, tuple) and len(value) == 2:
            x, y = value
        else:
            raise TypeError(
                "scale values must be a positive scalar, Point, or (x, y) tuple"
            )
        x_number = _number(x, "scale.x")
        y_number = _number(y, "scale.y")
        if x_number <= 0 or y_number <= 0:
            raise ValueError("scale components must be positive")
        return {"x": x_number, "y": y_number}
    raise ValueError(f"unsupported transition channel: {channel}")


def _canonical_layer(
    layer: TransitionLayer | None,
    default_easing: InterpolationValue,
) -> dict[str, object]:
    if layer is None:
        return {}
    channels: tuple[tuple[str, Animate[object] | None], ...] = (
        ("opacity", layer.opacity),
        ("position_offset", layer.position),
        ("scale_multiplier", layer.scale),
        ("rotation_offset_degrees", layer.rotation),
    )
    output: dict[str, object] = {}
    for channel, animation in channels:
        if animation is None:
            continue
        keyframes = []
        for keyframe in animation.keyframe_values:
            easing = keyframe.interpolation or animation._easing or default_easing
            keyframes.append(
                {
                    "progress": keyframe.progress,
                    "value": _canonical_channel_value(channel, keyframe.value),
                    "interpolation": interpolation_to_canonical(easing),
                }
            )
        output[channel] = {"keyframes": keyframes}
    if layer.effects:
        canonical_effects: list[dict[str, object]] = []
        for index, effect in enumerate(layer.effects):
            canonical = effect.to_canonical()
            effect_id = effect.id or f"transition-effect-{index:06d}"
            canonical["id"] = effect_id
            for name, value in canonical.items():
                if not isinstance(value, dict) or "keyframes" not in value:
                    continue
                keyframes = value["keyframes"]
                if not isinstance(keyframes, list):
                    continue
                for keyframe in keyframes:
                    if not isinstance(keyframe, dict):
                        continue
                    time = keyframe.get("time")
                    if not isinstance(time, int | float) or not math.isfinite(float(time)):
                        raise ValueError(f"transition effect {name} keyframe time must be finite")
                    if not 0.0 <= float(time) <= 1.0:
                        raise ValueError(f"transition effect {name} keyframe time must be between 0 and 1")
            canonical_effects.append(canonical)
        output["effects"] = canonical_effects
    return output


@dataclass(frozen=True, slots=True)
class CustomTransition(TransitionDefinition):
    """Reusable normalized presentation channels with no placement state."""

    outgoing: TransitionLayer | None = None
    incoming: TransitionLayer | None = None
    default_easing: InterpolationValue = Interpolation.LINEAR

    def __init__(
        self,
        *,
        outgoing: TransitionLayer | None = None,
        incoming: TransitionLayer | None = None,
        default_easing: InterpolationValue = Interpolation.LINEAR,
    ) -> None:
        default_easing = _interpolation(default_easing)
        if outgoing is not None and not isinstance(outgoing, TransitionLayer):
            raise TypeError("outgoing must be a TransitionLayer or None")
        if incoming is not None and not isinstance(incoming, TransitionLayer):
            raise TypeError("incoming must be a TransitionLayer or None")
        canonical_outgoing = _canonical_layer(outgoing, default_easing)
        canonical_incoming = _canonical_layer(incoming, default_easing)
        if not canonical_outgoing and not canonical_incoming:
            raise ValueError("custom transition requires at least one channel or effect")
        object.__setattr__(self, "outgoing", outgoing)
        object.__setattr__(self, "incoming", incoming)
        object.__setattr__(self, "default_easing", default_easing)
        object.__setattr__(
            self, "_canonical", _definition(canonical_outgoing, canonical_incoming)
        )


@dataclass(frozen=True, slots=True)
class Crossfade(TransitionDefinition):
    """Generic opacity crossfade."""

    easing: InterpolationValue = Interpolation.EASE_IN_OUT

    def __init__(
        self, *, easing: InterpolationValue = Interpolation.EASE_IN_OUT
    ) -> None:
        easing = _interpolation(easing)
        object.__setattr__(self, "easing", easing)
        object.__setattr__(self, "_canonical", _definition(*_opacity(easing)))


@dataclass(frozen=True, slots=True)
class DirectionalPush(TransitionDefinition):
    """Generic position-offset push; positive angles follow native coordinates."""

    angle_degrees: float = 0.0
    distance: float = 1.0
    easing: InterpolationValue = Interpolation.EASE_IN_OUT

    def __init__(
        self,
        *,
        angle_degrees: int | float = 0.0,
        distance: int | float = 1.0,
        easing: InterpolationValue = Interpolation.EASE_IN_OUT,
    ) -> None:
        angle = _number(angle_degrees, "angle_degrees")
        span = _nonnegative(distance, "distance")
        easing = _interpolation(easing)
        delta = {
            "x": math.cos(math.radians(angle)) * span,
            "y": math.sin(math.radians(angle)) * span,
        }
        object.__setattr__(self, "angle_degrees", angle)
        object.__setattr__(self, "distance", span)
        object.__setattr__(self, "easing", easing)
        object.__setattr__(
            self,
            "_canonical",
            _definition(
                {
                    "position_offset": _track(
                        ((0.0, {"x": 0.0, "y": 0.0}, easing), (1.0, delta, easing))
                    )
                },
                {
                    "position_offset": _track(
                        (
                            (0.0, {"x": -delta["x"], "y": -delta["y"]}, easing),
                            (1.0, {"x": 0.0, "y": 0.0}, easing),
                        )
                    )
                },
            ),
        )


class PushLeft(DirectionalPush):
    def __init__(
        self,
        *,
        distance: int | float = 1.0,
        easing: InterpolationValue = Interpolation.EASE_IN_OUT,
    ) -> None:
        super().__init__(angle_degrees=180.0, distance=distance, easing=easing)


class PushRight(DirectionalPush):
    def __init__(
        self,
        *,
        distance: int | float = 1.0,
        easing: InterpolationValue = Interpolation.EASE_IN_OUT,
    ) -> None:
        super().__init__(angle_degrees=0.0, distance=distance, easing=easing)


class PushUp(DirectionalPush):
    def __init__(
        self,
        *,
        distance: int | float = 1.0,
        easing: InterpolationValue = Interpolation.EASE_IN_OUT,
    ) -> None:
        super().__init__(angle_degrees=-90.0, distance=distance, easing=easing)


class PushDown(DirectionalPush):
    def __init__(
        self,
        *,
        distance: int | float = 1.0,
        easing: InterpolationValue = Interpolation.EASE_IN_OUT,
    ) -> None:
        super().__init__(angle_degrees=90.0, distance=distance, easing=easing)


@dataclass(frozen=True, slots=True)
class ZoomCrossfade(TransitionDefinition):
    outgoing_zoom: float = 1.1
    incoming_start_zoom: float = 0.9
    easing: InterpolationValue = Interpolation.EASE_IN_OUT

    def __init__(
        self,
        *,
        outgoing_zoom: int | float = 1.1,
        incoming_start_zoom: int | float = 0.9,
        easing: InterpolationValue = Interpolation.EASE_IN_OUT,
    ) -> None:
        outgoing = _positive(outgoing_zoom, "outgoing_zoom")
        incoming = _positive(incoming_start_zoom, "incoming_start_zoom")
        easing = _interpolation(easing)
        outgoing_opacity, incoming_opacity = _opacity(easing)
        object.__setattr__(self, "outgoing_zoom", outgoing)
        object.__setattr__(self, "incoming_start_zoom", incoming)
        object.__setattr__(self, "easing", easing)
        object.__setattr__(
            self,
            "_canonical",
            _definition(
                {
                    **outgoing_opacity,
                    "scale_multiplier": _track(
                        ((0.0, _scale(1.0), easing), (1.0, _scale(outgoing), easing))
                    ),
                },
                {
                    **incoming_opacity,
                    "scale_multiplier": _track(
                        ((0.0, _scale(incoming), easing), (1.0, _scale(1.0), easing))
                    ),
                },
            ),
        )


class ZoomIn(ZoomCrossfade):
    def __init__(
        self,
        *,
        amount: int | float = 0.9,
        easing: InterpolationValue = Interpolation.EASE_IN_OUT,
    ) -> None:
        super().__init__(outgoing_zoom=1.0, incoming_start_zoom=amount, easing=easing)


class ZoomOut(ZoomCrossfade):
    def __init__(
        self,
        *,
        amount: int | float = 1.1,
        easing: InterpolationValue = Interpolation.EASE_IN_OUT,
    ) -> None:
        super().__init__(outgoing_zoom=amount, incoming_start_zoom=1.0, easing=easing)


def _effect_with_radius(effect: Effect, values: tuple[tuple[float, float], ...]) -> Effect:
    effect._set_id("transition-effect")
    radius = effect.radius
    radius.clear_keyframes()
    for progress, value in values:
        radius.keyframe(progress, value)
    return effect


class BlurCrossfade(TransitionDefinition):
    """Opacity crossfade with a temporary symmetric Gaussian blur."""

    def __init__(self, *, radius: int | float = 12.0, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        peak = _positive(radius, "radius")
        easing = _interpolation(easing)
        outgoing_blur = _effect_with_radius(GaussianBlur(0.0), ((0.0, 0.0), (0.5, peak), (1.0, 0.0)))
        incoming_blur = _effect_with_radius(GaussianBlur(0.0), ((0.0, peak), (1.0, 0.0)))
        object.__setattr__(self, "_canonical", _definition(
            {**_opacity(easing)[0], "effects": [outgoing_blur.to_canonical()]},
            {**_opacity(easing)[1], "effects": [incoming_blur.to_canonical()]},
        ))


class ZoomBlurTransition(TransitionDefinition):
    """Zooming crossfade using the ordinary ZoomBlur effect."""

    def __init__(self, *, radius: int | float = 0.8, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        peak = _nonnegative(radius, "radius")
        easing = _interpolation(easing)
        outgoing = _effect_with_radius(ZoomBlur(0.0, 16, (0.5, 0.5)), ((0.0, 0.0), (0.5, peak), (1.0, 0.0)))
        incoming = _effect_with_radius(ZoomBlur(0.0, 16, (0.5, 0.5)), ((0.0, peak), (1.0, 0.0)))
        out_opacity, in_opacity = _opacity(easing)
        object.__setattr__(self, "_canonical", _definition(
            {**out_opacity, "scale_multiplier": _track(((0.0, _scale(1.0), easing), (1.0, _scale(1.08), easing))), "effects": [outgoing.to_canonical()]},
            {**in_opacity, "scale_multiplier": _track(((0.0, _scale(0.92), easing), (1.0, _scale(1.0), easing))), "effects": [incoming.to_canonical()]},
        ))


class _WhipPan(TransitionDefinition):
    def __init__(self, *, angle_degrees: float, distance: int | float, radius: int | float, easing: InterpolationValue) -> None:
        delta = {"x": math.cos(math.radians(angle_degrees)) * distance, "y": math.sin(math.radians(angle_degrees)) * distance}
        outgoing_blur = _effect_with_radius(DirectionalBlur(0.0, angle_degrees), ((0.0, 0.0), (0.5, radius), (1.0, 0.0)))
        incoming_blur = _effect_with_radius(DirectionalBlur(0.0, angle_degrees), ((0.0, radius), (1.0, 0.0)))
        out_opacity, in_opacity = _opacity(easing)
        object.__setattr__(self, "_canonical", _definition(
            {**out_opacity, "position_offset": _track(((0.0, {"x": 0.0, "y": 0.0}, easing), (1.0, delta, easing))), "effects": [outgoing_blur.to_canonical()]},
            {**in_opacity, "position_offset": _track(((0.0, {"x": -delta["x"], "y": -delta["y"]}, easing), (1.0, {"x": 0.0, "y": 0.0}, easing))), "effects": [incoming_blur.to_canonical()]},
        ))


class WhipPanLeft(_WhipPan):
    def __init__(self, *, distance: int | float = 1.0, radius: int | float = 12.0, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        super().__init__(angle_degrees=180.0, distance=_nonnegative(distance, "distance"), radius=_nonnegative(radius, "radius"), easing=_interpolation(easing))


class WhipPanRight(_WhipPan):
    def __init__(self, *, distance: int | float = 1.0, radius: int | float = 12.0, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        super().__init__(angle_degrees=0.0, distance=_nonnegative(distance, "distance"), radius=_nonnegative(radius, "radius"), easing=_interpolation(easing))


@dataclass(frozen=True, slots=True)
class TransitionPlacement:
    id: str
    outgoing: Layer
    incoming: Layer
    start: float
    duration: float
    definition: TransitionDefinition

    def to_canonical(self) -> dict[str, object]:
        return {
            "id": self.id,
            "outgoing": self.outgoing.id,
            "incoming": self.incoming.id,
            "start": self.start,
            "duration": self.duration,
            "definition": self.definition.to_canonical(),
        }


class TransitionCollection:
    """Stable ordered placements owned by one composition."""

    __slots__ = ("_composition", "_items", "_next_id")

    def __init__(self, composition: Composition) -> None:
        self._composition = composition
        self._items: list[TransitionPlacement] = []
        self._next_id = 1

    @property
    def items(self) -> tuple[TransitionPlacement, ...]:
        return tuple(self._items)

    def _valid_endpoint(self, layer: Layer) -> bool:
        from .editor import CompositionLayer
        from .lowering import source_capabilities

        if isinstance(layer, CompositionLayer):
            return True
        capabilities = source_capabilities(layer.source)
        return (
            capabilities.supports_direct_transition_endpoint
            or capabilities.supports_transition_adapter
        )

    def add(
        self,
        outgoing: Layer,
        incoming: Layer,
        definition: TransitionDefinition,
        *,
        start: int | float,
        duration: int | float,
        id: str | None = None,
    ) -> TransitionPlacement:
        from .editor import Layer

        if self._composition.parent_layer is not None:
            raise ValueError("transitions are supported only on the root composition")
        if not isinstance(outgoing, Layer) or not isinstance(incoming, Layer):
            raise TypeError("transition endpoints must be Layer objects")
        if not isinstance(definition, TransitionDefinition):
            raise TypeError("definition must be a TransitionDefinition")
        if outgoing is incoming:
            raise ValueError("transition requires two different layers")
        if (
            outgoing.composition is not self._composition
            or incoming.composition is not self._composition
        ):
            raise ValueError("transition endpoints must belong to the same composition")
        if (
            outgoing not in self._composition.layers
            or incoming not in self._composition.layers
        ):
            raise ValueError("transition endpoints must be owned by this composition")
        if not self._valid_endpoint(outgoing) or not self._valid_endpoint(incoming):
            raise TypeError("one or both layers do not support transition endpoints")
        placement_start = _nonnegative(start, "start")
        placement_duration = _positive(duration, "duration")
        if id is not None and (not isinstance(id, str) or not id or id.isspace()):
            raise ValueError("id must be a non-empty string")
        occupied = {item.id for item in self._items}
        next_id = self._next_id
        if id is None:
            while (identifier := f"transition-{next_id:06d}") in occupied:
                next_id += 1
        else:
            identifier = id
            if identifier in occupied:
                raise ValueError(f"duplicate transition ID: {identifier!r}")
        placement = TransitionPlacement(
            identifier,
            outgoing,
            incoming,
            placement_start,
            placement_duration,
            definition,
        )
        self._items.append(placement)
        if id is None:
            self._next_id = next_id + 1
        return placement

    def remove(self, placement: TransitionPlacement) -> None:
        try:
            self._items.remove(placement)
        except ValueError as error:
            raise ValueError("transition is not owned by this collection") from error


__all__ = [
    "InterpolationValue",
    "Animate",
    "TransitionLayer",
    "CustomTransition",
    "TransitionDefinition",
    "TransitionPlacement",
    "TransitionCollection",
    "Crossfade",
    "DirectionalPush",
    "PushLeft",
    "PushRight",
    "PushUp",
    "PushDown",
    "ZoomCrossfade",
    "ZoomIn",
    "ZoomOut",
    "BlurCrossfade",
    "ZoomBlurTransition",
    "WhipPanLeft",
    "WhipPanRight",
]
