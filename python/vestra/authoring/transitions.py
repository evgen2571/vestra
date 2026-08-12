"""Typed project-level coordinated transitions."""

from __future__ import annotations

from collections.abc import Callable
from typing import TYPE_CHECKING, Self

from ._internal import _IdAllocator, _Owner, _number, _require_owner
from .animation import InterpolationValue, interpolation_to_canonical
from .clips import ImageClip
from .errors import AuthoringError
from .values import Color, CubicBezier, Interpolation, color_to_canonical

if TYPE_CHECKING:
    from .builder import ProjectBuilder


def _start(value: int | float) -> float:
    result = _number(value, "start")
    if result < 0:
        raise ValueError("start must be non-negative")
    return result


def _duration(value: int | float) -> float:
    result = _number(value, "duration")
    if result <= 0:
        raise ValueError("duration must be positive")
    return result


def _interpolation(value: InterpolationValue) -> InterpolationValue:
    if not isinstance(value, Interpolation | CubicBezier):
        raise TypeError("interpolation must be Interpolation or CubicBezier")
    return value


class Transition:
    """Base class for transitions created by :class:`TransitionCollection`."""

    __slots__ = ("_owner", "_scope", "_id", "_kind", "_outgoing", "_incoming", "_start", "_duration", "_interpolation")

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError(f"{type(self).__name__} objects must be created by builder.transitions")

    def _initialize(self, owner: _Owner, scope: object, identifier: str, kind: str,
                    outgoing: ImageClip, incoming: ImageClip, start: int | float,
                    duration: int | float, interpolation: InterpolationValue) -> None:
        self._owner = owner
        self._scope = scope
        self._id = identifier
        self._kind = kind
        self._outgoing = outgoing
        self._incoming = incoming
        self._start = _start(start)
        self._duration = _duration(duration)
        self._interpolation = _interpolation(interpolation)

    @property
    def id(self) -> str: return self._id
    @property
    def kind(self) -> str: return self._kind
    @property
    def outgoing(self) -> ImageClip: return self._outgoing
    @property
    def incoming(self) -> ImageClip: return self._incoming
    @property
    def start(self) -> float: return self._start
    @start.setter
    def start(self, value: int | float) -> None: self._start = _start(value)
    @property
    def duration(self) -> float: return self._duration
    @duration.setter
    def duration(self, value: int | float) -> None: self._duration = _duration(value)
    @property
    def interpolation(self) -> InterpolationValue: return self._interpolation
    @interpolation.setter
    def interpolation(self, value: InterpolationValue) -> None: self._interpolation = _interpolation(value)

    def _canonical(self) -> dict[str, object]:
        return {"id": self.id, "type": self.kind, "outgoing": self.outgoing.id,
                "incoming": self.incoming.id, "start": self.start, "duration": self.duration,
                "interpolation": interpolation_to_canonical(self.interpolation)}

    def to_canonical(self) -> dict[str, object]:
        return self._canonical()

    def __repr__(self) -> str: return f"{type(self).__name__}(id={self.id!r})"


class CrossfadeTransition(Transition):
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, outgoing: ImageClip,
                incoming: ImageClip, start: int | float, duration: int | float,
                interpolation: InterpolationValue) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "crossfade", outgoing, incoming, start, duration, interpolation)
        return instance


class ZoomCrossfadeTransition(Transition):
    __slots__ = ("_outgoing_zoom", "_incoming_start_zoom")
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, outgoing: ImageClip, incoming: ImageClip,
                start: int | float, duration: int | float, interpolation: InterpolationValue,
                outgoing_zoom: int | float, incoming_start_zoom: int | float) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "zoom_crossfade", outgoing, incoming, start, duration, interpolation)
        instance._outgoing_zoom = _positive(outgoing_zoom, "outgoing_zoom")
        instance._incoming_start_zoom = _positive(incoming_start_zoom, "incoming_start_zoom")
        return instance
    @property
    def outgoing_zoom(self) -> float: return self._outgoing_zoom
    @outgoing_zoom.setter
    def outgoing_zoom(self, value: int | float) -> None: self._outgoing_zoom = _positive(value, "outgoing_zoom")
    @property
    def incoming_start_zoom(self) -> float: return self._incoming_start_zoom
    @incoming_start_zoom.setter
    def incoming_start_zoom(self, value: int | float) -> None: self._incoming_start_zoom = _positive(value, "incoming_start_zoom")
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "outgoing_zoom": self.outgoing_zoom, "incoming_start_zoom": self.incoming_start_zoom}


