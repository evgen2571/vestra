"""Authoring-time Spectrum2D preset definitions."""

from __future__ import annotations

from dataclasses import dataclass
from types import MappingProxyType
from math import isfinite
from typing import Literal, Mapping, TypeAlias

from .errors import AuthoringError
from .values import Color, color_to_canonical

Spectrum2DPreset: TypeAlias = Literal["classic", "dense", "neon", "mirror", "center_out", "circle", "neon_circle", "arc"]
Spectrum2DValue: TypeAlias = object
Spectrum2DEffectKind: TypeAlias = Literal["glow", "bloom"]
Spectrum2DAnchor: TypeAlias = Literal["bottom", "top", "center"]
Spectrum2DBandMapping: TypeAlias = Literal["forward", "reverse", "center_out"]
Spectrum2DRadialDirection: TypeAlias = Literal["outward", "inward", "both"]
Spectrum2DGradientDirection: TypeAlias = Literal["along_bar", "across_bands"]


class _Unset:
    pass


_UNSET = _Unset()

@dataclass(frozen=True, slots=True)
class Spectrum2DLinearLayout:
    anchor: Spectrum2DAnchor = "bottom"
    band_mapping: Spectrum2DBandMapping = "forward"

    def __post_init__(self) -> None:
        if self.anchor not in {"bottom", "top", "center"}:
            raise ValueError("linear anchor is invalid")
        if self.band_mapping not in {"forward", "reverse", "center_out"}:
            raise ValueError("linear band_mapping is invalid")

    def to_canonical(self) -> dict[str, object]:
        return {"type": "linear", "anchor": self.anchor, "band_mapping": self.band_mapping}

@dataclass(frozen=True, slots=True)
class Spectrum2DRadialLayout:
    inner_radius_ratio: float = 0.55
    start_angle_degrees: float = 0.0
    sweep_angle_degrees: float = 360.0
    direction: Spectrum2DRadialDirection = "outward"
    band_mapping: Literal["forward", "reverse"] = "forward"

    def __post_init__(self) -> None:
        for name in ("inner_radius_ratio", "start_angle_degrees", "sweep_angle_degrees"):
            value = getattr(self, name)
            if isinstance(value, bool) or not isinstance(value, int | float):
                raise TypeError(f"{name} must be a real number")
            if not isfinite(float(value)):
                raise ValueError(f"{name} must be finite")
        if not 0 <= self.inner_radius_ratio < 1:
            raise ValueError("inner_radius_ratio must be in [0, 1)")
        if not 0 < self.sweep_angle_degrees <= 360:
            raise ValueError("sweep_angle_degrees must be in (0, 360]")
        if self.direction not in {"outward", "inward", "both"}:
            raise ValueError("radial direction is invalid")
        if self.band_mapping not in {"forward", "reverse"}:
            raise ValueError("radial band_mapping is invalid")
    def to_canonical(self) -> dict[str, object]:
        return {"type": "radial", "inner_radius_ratio": self.inner_radius_ratio,
                "start_angle_degrees": self.start_angle_degrees,
                "sweep_angle_degrees": self.sweep_angle_degrees,
                "direction": self.direction, "band_mapping": self.band_mapping}

Spectrum2DLayout: TypeAlias = Spectrum2DLinearLayout | Spectrum2DRadialLayout

@dataclass(frozen=True, slots=True)
class Spectrum2DGradient:
    start_color: Color | str
    end_color: Color | str
    direction: Spectrum2DGradientDirection

    def __post_init__(self) -> None:
        object.__setattr__(self, "start_color", color_to_canonical(self.start_color))
        object.__setattr__(self, "end_color", color_to_canonical(self.end_color))
        if self.direction not in {"along_bar", "across_bands"}:
            raise ValueError("gradient direction is invalid")

    def to_canonical(self) -> dict[str, object]:
        from .values import color_to_canonical
        return {"start_colour": color_to_canonical(self.start_color),
                "end_colour": color_to_canonical(self.end_color), "direction": self.direction}


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
    min_bar_height_ratio=0.0,
    layout=Spectrum2DLinearLayout(),
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
    "mirror": Spectrum2DPresetDefinition(source=_source(band_count=32, sensitivity=9.0, x=0.08, y=0.58, width=0.84, height=0.32, bar_gap_ratio=0.14, layout=Spectrum2DLinearLayout("center", "forward"))),
    "center_out": Spectrum2DPresetDefinition(source=_source(x=0.08, width=0.84, bar_gap_ratio=0.12, layout=Spectrum2DLinearLayout("bottom", "center_out"))),
    "circle": Spectrum2DPresetDefinition(source=_source(band_count=32, sensitivity=9.0, attack_seconds=0.015, x=0.22, y=0.22, width=0.56, height=0.56, bar_gap_ratio=0.18, min_bar_height_ratio=0.02, layout=Spectrum2DRadialLayout(0.58))),
    "neon_circle": Spectrum2DPresetDefinition(
        source=_source(band_count=48, max_hz=18_000.0, sensitivity=10.0, attack_seconds=0.012, release_seconds=0.160, x=0.20, y=0.20, width=0.60, height=0.60, bar_gap_ratio=0.12, min_bar_height_ratio=0.035, layout=Spectrum2DRadialLayout(0.55), gradient=Spectrum2DGradient("#00ffff", "#ff00ff", "across_bands")),
        effects=(Spectrum2DEffectPreset("glow", _source(threshold=0.35, radius=3.0, intensity=0.85, colour="#ffffff")), Spectrum2DEffectPreset("bloom", _source(threshold=0.55, radius=4.0, intensity=0.65))),
    ),
    "arc": Spectrum2DPresetDefinition(source=_source(band_count=32, x=0.15, y=0.22, width=0.70, height=0.70, bar_gap_ratio=0.14, min_bar_height_ratio=0.02, layout=Spectrum2DRadialLayout(0.55, 270.0, 180.0))),
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
