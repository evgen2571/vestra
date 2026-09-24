"""Typed canonical preset values owned by image clips."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Literal

from ._internal import _number
from .errors import AuthoringError

if TYPE_CHECKING:
    from .clips import ImageClip


def _start(value: int | float) -> float:
    result = _number(value, "start")
    if result < 0:
        raise ValueError("start must be non-negative")
    return result


def _duration(value: int | float | None) -> float | None:
    if value is None:
        return None
    result = _number(value, "duration")
    if result <= 0:
        raise ValueError("duration must be positive")
    return result


def _intensity(value: int | float) -> float:
    result = _number(value, "intensity")
    if not 0 <= result <= 2:
        raise ValueError("intensity must be between 0 and 2")
    return result


def _seed(value: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError("seed must be an integer")
    if not 0 <= value <= 2**64 - 1:
        raise ValueError("seed must be between 0 and 18446744073709551615")
    return value


@dataclass(frozen=True, slots=True)
class Preset:
    """An immutable schema-version 1 preset value."""

    kind: Literal["slow_drift", "zoom_punch", "impact", "heavy_impact", "focus_reveal"]
    intensity: float = 1.0
    start: float = 0.0
    duration: float | None = None
    seed: int | None = None

    def __post_init__(self) -> None:
        if self.kind not in {"slow_drift", "zoom_punch", "impact", "heavy_impact", "focus_reveal"}:
            raise ValueError("unknown preset kind")
        object.__setattr__(self, "intensity", _intensity(self.intensity))
        object.__setattr__(self, "start", _start(self.start))
        object.__setattr__(self, "duration", _duration(self.duration))
        seeded = self.kind in {"impact", "heavy_impact"}
        if seeded and self.seed is None:
            raise ValueError(f"{self.kind} requires seed")
        if not seeded and self.seed is not None:
            raise ValueError(f"{self.kind} does not accept seed")
        if self.seed is not None:
            object.__setattr__(self, "seed", _seed(self.seed))

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {"type": self.kind, "intensity": self.intensity}
        if self.start != 0:
            data["start"] = self.start
        if self.duration is not None:
            data["duration"] = self.duration
        if self.seed is not None:
            data["seed"] = self.seed
        return data


class PresetCollection:
    """Stable image-clip preset entry point. A clip has at most one preset."""

    __slots__ = ("_preset",)
    _preset: Preset | None

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("PresetCollection is owned by ImageClip")

    @classmethod
    def _create(cls, clip: ImageClip) -> PresetCollection:
        instance = object.__new__(cls)
        instance._preset = None
        return instance

    @property
    def current(self) -> Preset | None:
        return self._preset

    def clear(self) -> None:
        self._preset = None

    def _apply(self, preset: Preset) -> Preset:
        if self._preset is not None:
            raise AuthoringError("clip already has a preset; call clear() before applying another")
        # Preset validates all inputs before this single mutation.
        self._preset = preset
        return preset

    def apply_slow_drift(self, *, intensity: int | float = 1.0, start: int | float = 0.0,
                         duration: int | float | None = None) -> Preset:
        return self._apply(Preset("slow_drift", intensity, start, duration))

    def apply_zoom_punch(self, *, intensity: int | float = 1.0, start: int | float = 0.0,
                          duration: int | float | None = None) -> Preset:
        return self._apply(Preset("zoom_punch", intensity, start, duration))

    def apply_impact(self, *, seed: int, intensity: int | float = 1.0, start: int | float = 0.0,
                     duration: int | float | None = None) -> Preset:
        return self._apply(Preset("impact", intensity, start, duration, seed))

    def apply_heavy_impact(self, *, seed: int, intensity: int | float = 1.0, start: int | float = 0.0,
                           duration: int | float | None = None) -> Preset:
        return self._apply(Preset("heavy_impact", intensity, start, duration, seed))

    def apply_focus_reveal(self, *, intensity: int | float = 1.0, start: int | float = 0.0,
                           duration: int | float | None = None) -> Preset:
        return self._apply(Preset("focus_reveal", intensity, start, duration))
