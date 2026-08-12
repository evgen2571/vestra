"""Authoring-time Spectrum2D preset definitions."""

from __future__ import annotations

from dataclasses import dataclass
from types import MappingProxyType
from typing import Literal, Mapping, TypeAlias

from .errors import AuthoringError
from .values import Color

Spectrum2DPreset: TypeAlias = Literal["classic", "dense", "neon"]
Spectrum2DValue: TypeAlias = int | float | str | Color
Spectrum2DEffectKind: TypeAlias = Literal["glow", "bloom"]


class _Unset:
    pass


_UNSET = _Unset()


@dataclass(frozen=True, slots=True)
class Spectrum2DEffectPreset:
    kind: Spectrum2DEffectKind
    parameters: Mapping[str, Spectrum2DValue]


@dataclass(frozen=True, slots=True)
class Spectrum2DPresetDefinition:
    source: Mapping[str, Spectrum2DValue]
    effects: tuple[Spectrum2DEffectPreset, ...] = ()


def _source(**values: Spectrum2DValue) -> Mapping[str, Spectrum2DValue]:
    return MappingProxyType(values)


_BASE_SOURCE = _source(
    band_count=24,
    min_hz=40.0,
    max_hz=16_000.0,
    sensitivity=8.0,
    attack_seconds=0.020,
    release_seconds=0.150,
    x=0.10,
    y=0.70,
    width=0.80,
    height=0.25,
    bar_gap_ratio=0.20,
    colour="#ffffff",
)

_PRESETS: Mapping[str, Spectrum2DPresetDefinition] = MappingProxyType({
    "classic": Spectrum2DPresetDefinition(source=_BASE_SOURCE),
    "dense": Spectrum2DPresetDefinition(source=_source(
        band_count=48,
        min_hz=40.0,
        max_hz=18_000.0,
        sensitivity=9.0,
        attack_seconds=0.012,
        release_seconds=0.110,
        x=0.08,
        y=0.68,
        width=0.84,
        height=0.27,
        bar_gap_ratio=0.10,
        colour="#ffffff",
    )),
    "neon": Spectrum2DPresetDefinition(
        source=_source(
            band_count=32,
            min_hz=40.0,
            max_hz=16_000.0,
            sensitivity=10.0,
            attack_seconds=0.015,
            release_seconds=0.180,
            x=0.10,
            y=0.68,
            width=0.80,
            height=0.27,
            bar_gap_ratio=0.14,
            colour="#ffffff",
        ),
        effects=(
            Spectrum2DEffectPreset(
                "glow",
                _source(threshold=0.35, radius=3.0, intensity=0.85, colour="#ffffff"),
            ),
            Spectrum2DEffectPreset(
                "bloom",
                _source(threshold=0.55, radius=4.0, intensity=0.65),
            ),
        ),
    ),
})


def _spectrum2d_preset(name: Spectrum2DPreset) -> Spectrum2DPresetDefinition:
    """Return the immutable definition for one supported authoring preset."""
    try:
        return _PRESETS[name]
    except KeyError as error:
        raise AuthoringError(f"unknown Spectrum2D preset: {name!r}") from error


def _resolve_spectrum2d_source(
    preset: Spectrum2DPreset | None,
    overrides: Mapping[str, Spectrum2DValue | _Unset],
) -> tuple[Mapping[str, Spectrum2DValue], tuple[Spectrum2DEffectPreset, ...]]:
    """Expand defaults, a preset, and explicit overrides in that order."""
    definition = _spectrum2d_preset(preset) if preset is not None else None
    source = dict(_BASE_SOURCE)
    if definition is not None:
        source.update(definition.source)
    for key, value in overrides.items():
        if not isinstance(value, _Unset):
            source[key] = value
    return source, () if definition is None else definition.effects
