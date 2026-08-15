"""Reusable transition definitions and composition-owned placements."""

from __future__ import annotations

from dataclasses import dataclass
from copy import deepcopy
import math
from typing import TYPE_CHECKING

from .authoring.animation import interpolation_to_canonical
from .authoring.values import CubicBezier, Interpolation
if TYPE_CHECKING:
    from .editor import Composition, Layer

InterpolationValue = Interpolation | CubicBezier


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


@dataclass(frozen=True, slots=True)
class Crossfade(TransitionDefinition):
    """Generic opacity crossfade."""

    easing: InterpolationValue = Interpolation.EASE_IN_OUT

    def __init__(self, *, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
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
        delta = {"x": math.cos(math.radians(angle)) * span, "y": math.sin(math.radians(angle)) * span}
        object.__setattr__(self, "angle_degrees", angle)
        object.__setattr__(self, "distance", span)
        object.__setattr__(self, "easing", easing)
        object.__setattr__(self, "_canonical", _definition(
            {"position_offset": _track(((0.0, {"x": 0.0, "y": 0.0}, easing), (1.0, delta, easing)))},
            {"position_offset": _track(((0.0, {"x": -delta["x"], "y": -delta["y"]}, easing), (1.0, {"x": 0.0, "y": 0.0}, easing)))},
        ))


class PushLeft(DirectionalPush):
    def __init__(self, *, distance: int | float = 1.0, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        super().__init__(angle_degrees=180.0, distance=distance, easing=easing)


class PushRight(DirectionalPush):
    def __init__(self, *, distance: int | float = 1.0, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        super().__init__(angle_degrees=0.0, distance=distance, easing=easing)


class PushUp(DirectionalPush):
    def __init__(self, *, distance: int | float = 1.0, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        super().__init__(angle_degrees=-90.0, distance=distance, easing=easing)


class PushDown(DirectionalPush):
    def __init__(self, *, distance: int | float = 1.0, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        super().__init__(angle_degrees=90.0, distance=distance, easing=easing)


@dataclass(frozen=True, slots=True)
class ZoomCrossfade(TransitionDefinition):
    outgoing_zoom: float = 1.1
    incoming_start_zoom: float = 0.9
    easing: InterpolationValue = Interpolation.EASE_IN_OUT

    def __init__(self, *, outgoing_zoom: int | float = 1.1, incoming_start_zoom: int | float = 0.9, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        outgoing = _positive(outgoing_zoom, "outgoing_zoom")
        incoming = _positive(incoming_start_zoom, "incoming_start_zoom")
        easing = _interpolation(easing)
        outgoing_opacity, incoming_opacity = _opacity(easing)
        object.__setattr__(self, "outgoing_zoom", outgoing)
        object.__setattr__(self, "incoming_start_zoom", incoming)
        object.__setattr__(self, "easing", easing)
        object.__setattr__(self, "_canonical", _definition(
            {**outgoing_opacity, "scale_multiplier": _track(((0.0, _scale(1.0), easing), (1.0, _scale(outgoing), easing)))},
            {**incoming_opacity, "scale_multiplier": _track(((0.0, _scale(incoming), easing), (1.0, _scale(1.0), easing)))},
        ))


class ZoomIn(ZoomCrossfade):
    def __init__(self, *, amount: int | float = 0.9, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        super().__init__(outgoing_zoom=1.0, incoming_start_zoom=amount, easing=easing)


class ZoomOut(ZoomCrossfade):
    def __init__(self, *, amount: int | float = 1.1, easing: InterpolationValue = Interpolation.EASE_IN_OUT) -> None:
        super().__init__(outgoing_zoom=amount, incoming_start_zoom=1.0, easing=easing)


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
        return capabilities.supports_direct_transition_endpoint or capabilities.supports_transition_adapter

    def add(self, outgoing: Layer, incoming: Layer, definition: TransitionDefinition, *, start: int | float, duration: int | float, id: str | None = None) -> TransitionPlacement:
        from .editor import Layer
        if self._composition.parent_layer is not None:
            raise ValueError("transitions are supported only on the root composition")
        if not isinstance(outgoing, Layer) or not isinstance(incoming, Layer):
            raise TypeError("transition endpoints must be Layer objects")
        if not isinstance(definition, TransitionDefinition):
            raise TypeError("definition must be a TransitionDefinition")
        if outgoing is incoming:
            raise ValueError("transition requires two different layers")
        if outgoing.composition is not self._composition or incoming.composition is not self._composition:
            raise ValueError("transition endpoints must belong to the same composition")
        if outgoing not in self._composition.layers or incoming not in self._composition.layers:
            raise ValueError("transition endpoints must be owned by this composition")
        if not self._valid_endpoint(outgoing) or not self._valid_endpoint(incoming):
            raise TypeError("one or both layers do not support transition endpoints")
        placement_start = _nonnegative(start, "start")
        placement_duration = _positive(duration, "duration")
        identifier = id if id is not None else f"transition-{self._next_id:06d}"
        while identifier in {item.id for item in self._items}:
            self._next_id += 1
            identifier = f"transition-{self._next_id:06d}"
        if not isinstance(identifier, str) or not identifier or identifier.isspace():
            raise ValueError("id must be a non-empty string")
        if any(item.id == identifier for item in self._items):
            raise ValueError(f"duplicate transition ID: {identifier!r}")
        placement = TransitionPlacement(identifier, outgoing, incoming, placement_start, placement_duration, definition)
        self._items.append(placement)
        self._next_id += 1
        return placement

    def remove(self, placement: TransitionPlacement) -> None:
        try:
            self._items.remove(placement)
        except ValueError as error:
            raise ValueError("transition is not owned by this collection") from error


__all__ = [
    "InterpolationValue", "TransitionDefinition", "TransitionPlacement", "TransitionCollection",
    "Crossfade", "DirectionalPush", "PushLeft", "PushRight", "PushUp", "PushDown",
    "ZoomCrossfade", "ZoomIn", "ZoomOut",
]
