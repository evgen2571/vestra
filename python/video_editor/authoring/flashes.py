"""Typed project-level flash overlays."""
from __future__ import annotations
from typing import TYPE_CHECKING, Self
from ._internal import _IdAllocator, _Owner, _number
from .values import Color, color_to_canonical
if TYPE_CHECKING: from .builder import ProjectBuilder

def _timing(value: int | float, name: str, positive: bool = False) -> float:
    value = _number(value, name)
    if value <= 0 if positive else value < 0: raise ValueError(f"{name} must be {'positive' if positive else 'non-negative'}")
    return value
def _opacity(value: int | float) -> float:
    value = _number(value, "opacity")
    if not 0 <= value <= 1: raise ValueError("opacity must be between 0 and 1")
    return value
def _layer(value: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int): raise TypeError("layer must be an integer")
    return value

class Flash:
    __slots__ = ("_owner", "_scope", "_id", "_start", "_duration", "_colour", "_opacity", "_fade_in", "_fade_out", "_layer")
    _owner: _Owner
    _scope: object
    _id: str
    _start: float
    _duration: float
    _colour: str
    _opacity: float
    _fade_in: float
    _fade_out: float
    _layer: int
    def __init__(self, *args: object, **kwargs: object) -> None: raise TypeError("Flash objects must be created by builder.flashes")
    @classmethod
    def _create(cls, owner: _Owner, scope: object, identifier: str, start: int | float, duration: int | float, colour: Color | str, opacity: int | float, fade_in: int | float, fade_out: int | float, layer: int) -> Self:
        instance = object.__new__(cls); instance._owner = owner; instance._scope = scope; instance._id = identifier; instance._start = _timing(start, "start"); instance._duration = _timing(duration, "duration", True); instance._colour = color_to_canonical(colour); instance._opacity = _opacity(opacity); instance._fade_in = _timing(fade_in, "fade_in"); instance._fade_out = _timing(fade_out, "fade_out"); instance._layer = _layer(layer)
        if instance._fade_in + instance._fade_out > instance._duration: raise ValueError("flash fades must fit within duration")
        return instance
    @property
    def id(self) -> str: return self._id
    @property
    def start(self) -> float: return self._start
    @start.setter
    def start(self, value: int | float) -> None: self._start = _timing(value, "start")
    @property
    def duration(self) -> float: return self._duration
    @duration.setter
    def duration(self, value: int | float) -> None:
        duration = _timing(value, "duration", True)
        if self.fade_in + self.fade_out > duration: raise ValueError("flash fades must fit within duration")
        self._duration = duration
    @property
    def colour(self) -> str: return self._colour
    @colour.setter
    def colour(self, value: Color | str) -> None: self._colour = color_to_canonical(value)
    @property
    def opacity(self) -> float: return self._opacity
    @opacity.setter
    def opacity(self, value: int | float) -> None: self._opacity = _opacity(value)
    @property
    def fade_in(self) -> float: return self._fade_in
    @fade_in.setter
    def fade_in(self, value: int | float) -> None:
        fade_in = _timing(value, "fade_in")
        if fade_in + self.fade_out > self.duration: raise ValueError("flash fades must fit within duration")
        self._fade_in = fade_in
    @property
    def fade_out(self) -> float: return self._fade_out
    @fade_out.setter
    def fade_out(self, value: int | float) -> None:
        fade_out = _timing(value, "fade_out")
        if self.fade_in + fade_out > self.duration: raise ValueError("flash fades must fit within duration")
        self._fade_out = fade_out
    @property
    def layer(self) -> int: return self._layer
    @layer.setter
    def layer(self, value: int) -> None: self._layer = _layer(value)
    def to_canonical(self) -> dict[str, object]: return {"id": self.id, "start": self.start, "duration": self.duration, "colour": self.colour, "opacity": self.opacity, "fade_in": self.fade_in, "fade_out": self.fade_out, "layer": self.layer}
    def __repr__(self) -> str: return f"Flash(id={self.id!r})"

class FlashCollection:
    __slots__ = ("_owner", "_ids", "_builder", "_items")
    _owner: _Owner
    _ids: _IdAllocator
    _builder: ProjectBuilder
    _items: list[Flash]
    def __init__(self, *args: object, **kwargs: object) -> None: raise TypeError("FlashCollection is owned by ProjectBuilder")
    @classmethod
    def _create(cls, owner: _Owner, ids: _IdAllocator, builder: ProjectBuilder) -> Self:
        instance = object.__new__(cls); instance._owner = owner; instance._ids = ids; instance._builder = builder; instance._items = []; return instance
    @property
    def items(self) -> tuple[Flash, ...]: return tuple(self._items)
    def add(self, *, start: int | float, duration: int | float, colour: Color | str, opacity: int | float = 1.0, fade_in: int | float = 0.0, fade_out: int | float = 0.0, layer: int = 0, id: str | None = None) -> Flash:
        if id is not None: self._ids.validate("flash", id)
        flash = Flash._create(self._owner, self, "", start, duration, colour, opacity, fade_in, fade_out, layer)
        flash._id = self._ids.allocate("flash") if id is None else self._ids.reserve("flash", id)
        self._items.append(flash); return flash
