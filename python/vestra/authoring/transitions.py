"""Generic transition definitions and builder-owned placements."""

from __future__ import annotations

from collections.abc import Mapping
from copy import deepcopy
from typing import TYPE_CHECKING, Self

from ._internal import _IdAllocator, _Owner, _number, _require_owner
from .clips import GroupClip, ImageClip
from .errors import AuthoringError

if TYPE_CHECKING:
    from .builder import ProjectBuilder

Clip = ImageClip | GroupClip


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


class TransitionDefinition:
    """Reusable generic channel data, independent of clips and timing."""

    __slots__ = ("_data",)

    def __init__(self, data: Mapping[str, object]) -> None:
        if not isinstance(data, Mapping):
            raise TypeError("definition must be a mapping")
        if not isinstance(data.get("outgoing"), Mapping) or not isinstance(data.get("incoming"), Mapping):
            raise ValueError("definition requires outgoing and incoming channel mappings")
        self._data = deepcopy({"outgoing": dict(data["outgoing"]), "incoming": dict(data["incoming"])})

    def to_canonical(self) -> dict[str, object]:
        return deepcopy(self._data)


class TransitionPlacement:
    """Builder-owned endpoint/timing relationship."""

    __slots__ = ("_owner", "_id", "_outgoing", "_incoming", "_start", "_duration", "_definition")

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("TransitionPlacement objects must be created by builder.transitions")

    @classmethod
    def _create(cls, owner: _Owner, identifier: str, outgoing: Clip, incoming: Clip, start: int | float, duration: int | float, definition: TransitionDefinition) -> Self:
        instance = object.__new__(cls)
        instance._owner = owner
        instance._id = identifier
        instance._outgoing = outgoing
        instance._incoming = incoming
        instance._start = _start(start)
        instance._duration = _duration(duration)
        instance._definition = definition
        return instance

    @property
    def id(self) -> str: return self._id
    @property
    def outgoing(self) -> Clip: return self._outgoing
    @property
    def incoming(self) -> Clip: return self._incoming
    @property
    def start(self) -> float: return self._start
    @start.setter
    def start(self, value: int | float) -> None: self._start = _start(value)
    @property
    def duration(self) -> float: return self._duration
    @duration.setter
    def duration(self, value: int | float) -> None: self._duration = _duration(value)
    @property
    def definition(self) -> TransitionDefinition: return self._definition

    def to_canonical(self) -> dict[str, object]:
        return {"id": self.id, "outgoing": self.outgoing.id, "incoming": self.incoming.id,
                "start": self.start, "duration": self.duration, "definition": self.definition.to_canonical()}

    def __repr__(self) -> str: return f"TransitionPlacement(id={self.id!r})"


class TransitionCollection:
    __slots__ = ("_owner", "_ids", "_builder", "_items")

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("TransitionCollection is owned by ProjectBuilder")

    @classmethod
    def _create(cls, owner: _Owner, ids: _IdAllocator, builder: ProjectBuilder) -> Self:
        instance = object.__new__(cls)
        instance._owner = owner
        instance._ids = ids
        instance._builder = builder
        instance._items = []
        return instance

    @property
    def items(self) -> tuple[TransitionPlacement, ...]: return tuple(self._items)

    def add_transition(self, *, outgoing: Clip, incoming: Clip, start: int | float, duration: int | float,
                       definition: TransitionDefinition | Mapping[str, object], id: str | None = None) -> TransitionPlacement:
        if not isinstance(outgoing, ImageClip | GroupClip) or not isinstance(incoming, ImageClip | GroupClip):
            raise TypeError("outgoing and incoming must be ImageClip or GroupClip")
        _require_owner(self._owner, outgoing._owner)
        _require_owner(self._owner, incoming._owner)
        if outgoing not in self._builder.clips or incoming not in self._builder.clips:
            raise AuthoringError("transitions require visible root clips; nested Group children are not endpoints")
        if outgoing is incoming:
            raise AuthoringError("transition requires two different clips")
        if id is not None:
            self._ids.validate("transition", id)
        # Construct and validate every value before reserving an ID or mutating the collection.
        resolved = definition if isinstance(definition, TransitionDefinition) else TransitionDefinition(definition)
        placement = TransitionPlacement._create(self._owner, "", outgoing, incoming, start, duration, resolved)
        placement._id = self._ids.allocate("transition") if id is None else self._ids.reserve("transition", id)
        self._items.append(placement)
        return placement


__all__ = ["TransitionDefinition", "TransitionPlacement", "TransitionCollection"]
