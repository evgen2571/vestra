"""Layer-owned cinematic preset values."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from .editor import Layer

PresetKind = Literal[
    "slow_drift", "zoom_punch", "impact", "heavy_impact", "focus_reveal"
]


def _real(value: int | float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    result = float(value)
    if result != result or result in (float("inf"), float("-inf")):
        raise ValueError(f"{name} must be finite")
    return result


@dataclass(frozen=True, slots=True)
class Preset:
    """Immutable cinematic preset intent, lowered through native ImageClip presets."""

    kind: PresetKind
    intensity: float = 1.0
    start: float = 0.0
    duration: float | None = None
    seed: int | None = None

    def __post_init__(self) -> None:
        if self.kind not in {
            "slow_drift",
            "zoom_punch",
            "impact",
            "heavy_impact",
            "focus_reveal",
        }:
            raise ValueError("unknown preset kind")
        intensity = _real(self.intensity, "intensity")
        if not 0 <= intensity <= 2:
            raise ValueError("intensity must be between 0 and 2")
        start = _real(self.start, "start")
        if start < 0:
            raise ValueError("start must be non-negative")
        duration = None if self.duration is None else _real(self.duration, "duration")
        if duration is not None and duration <= 0:
            raise ValueError("duration must be positive")
        seeded = self.kind in {"impact", "heavy_impact"}
        if seeded and self.seed is None:
            raise ValueError(f"{self.kind} requires seed")
        if not seeded and self.seed is not None:
            raise ValueError(f"{self.kind} does not accept seed")
        if self.seed is not None:
            if isinstance(self.seed, bool) or not isinstance(self.seed, int):
                raise TypeError("seed must be an integer")
            if not 0 <= self.seed <= 2**64 - 1:
                raise ValueError("seed must be between 0 and 18446744073709551615")
        object.__setattr__(self, "intensity", intensity)
        object.__setattr__(self, "start", start)
        object.__setattr__(self, "duration", duration)

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
    """At most one cinematic preset for a Layer."""

    __slots__ = ("_layer", "_current")

    def __init__(self, layer: Layer) -> None:
        self._layer = layer
        self._current: Preset | None = None

    @property
    def current(self) -> Preset | None:
        return self._current

    def clear(self) -> None:
        self._current = None

    def add(self, preset: Preset) -> Preset:
        from .lowering import source_capabilities

        if not source_capabilities(self._layer.source).supports_cinematic_preset:
            raise TypeError("cinematic presets are supported only on Image layers")
        if not isinstance(preset, Preset):
            raise TypeError("preset must be a Preset")
        if self._current is not None:
            raise ValueError(
                "layer already has a preset; call clear() before applying another"
            )
        self._current = preset
        return preset

    def apply_slow_drift(
        self,
        *,
        intensity: int | float = 1.0,
        start: int | float = 0.0,
        duration: int | float | None = None,
    ) -> Preset:
        return self.add(Preset("slow_drift", intensity, start, duration))

    def apply_zoom_punch(
        self,
        *,
        intensity: int | float = 1.0,
        start: int | float = 0.0,
        duration: int | float | None = None,
    ) -> Preset:
        return self.add(Preset("zoom_punch", intensity, start, duration))

    def apply_impact(
        self,
        *,
        seed: int,
        intensity: int | float = 1.0,
        start: int | float = 0.0,
        duration: int | float | None = None,
    ) -> Preset:
        return self.add(Preset("impact", intensity, start, duration, seed))

    def apply_heavy_impact(
        self,
        *,
        seed: int,
        intensity: int | float = 1.0,
        start: int | float = 0.0,
        duration: int | float | None = None,
    ) -> Preset:
        return self.add(Preset("heavy_impact", intensity, start, duration, seed))

    def apply_focus_reveal(
        self,
        *,
        intensity: int | float = 1.0,
        start: int | float = 0.0,
        duration: int | float | None = None,
    ) -> Preset:
        return self.add(Preset("focus_reveal", intensity, start, duration))


__all__ = ["PresetKind", "Preset", "PresetCollection"]