class FlashCutTransition(Transition):
    __slots__ = ("_colour", "_intensity")
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, outgoing: ImageClip, incoming: ImageClip,
                start: int | float, duration: int | float, interpolation: InterpolationValue,
                colour: Color | str, intensity: int | float) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "flash_cut", outgoing, incoming, start, duration, interpolation)
        instance._colour = color_to_canonical(colour)
        instance._intensity = _unit(intensity, "intensity")
        return instance
    @property
    def colour(self) -> str: return self._colour
    @colour.setter
    def colour(self, value: Color | str) -> None: self._colour = color_to_canonical(value)
    @property
    def intensity(self) -> float: return self._intensity
    @intensity.setter
    def intensity(self, value: int | float) -> None: self._intensity = _unit(value, "intensity")
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "colour": self.colour, "intensity": self.intensity}


class DirectionalPushTransition(Transition):
    __slots__ = ("_angle_degrees", "_distance", "_blur_radius")
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, outgoing: ImageClip, incoming: ImageClip,
                start: int | float, duration: int | float, interpolation: InterpolationValue,
                angle_degrees: int | float, distance: int | float, blur_radius: int | float) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "directional_push", outgoing, incoming, start, duration, interpolation)
        instance._angle_degrees = _number(angle_degrees, "angle_degrees")
        instance._distance = _nonnegative(distance, "distance")
        instance._blur_radius = _blur(blur_radius)
        return instance
    @property
    def angle_degrees(self) -> float: return self._angle_degrees
    @angle_degrees.setter
    def angle_degrees(self, value: int | float) -> None: self._angle_degrees = _number(value, "angle_degrees")
    @property
    def distance(self) -> float: return self._distance
    @distance.setter
    def distance(self, value: int | float) -> None: self._distance = _nonnegative(value, "distance")
    @property
    def blur_radius(self) -> float: return self._blur_radius
    @blur_radius.setter
    def blur_radius(self, value: int | float) -> None: self._blur_radius = _blur(value)
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "angle_degrees": self.angle_degrees, "distance": self.distance, "blur_radius": self.blur_radius}


class ZoomBlurTransition(Transition):
    __slots__ = ("_outgoing_zoom", "_incoming_start_zoom", "_blur_radius")
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, outgoing: ImageClip, incoming: ImageClip,
                start: int | float, duration: int | float, interpolation: InterpolationValue,
                outgoing_zoom: int | float, incoming_start_zoom: int | float, blur_radius: int | float) -> Self:
        instance = object.__new__(cls)
        instance._initialize(owner, scope, identifier, "zoom_blur", outgoing, incoming, start, duration, interpolation)
        instance._outgoing_zoom = _positive(outgoing_zoom, "outgoing_zoom")
        instance._incoming_start_zoom = _positive(incoming_start_zoom, "incoming_start_zoom")
        instance._blur_radius = _blur(blur_radius)
        return instance
    @property
    def outgoing_zoom(self) -> float: return self._outgoing_zoom
    @outgoing_zoom.setter
    def outgoing_zoom(self, value: int | float) -> None: self._outgoing_zoom = _positive(value, "outgoing_zoom")
    @property
    def incoming_start_zoom(self) -> float: return self._incoming_start_zoom
    @incoming_start_zoom.setter
    def incoming_start_zoom(self, value: int | float) -> None: self._incoming_start_zoom = _positive(value, "incoming_start_zoom")
    @property
    def blur_radius(self) -> float: return self._blur_radius
    @blur_radius.setter
    def blur_radius(self, value: int | float) -> None: self._blur_radius = _blur(value)
    def to_canonical(self) -> dict[str, object]: return {**self._canonical(), "outgoing_zoom": self.outgoing_zoom, "incoming_start_zoom": self.incoming_start_zoom, "blur_radius": self.blur_radius}


def _positive(value: int | float, name: str) -> float:
    value = _number(value, name)
    if value <= 0: raise ValueError(f"{name} must be positive")
    return value
