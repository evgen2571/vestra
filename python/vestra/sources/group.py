"""Small source-local groups usable as owned mask inputs."""

from __future__ import annotations

from collections.abc import Iterable
from copy import deepcopy
from typing import TYPE_CHECKING

from .base import Source
from .image import Image
from .video import Video

if TYPE_CHECKING:
    from ..masks import MaskCollection


def _identity_transform() -> dict[str, object]:
    return {
        "position": {"base_value": {"x": 0.5, "y": 0.5}},
        "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
        "scale": {"base_value": {"x": 1.0, "y": 1.0}},
        "rotation_degrees": {"base_value": 0.0},
    }


class GroupChild:
    """A source-local placement owned by :class:`Group`."""

    __slots__ = (
        "source", "start", "duration", "source_start", "playback_rate",
        "layer", "visible", "opacity",
        "_masks",
    )

    def __init__(
        self,
        source: Source,
        *,
        start: int | float = 0.0,
        duration: int | float = 1.0,
        source_start: int | float = 0.0,
        playback_rate: int | float = 1.0,
        layer: int = 0,
        visible: bool = True,
        opacity: int | float = 1.0,
    ) -> None:
        if not isinstance(source, Source):
            raise TypeError("Group child source must be a vestra source value")
        if isinstance(start, bool) or not isinstance(start, int | float) or start < 0:
            raise ValueError("Group child start must be non-negative")
        if isinstance(duration, bool) or not isinstance(duration, int | float) or duration <= 0:
            raise ValueError("Group child duration must be positive")
        if isinstance(source_start, bool) or not isinstance(source_start, int | float) or source_start < 0:
            raise ValueError("Group child source_start must be non-negative")
        if isinstance(playback_rate, bool) or not isinstance(playback_rate, int | float) or playback_rate <= 0:
            raise ValueError("Group child playback_rate must be positive")
        if isinstance(layer, bool) or not isinstance(layer, int):
            raise TypeError("Group child layer must be an integer")
        if not isinstance(visible, bool):
            raise TypeError("Group child visible must be a boolean")
        if isinstance(opacity, bool) or not isinstance(opacity, int | float) or not 0 <= opacity <= 1:
            raise ValueError("Group child opacity must be between 0 and 1")
        self.source = source.snapshot()
        self.start = float(start)
        self.duration = float(duration)
        self.source_start = float(source_start)
        self.playback_rate = float(playback_rate)
        self.layer = layer
        self.visible = visible
        self.opacity = float(opacity)
        self._masks = None

    @property
    def masks(self) -> "MaskCollection":
        if self._masks is None:
            from ..masks import MaskCollection

            self._masks = MaskCollection()
        return self._masks

    def snapshot(self) -> "GroupChild":
        return deepcopy(self)


class Group(Source):
    """A source-local composition of visual sources.

    Children are intentionally source values, rather than timeline layers. They
    render in declaration order at local time zero; the owning mask supplies
    the only placement and timing boundary.
    """

    __slots__ = ("_children",)

    def __init__(self, children: Iterable[Source] = ()) -> None:
        values = tuple(children)
        if not all(isinstance(child, Source | GroupChild) for child in values):
            raise TypeError("Group children must be source values or GroupChild objects")
        self._children = tuple(
            child.snapshot() if isinstance(child, GroupChild) else GroupChild(child)
            for child in values
        )

    @property
    def children(self) -> tuple[GroupChild, ...]:
        return self._children

    def add(
        self,
        source: Source,
        *,
        start: int | float = 0.0,
        duration: int | float = 1.0,
        source_start: int | float = 0.0,
        playback_rate: int | float = 1.0,
        layer: int | None = None,
        visible: bool = True,
        opacity: int | float = 1.0,
    ) -> GroupChild:
        child = GroupChild(
            source,
            start=start,
            duration=duration,
            source_start=source_start,
            playback_rate=playback_rate,
            layer=len(self._children) if layer is None else layer,
            visible=visible,
            opacity=opacity,
        )
        self._children = (*self._children, child)
        return child

    def to_canonical(self) -> dict[str, object]:
        clips = []
        for index, child in enumerate(self.children):
            source = child.source.to_canonical()
            if isinstance(source, str):
                source = {"type": "solid_color", "colour": source}
            masks = [mask.to_canonical() for mask in child.masks.items]
            clip: dict[str, object] = {
                "id": f"group-child-{index}",
                "start": child.start,
                "duration": child.duration,
                "layer": child.layer,
                "visible": child.visible,
                "opacity": {"base_value": child.opacity},
                "effects": [],
                "source": source,
                "masks": masks,
                "transform": _identity_transform(),
            }
            if isinstance(child.source, (Image, Video)):
                if child.source.sizing is not None:
                    clip["sizing"] = child.source.sizing.to_canonical()
                if child.source.crop.active:
                    clip["crop"] = child.source.crop.to_canonical()
            if isinstance(child.source, Video):
                clip["source_start"] = child.source_start
                if child.playback_rate != 1.0:
                    clip["playback_rate"] = child.playback_rate
            clips.append(clip)
        return {"type": "group", "clips": clips}


__all__ = ["Group", "GroupChild"]
