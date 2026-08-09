"""Immutable scalar audio-analysis signals for canonical project authoring."""

from __future__ import annotations

from dataclasses import dataclass

from ._internal import _number


def _finite(value: int | float, name: str) -> float:
    return _number(value, name)


@dataclass(frozen=True, slots=True)
class ScalarSignal:
    """An immutable Master-audio scalar signal with ordered transforms.

    Band frequencies are expressed in Hz; envelope attack and release are
    elapsed seconds. Each transform method returns a new reusable signal.
    """

    _feature: dict[str, object]
    _transforms: tuple[dict[str, object], ...] = ()

    def _append(self, transform: dict[str, object]) -> "ScalarSignal":
        return ScalarSignal(self._feature.copy(), self._transforms + (transform,))

    def gain(self, value: int | float) -> "ScalarSignal":
        return self._append({"type": "gain", "gain": _finite(value, "gain")})

    def remap(self, input_min: int | float, input_max: int | float, output_start: int | float, output_end: int | float) -> "ScalarSignal":
        start, end = _finite(input_min, "input_min"), _finite(input_max, "input_max")
        if start >= end:
            raise ValueError("input_min must be smaller than input_max")
        return self._append({"type": "remap", "input_min": start, "input_max": end, "output_start": _finite(output_start, "output_start"), "output_end": _finite(output_end, "output_end")})

    def clamp(self, minimum: int | float, maximum: int | float) -> "ScalarSignal":
        low, high = _finite(minimum, "minimum"), _finite(maximum, "maximum")
        if low > high:
            raise ValueError("minimum must not exceed maximum")
        return self._append({"type": "clamp", "min": low, "max": high})

    def envelope(self, attack: int | float, release: int | float) -> "ScalarSignal":
        """Add smoothing with non-negative attack and release durations in seconds."""
        attack_seconds, release_seconds = _finite(attack, "attack"), _finite(release, "release")
        if attack_seconds < 0 or release_seconds < 0:
            raise ValueError("attack and release must be non-negative")
        return self._append({"type": "envelope", "attack": attack_seconds, "release": release_seconds})

    def response_curve(self, x1: int | float, y1: int | float, x2: int | float, y2: int | float) -> "ScalarSignal":
        first, second = _finite(x1, "x1"), _finite(x2, "x2")
        if not 0 <= first <= second <= 1:
            raise ValueError("response curve requires 0 <= x1 <= x2 <= 1")
        return self._append({"type": "response_curve", "x1": first, "y1": _finite(y1, "y1"), "x2": second, "y2": _finite(y2, "y2")})

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {"source": {"type": "audio", "tap": "master", "feature": self._feature.copy()}}
        if self._transforms:
            data["transforms"] = [transform.copy() for transform in self._transforms]
        return data


class MasterAudioSignals:
    """Factory for RMS, peak, and band-energy Master signals in Hz."""

    def rms(self) -> ScalarSignal:
        return ScalarSignal({"type": "rms"})

    def peak(self) -> ScalarSignal:
        return ScalarSignal({"type": "peak"})

    def band(self, min_hz: int | float, max_hz: int | float) -> ScalarSignal:
        low, high = _finite(min_hz, "min_hz"), _finite(max_hz, "max_hz")
        if low < 0 or low >= high or high > 24_000:
            raise ValueError("band requires 0 <= min_hz < max_hz <= 24000")
        return ScalarSignal({"type": "band_energy", "min_hz": low, "max_hz": high})
