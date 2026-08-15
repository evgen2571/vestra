"""Narrow, explicit project-timeline authoring helpers."""

from __future__ import annotations

from typing import TYPE_CHECKING

from ._internal import _Owner, _number, _require_owner
from .clips import GroupClip, ImageClip, ParticleSystemClip, SolidColorClip, Spectrum2DClip

if TYPE_CHECKING:
    from .builder import ProjectBuilder

Clip = ImageClip | SolidColorClip | ParticleSystemClip | Spectrum2DClip | GroupClip


class Timeline:
    """Builder-owned helpers that do not serialize or move related events."""

    __slots__ = ("_builder", "_owner")
    _builder: ProjectBuilder
    _owner: _Owner

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("Timeline is owned by ProjectBuilder")

    @classmethod
    def _create(cls, builder: ProjectBuilder) -> Timeline:
        instance = object.__new__(cls)
        instance._builder = builder
        instance._owner = builder._owner
        return instance

    @staticmethod
    def _delta(value: int | float) -> float:
        return _number(value, "delta")

    def _check_clip(self, clip: Clip) -> None:
        if not isinstance(clip, ImageClip | SolidColorClip | ParticleSystemClip | Spectrum2DClip | GroupClip):
            raise TypeError("clip must be a supported visual clip")
        _require_owner(self._owner, clip._owner)

    def shift_clip(self, clip: Clip, *, delta: int | float) -> None:
        self.shift_clips((clip,), delta=delta)

    def shift_clips(self, clips: list[Clip] | tuple[Clip, ...], *, delta: int | float) -> None:
        if not isinstance(clips, list | tuple):
            raise TypeError("clips must be a list or tuple of clips")
        change = self._delta(delta)
        unique: list[Clip] = []
        seen: set[int] = set()
        for clip in clips:
            self._check_clip(clip)
            if id(clip) not in seen:
                unique.append(clip)
                seen.add(id(clip))
        starts = [clip.start + change for clip in unique]
        if any(start < 0 for start in starts):
            raise ValueError("clip shift would make start negative")
        for clip, start in zip(unique, starts, strict=True):
            clip.start = start
