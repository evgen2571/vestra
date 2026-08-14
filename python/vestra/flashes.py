"""Root-owned flash overlays for the high-level editor."""

from __future__ import annotations

from typing import cast, overload

from .authoring.values import Color, color_to_canonical
from .sources import Color as SourceColor


_UNSET = object()


def _number(value: int | float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    result = float(value)
    if result != result or result in (float("inf"), float("-inf")):
        raise ValueError(f"{name} must be finite")
    return result


def _timing(value: int | float, name: str, *, positive: bool = False) -> float:
    result = _number(value, name)
    if result <= 0 if positive else result < 0:
        raise ValueError(f"{name} must be {'positive' if positive else 'non-negative'}")
    return result


class Flash:
    """Safely mutable flash overlay descriptor."""

    __slots__ = (
        "_start",
        "_duration",
        "_colour",
        "_opacity",
        "_fade_in",
        "_fade_out",
        "_layer",
        "_id",
    )

    def __init__(
        self,
        start: int | float,
        duration: int | float,
        colour: str | Color | SourceColor,
        opacity: int | float = 1.0,
        fade_in: int | float = 0.0,
        fade_out: int | float = 0.0,
        layer: int = 0,
        id: str | None = None,
    ) -> None:
        self._start = _timing(start, "start")
        self._duration = _timing(duration, "duration", positive=True)
        value = colour.value if isinstance(colour, SourceColor) else colour
        self._colour = color_to_canonical(value)
        self._opacity = self._validate_opacity(opacity)
        self._fade_in = _timing(fade_in, "fade_in")
        self._fade_out = _timing(fade_out, "fade_out")
        self._layer = self._validate_layer(layer)
        self._id = self._validate_id(id)
        self._validate_fades(self._fade_in, self._fade_out, self._duration)

    @staticmethod
    def _validate_opacity(value: int | float) -> float:
        result = _number(value, "opacity")
        if not 0 <= result <= 1:
            raise ValueError("opacity must be between 0 and 1")
        return result

    @staticmethod
    def _validate_layer(value: int) -> int:
        if isinstance(value, bool) or not isinstance(value, int):
            raise TypeError("layer must be an integer")
        return value

    @staticmethod
    def _validate_id(value: str | None) -> str | None:
        if value is not None and (
            not isinstance(value, str) or not value or value.isspace()
        ):
            raise ValueError("id must be a non-empty string or None")
        return value

    @staticmethod
    def _validate_fades(fade_in: float, fade_out: float, duration: float) -> None:
        if fade_in + fade_out > duration:
            raise ValueError("flash fades must fit within duration")

    def _copy(self, *, id: str | None = None) -> "Flash":
        return Flash(
            self.start,
            self.duration,
            self.colour,
            self.opacity,
            self.fade_in,
            self.fade_out,
            self.layer,
            self.id if id is None else id,
        )

    @property
    def start(self) -> float:
        return self._start

    @start.setter
    def start(self, value: int | float) -> None:
        self._start = _timing(value, "start")

    @property
    def duration(self) -> float:
        return self._duration

    @duration.setter
    def duration(self, value: int | float) -> None:
        duration = _timing(value, "duration", positive=True)
        self._validate_fades(self.fade_in, self.fade_out, duration)
        self._duration = duration

    @property
    def colour(self) -> str:
        return self._colour

    @colour.setter
    def colour(self, value: str | Color | SourceColor) -> None:
        raw = value.value if isinstance(value, SourceColor) else value
        self._colour = color_to_canonical(raw)

    @property
    def opacity(self) -> float:
        return self._opacity

    @opacity.setter
    def opacity(self, value: int | float) -> None:
        self._opacity = self._validate_opacity(value)

    @property
    def fade_in(self) -> float:
        return self._fade_in

    @fade_in.setter
    def fade_in(self, value: int | float) -> None:
        fade = _timing(value, "fade_in")
        self._validate_fades(fade, self.fade_out, self.duration)
        self._fade_in = fade

    @property
    def fade_out(self) -> float:
        return self._fade_out

    @fade_out.setter
    def fade_out(self, value: int | float) -> None:
        fade = _timing(value, "fade_out")
        self._validate_fades(self.fade_in, fade, self.duration)
        self._fade_out = fade

    @property
    def layer(self) -> int:
        return self._layer

    @layer.setter
    def layer(self, value: int) -> None:
        self._layer = self._validate_layer(value)

    @property
    def id(self) -> str | None:
        return self._id

    def to_canonical(self) -> dict[str, object]:
        return {
            "id": self.id,
            "start": self.start,
            "duration": self.duration,
            "colour": self.colour,
            "opacity": self.opacity,
            "fade_in": self.fade_in,
            "fade_out": self.fade_out,
            "layer": self.layer,
        }


class FlashCollection:
    """Stable ordered flashes. Only a Project owns this collection."""

    __slots__ = ("_project", "_items", "_next_id")

    def __init__(self, project: object) -> None:
        self._project = project
        self._items: list[Flash] = []
        self._next_id = 1

    @property
    def items(self) -> tuple[Flash, ...]:
        return tuple(self._items)

    @overload
    def add(self, flash: Flash, *, id: str | None = None) -> Flash: ...

    @overload
    def add(
        self,
        *,
        start: int | float,
        duration: int | float,
        colour: Color | SourceColor | str,
        opacity: int | float = 1.0,
        fade_in: int | float = 0.0,
        fade_out: int | float = 0.0,
        layer: int = 0,
        id: str | None = None,
    ) -> Flash: ...

    def add(
        self,
        flash: Flash | None = None,
        *,
        start: int | float | object = _UNSET,
        duration: int | float | object = _UNSET,
        colour: Color | SourceColor | str | object = _UNSET,
        opacity: int | float | object = _UNSET,
        fade_in: int | float | object = _UNSET,
        fade_out: int | float | object = _UNSET,
        layer: int | object = _UNSET,
        id: str | None = None,
    ) -> Flash:
        if flash is not None:
            if any(
                value is not _UNSET
                for value in (
                    start,
                    duration,
                    colour,
                    opacity,
                    fade_in,
                    fade_out,
                    layer,
                )
            ):
                raise TypeError("pass either a Flash or flash fields, not both")
            if not isinstance(flash, Flash):
                raise TypeError("flash must be a Flash descriptor")
            candidate = flash._copy(id=flash.id if id is None else id)
        else:
            if start is _UNSET or duration is _UNSET or colour is _UNSET:
                raise TypeError("start, duration, and colour are required")
            candidate = Flash(
                cast(int | float, start),
                cast(int | float, duration),
                cast(Color | SourceColor | str, colour),
                cast(int | float, 1.0 if opacity is _UNSET else opacity),
                cast(int | float, 0.0 if fade_in is _UNSET else fade_in),
                cast(int | float, 0.0 if fade_out is _UNSET else fade_out),
                cast(int, 0 if layer is _UNSET else layer),
                id,
            )
        identifier = candidate.id
        if identifier is None:
            while any(item.id == f"flash-{self._next_id:06d}" for item in self._items):
                self._next_id += 1
            identifier = f"flash-{self._next_id:06d}"
            self._next_id += 1
            candidate = candidate._copy(id=identifier)
        if any(item.id == identifier for item in self._items):
            raise ValueError(f"duplicate flash ID: {identifier!r}")
        self._items.append(candidate)
        return candidate


__all__ = ["Flash", "FlashCollection"]
