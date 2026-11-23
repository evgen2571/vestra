"""Spectrum2D layouts, styles and authoring presets."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from math import isfinite
from types import MappingProxyType
from typing import Literal, Mapping, TypeAlias, cast

from ._internal import _number
from .errors import AuthoringError
from .values import Color, Point, color_to_canonical

Spectrum2DPreset: TypeAlias = Literal[
    "classic", "dense", "neon", "mirror", "center_out", "circle", "neon_circle", "arc",
    "stereo_glow",
]
Spectrum2DStyle: TypeAlias = Literal["bars", "wave", "both"]
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

@dataclass(frozen=True, slots=True)
class Spectrum2DEllipseLayout:
    """An arc centered in the spectrum rectangle, with normalized radii."""

    radius_x: float
    radius_y: float
    start_angle_degrees: float = 0.0
    sweep_angle_degrees: float = 360.0
    direction: Spectrum2DRadialDirection = "outward"
    band_mapping: Literal["forward", "reverse"] = "forward"

    def __post_init__(self) -> None:
        for name in ("radius_x", "radius_y"):
            if not 0 < _number(getattr(self, name), name) <= 0.5:
                raise ValueError(f"{name} must be in (0, 0.5]")
        _number(self.start_angle_degrees, "start_angle_degrees")
        if not 0 < _number(self.sweep_angle_degrees, "sweep_angle_degrees") <= 360:
            raise ValueError("sweep_angle_degrees must be in (0, 360]")
        if self.direction not in {"outward", "inward", "both"}:
            raise ValueError("ellipse direction is invalid")
        if self.band_mapping not in {"forward", "reverse"}:
            raise ValueError("ellipse band_mapping is invalid")

    def to_canonical(self) -> dict[str, object]:
        return {
            "type": "ellipse",
            "radius_x": self.radius_x,
            "radius_y": self.radius_y,
            "start_angle_degrees": self.start_angle_degrees,
            "sweep_angle_degrees": self.sweep_angle_degrees,
            "direction": self.direction,
            "band_mapping": self.band_mapping,
        }


@dataclass(frozen=True, slots=True)
class Spectrum2DPathLayout:
    """A polyline of points normalized to the spectrum rectangle."""

    points: Sequence[Point | tuple[float, float]]
    closed: bool = False
    direction: Literal["left", "right", "both"] = "left"
    band_mapping: Literal["forward", "reverse"] = "forward"

    def __post_init__(self) -> None:
        if not isinstance(self.closed, bool):
            raise TypeError("closed must be a boolean")
        if self.direction not in {"left", "right", "both"}:
            raise ValueError("path direction is invalid")
        if self.band_mapping not in {"forward", "reverse"}:
            raise ValueError("path band_mapping is invalid")
        minimum = 3 if self.closed else 2
        if not minimum <= len(self.points) <= 256:
            raise ValueError(f"path must contain {minimum} to 256 points")

        points: list[Point] = []
        for point in self.points:
            if isinstance(point, Point):
                value = point
            elif isinstance(point, tuple) and len(point) == 2:
                value = Point(*point)
            else:
                raise TypeError("each path point must be a Point or an (x, y) tuple")
            if not 0 <= value.x <= 1 or not 0 <= value.y <= 1:
                raise ValueError("path points must be normalized to [0, 1]")
            if points and value == points[-1]:
                raise ValueError("consecutive path points must differ")
            points.append(value)
        if self.closed and points[0] == points[-1]:
            raise ValueError("the first and last points of a closed path must differ")
        object.__setattr__(self, "points", tuple(points))

    def to_canonical(self) -> dict[str, object]:
        return {
            "type": "path",
            "points": [point.to_canonical() for point in cast(tuple[Point, ...], self.points)],
            "closed": self.closed,
            "direction": self.direction,
            "band_mapping": self.band_mapping,
        }


Spectrum2DLayout: TypeAlias = (
    Spectrum2DLinearLayout | Spectrum2DRadialLayout | Spectrum2DEllipseLayout | Spectrum2DPathLayout
)

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


@dataclass(frozen=True, slots=True, kw_only=True)
class Spectrum2DStereo:
    """Independent tints and opacity weights for the two channel curves."""

    left_color: Color | str | None = None
    right_color: Color | str | None = None
    left_weight: float = 0.65
    right_weight: float = 1.0

    def __post_init__(self) -> None:
        for name in ("left_weight", "right_weight"):
            if not 0 <= _number(getattr(self, name), name) <= 1:
                raise ValueError(f"{name} must be in [0, 1]")
        for name in ("left_color", "right_color"):
            color = getattr(self, name)
            if color is not None:
                object.__setattr__(self, name, color_to_canonical(color))

    def to_canonical(self) -> dict[str, object]:
        result: dict[str, object] = {"left_weight": self.left_weight, "right_weight": self.right_weight}
        if self.left_color is not None:
            result["left_colour"] = self.left_color
        if self.right_color is not None:
            result["right_colour"] = self.right_color
        return result


@dataclass(frozen=True, slots=True, kw_only=True)
class Spectrum2DWave:
    """Stroke, fill and glow for a mixed or stereo wave."""

    stroke_width: float = 2.0
    color: Color | str | None = None
    fill_opacity: float = 0.18
    glow_radius: float = 0.0
    glow_opacity: float = 0.45
    bar_opacity: float = 1.0
    stereo: Spectrum2DStereo | None = None

    def __post_init__(self) -> None:
        if not 0 < _number(self.stroke_width, "stroke_width") <= 64:
            raise ValueError("stroke_width must be in (0, 64]")
        if not 0 <= _number(self.glow_radius, "glow_radius") <= 32:
            raise ValueError("glow_radius must be in [0, 32]")
        for name in ("fill_opacity", "glow_opacity", "bar_opacity"):
            if not 0 <= _number(getattr(self, name), name) <= 1:
                raise ValueError(f"{name} must be in [0, 1]")
        if self.color is not None:
            object.__setattr__(self, "color", color_to_canonical(self.color))
        if self.stereo is not None and not isinstance(self.stereo, Spectrum2DStereo):
            raise TypeError("stereo must be Spectrum2DStereo or None")

    def to_canonical(self) -> dict[str, object]:
        result: dict[str, object] = {
            "stroke_width": self.stroke_width,
            "fill_opacity": self.fill_opacity,
            "glow_radius": self.glow_radius,
            "glow_opacity": self.glow_opacity,
            "bar_opacity": self.bar_opacity,
        }
        if self.color is not None:
            result["colour"] = self.color
        if self.stereo is not None:
            result["stereo"] = self.stereo.to_canonical()
        return result


_DEFAULT_WAVE = Spectrum2DWave()


def _canonical_spectrum2d_options(
    layout: Spectrum2DLayout,
    gradient: Spectrum2DGradient | None,
    style: Spectrum2DStyle,
    wave: Spectrum2DWave | None,
    soft_knee: float | None,
    response_power: float,
) -> dict[str, object]:
    result: dict[str, object] = {}
    if not (
        isinstance(layout, Spectrum2DLinearLayout)
        and layout.anchor == "bottom"
        and layout.band_mapping == "forward"
    ):
        result["layout"] = layout.to_canonical()
    if gradient is not None:
        result["gradient"] = gradient.to_canonical()
    if style != "bars":
        result["style"] = style
    if wave is not None and wave != _DEFAULT_WAVE:
        result["wave"] = wave.to_canonical()
    if soft_knee is not None:
        result["soft_knee"] = soft_knee
    if response_power != 1.0:
        result["response_power"] = response_power
    return result


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
    style="bars",
    wave=None,
    soft_knee=None,
    response_power=1.0,
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
    "stereo_glow": Spectrum2DPresetDefinition(source=_source(
        band_count=32,
        min_hz=100.0,
        max_hz=6_000.0,
        sensitivity=2.3,
        attack_seconds=0.035,
        release_seconds=0.33,
        response_power=0.25,
        soft_knee=0.6,
        x=0.06,
        y=0.30,
        width=0.88,
        height=0.58,
        colour="#9bc3e0",
        style="both",
        wave=Spectrum2DWave(
            fill_opacity=0.22,
            glow_radius=10.0,
            glow_opacity=0.65,
            bar_opacity=0.18,
            stereo=Spectrum2DStereo(left_color="#819cfa", right_color="#8af3ee"),
        ),
    )),
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
