"""Builder-independent transition values and root composition ownership."""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import TYPE_CHECKING, ClassVar

from .authoring.animation import interpolation_to_canonical
from .authoring.values import Color, CubicBezier, Interpolation, color_to_canonical
from .sources import Color as SourceColor

if TYPE_CHECKING:
    from .editor import Composition, Layer

InterpolationValue = Interpolation | CubicBezier


def _number(value: int | float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    result = float(value)
    if result != result or result in (float("inf"), float("-inf")):
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


def _unit(value: int | float, name: str) -> float:
    result = _number(value, name)
    if not 0 <= result <= 1:
        raise ValueError(f"{name} must be between 0 and 1")
    return result


def _interpolation(value: InterpolationValue) -> InterpolationValue:
    if not isinstance(value, Interpolation | CubicBezier):
        raise TypeError("interpolation must be Interpolation or CubicBezier")
    return value


def _blur(value: int | float) -> float:
    result = _nonnegative(value, "blur_radius")
    if result > 32:
        raise ValueError("blur_radius must be between 0 and 32")
    return result


@dataclass(frozen=True, slots=True)
class Transition:
    """Immutable transition intent, independent of any native builder."""

    start: float
    duration: float
    interpolation: InterpolationValue = Interpolation.LINEAR
    id: str | None = None

    kind: ClassVar[str] = "transition"

    def __post_init__(self) -> None:
        object.__setattr__(self, "start", _nonnegative(self.start, "start"))
        object.__setattr__(self, "duration", _positive(self.duration, "duration"))
        object.__setattr__(self, "interpolation", _interpolation(self.interpolation))
        if self.id is not None and (
            not isinstance(self.id, str) or not self.id or self.id.isspace()
        ):
            raise ValueError("id must be a non-empty string or None")

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {
            "type": self.kind,
            "start": self.start,
            "duration": self.duration,
            "interpolation": interpolation_to_canonical(self.interpolation),
        }
        if self.id is not None:
            data["id"] = self.id
        return data


@dataclass(frozen=True, slots=True)
class Crossfade(Transition):
    kind: ClassVar[str] = "crossfade"


@dataclass(frozen=True, slots=True)
class ZoomCrossfade(Transition):
    outgoing_zoom: float = 1.0
    incoming_start_zoom: float = 1.0
    kind: ClassVar[str] = "zoom_crossfade"

    def __post_init__(self) -> None:
        Transition.__post_init__(self)
        object.__setattr__(
            self, "outgoing_zoom", _positive(self.outgoing_zoom, "outgoing_zoom")
        )
        object.__setattr__(
            self,
            "incoming_start_zoom",
            _positive(self.incoming_start_zoom, "incoming_start_zoom"),
        )

    def to_canonical(self) -> dict[str, object]:
        return {
            **super().to_canonical(),
            "outgoing_zoom": self.outgoing_zoom,
            "incoming_start_zoom": self.incoming_start_zoom,
        }


@dataclass(frozen=True, slots=True)
class FlashCut(Transition):
    colour: str | Color | SourceColor = "#ffffff"
    intensity: float = 1.0
    kind: ClassVar[str] = "flash_cut"

    def __post_init__(self) -> None:
        Transition.__post_init__(self)
        colour = (
            self.colour.value if isinstance(self.colour, SourceColor) else self.colour
        )
        object.__setattr__(self, "colour", color_to_canonical(colour))
        object.__setattr__(self, "intensity", _unit(self.intensity, "intensity"))

    def to_canonical(self) -> dict[str, object]:
        return {
            **super().to_canonical(),
            "colour": self.colour,
            "intensity": self.intensity,
        }


@dataclass(frozen=True, slots=True)
class DirectionalPush(Transition):
    angle_degrees: float = 0.0
    distance: float = 1.0
    blur_radius: float = 0.0
    kind: ClassVar[str] = "directional_push"

    def __post_init__(self) -> None:
        Transition.__post_init__(self)
        object.__setattr__(
            self, "angle_degrees", _number(self.angle_degrees, "angle_degrees")
        )
        object.__setattr__(self, "distance", _nonnegative(self.distance, "distance"))
        object.__setattr__(self, "blur_radius", _blur(self.blur_radius))

    def to_canonical(self) -> dict[str, object]:
        return {
            **super().to_canonical(),
            "angle_degrees": self.angle_degrees,
            "distance": self.distance,
            "blur_radius": self.blur_radius,
        }


@dataclass(frozen=True, slots=True)
class ZoomBlur(Transition):
    outgoing_zoom: float = 1.0
    incoming_start_zoom: float = 1.0
    blur_radius: float = 0.0
    kind: ClassVar[str] = "zoom_blur"

    def __post_init__(self) -> None:
        Transition.__post_init__(self)
        object.__setattr__(
            self, "outgoing_zoom", _positive(self.outgoing_zoom, "outgoing_zoom")
        )
        object.__setattr__(
            self,
            "incoming_start_zoom",
            _positive(self.incoming_start_zoom, "incoming_start_zoom"),
        )
        object.__setattr__(self, "blur_radius", _blur(self.blur_radius))

    def to_canonical(self) -> dict[str, object]:
        return {
            **super().to_canonical(),
            "outgoing_zoom": self.outgoing_zoom,
            "incoming_start_zoom": self.incoming_start_zoom,
            "blur_radius": self.blur_radius,
        }


class TransitionCollection:
    """Stable ordered transitions owned by one composition."""

    __slots__ = ("_composition", "_items", "_next_id")

    def __init__(self, composition: Composition) -> None:
        self._composition = composition
        self._items: list[tuple[Layer, Layer, Transition]] = []
        self._next_id = 1

    @property
    def items(self) -> tuple[Transition, ...]:
        return tuple(item[2] for item in self._items)

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
        transition: Transition,
        *,
        id: str | None = None,
    ) -> Transition:
        if self._composition.parent_layer is not None:
            raise ValueError("transitions are supported only on the root composition")
        from .editor import Layer

        if not isinstance(outgoing, Layer) or not isinstance(incoming, Layer):
            raise TypeError("transition endpoints must be Layer objects")
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
        if type(transition) not in {
            Crossfade,
            ZoomCrossfade,
            FlashCut,
            DirectionalPush,
            ZoomBlur,
        }:
            raise TypeError("transition must be a supported transition descriptor")
        if not self._valid_endpoint(outgoing) or not self._valid_endpoint(incoming):
            raise TypeError("one or both layers do not support transition endpoints")
        identifier = transition.id if id is None else id
        if identifier is None:
            while any(
                item[2].id == f"transition-{self._next_id:06d}" for item in self._items
            ):
                self._next_id += 1
            identifier = f"transition-{self._next_id:06d}"
            self._next_id += 1
        if not isinstance(identifier, str) or not identifier or identifier.isspace():
            raise ValueError("id must be a non-empty string")
        if any(item[2].id == identifier for item in self._items):
            raise ValueError(f"duplicate transition ID: {identifier!r}")
        stored = replace(transition, id=identifier)
        self._items.append((outgoing, incoming, stored))
        return stored

    def remove(self, transition: Transition) -> None:
        for index, item in enumerate(self._items):
            if item[2] is transition:
                del self._items[index]
                return
        raise ValueError("transition is not owned by this collection")


__all__ = [
    "InterpolationValue",
    "Transition",
    "Crossfade",
    "ZoomCrossfade",
    "FlashCut",
    "DirectionalPush",
    "ZoomBlur",
    "TransitionCollection",
]