def _nonnegative(value: int | float, name: str) -> float:
    value = _number(value, name)
    if value < 0: raise ValueError(f"{name} must be non-negative")
    return value
def _unit(value: int | float, name: str) -> float:
    value = _number(value, name)
    if not 0 <= value <= 1: raise ValueError(f"{name} must be between 0 and 1")
    return value
def _blur(value: int | float) -> float:
    value = _nonnegative(value, "blur_radius")
    if value > 32: raise ValueError("blur_radius must be between 0 and 32")
    return value


class TransitionCollection:
    __slots__ = ("_owner", "_ids", "_builder", "_items")
    _owner: _Owner
    _ids: _IdAllocator
    _builder: ProjectBuilder
    _items: list[Transition]
    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("TransitionCollection is owned by ProjectBuilder")
    @classmethod
    def _create(cls, owner: _Owner, ids: _IdAllocator, builder: ProjectBuilder) -> Self:
        instance = object.__new__(cls); instance._owner = owner; instance._ids = ids; instance._builder = builder; instance._items = []; return instance
    @property
    def items(self) -> tuple[Transition, ...]: return tuple(self._items)
    def _add(self, factory: Callable[..., Transition], identifier: str | None, outgoing: ImageClip, incoming: ImageClip, *args: object) -> Transition:
        if not isinstance(outgoing, ImageClip) or not isinstance(incoming, ImageClip): raise TypeError("outgoing and incoming must be ImageClip")
        _require_owner(self._owner, outgoing._owner); _require_owner(self._owner, incoming._owner)
        if outgoing is incoming: raise AuthoringError("transition requires two different clips")
        if identifier is not None: self._ids.validate("transition", identifier)
        transition = factory(self._owner, self, "", outgoing, incoming, *args)
        transition._id = self._ids.allocate("transition") if identifier is None else self._ids.reserve("transition", identifier)
        self._items.append(transition); return transition
    def add_crossfade(self, *, outgoing: ImageClip, incoming: ImageClip, start: int | float, duration: int | float, interpolation: InterpolationValue = Interpolation.LINEAR, id: str | None = None) -> CrossfadeTransition: return self._add(CrossfadeTransition._create, id, outgoing, incoming, start, duration, interpolation)  # type: ignore[return-value]
    def add_zoom_crossfade(self, *, outgoing: ImageClip, incoming: ImageClip, start: int | float, duration: int | float, outgoing_zoom: int | float, incoming_start_zoom: int | float, interpolation: InterpolationValue = Interpolation.LINEAR, id: str | None = None) -> ZoomCrossfadeTransition: return self._add(ZoomCrossfadeTransition._create, id, outgoing, incoming, start, duration, interpolation, outgoing_zoom, incoming_start_zoom)  # type: ignore[return-value]
    def add_flash_cut(self, *, outgoing: ImageClip, incoming: ImageClip, start: int | float, duration: int | float, colour: Color | str, intensity: int | float, interpolation: InterpolationValue = Interpolation.LINEAR, id: str | None = None) -> FlashCutTransition: return self._add(FlashCutTransition._create, id, outgoing, incoming, start, duration, interpolation, colour, intensity)  # type: ignore[return-value]
    def add_directional_push(self, *, outgoing: ImageClip, incoming: ImageClip, start: int | float, duration: int | float, angle_degrees: int | float, distance: int | float, blur_radius: int | float, interpolation: InterpolationValue = Interpolation.LINEAR, id: str | None = None) -> DirectionalPushTransition: return self._add(DirectionalPushTransition._create, id, outgoing, incoming, start, duration, interpolation, angle_degrees, distance, blur_radius)  # type: ignore[return-value]
    def add_zoom_blur(self, *, outgoing: ImageClip, incoming: ImageClip, start: int | float, duration: int | float, outgoing_zoom: int | float, incoming_start_zoom: int | float, blur_radius: int | float, interpolation: InterpolationValue = Interpolation.LINEAR, id: str | None = None) -> ZoomBlurTransition: return self._add(ZoomBlurTransition._create, id, outgoing, incoming, start, duration, interpolation, outgoing_zoom, incoming_start_zoom, blur_radius)  # type: ignore[return-value]
