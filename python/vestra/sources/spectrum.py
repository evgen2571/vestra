"""Spectrum source descriptors and layout configuration values."""

from __future__ import annotations

from typing import cast

from ..authoring._internal import _Owner
from ..authoring.clips import Spectrum2DClip
from ..authoring.spectrum2d import (
    Spectrum2DEllipseLayout,
    Spectrum2DGradient,
    Spectrum2DLayout,
    Spectrum2DLinearLayout,
    Spectrum2DPathLayout,
    Spectrum2DPreset,
    Spectrum2DRadialLayout,
    Spectrum2DStereo,
    Spectrum2DStyle,
    Spectrum2DWave,
    _UNSET,
    _Unset,
    _canonical_spectrum2d_options,
    _resolve_spectrum2d_source,
)
from ..authoring.values import Color as AuthoringColor, color_to_canonical
from ..properties.transform import Transform, TransformPoint, TransformScale
from .base import Source

_SPECTRUM_FIELDS = (
    "band_count",
    "min_hz",
    "max_hz",
    "sensitivity",
    "attack_seconds",
    "release_seconds",
    "x",
    "y",
    "width",
    "height",
    "bar_gap_ratio",
    "colour",
    "min_bar_height_ratio",
    "layout",
    "gradient",
    "style",
    "wave",
    "soft_knee",
    "response_power",
)


class Spectrum2D(Source):
    """An audio-reactive Spectrum2D source using the authoring presets."""

    __slots__ = ("_preset", "_overrides", "_values")

    _preset: Spectrum2DPreset | None

    def __init__(
        self,
        *,
        preset: Spectrum2DPreset | None = None,
        band_count: int | None = None,
        min_hz: int | float | None = None,
        max_hz: int | float | None = None,
        sensitivity: int | float | None = None,
        attack_seconds: int | float | None = None,
        release_seconds: int | float | None = None,
        x: int | float | None = None,
        y: int | float | None = None,
        width: int | float | None = None,
        height: int | float | None = None,
        bar_gap_ratio: int | float | None = None,
        colour: str | AuthoringColor | None = None,
        min_bar_height_ratio: int | float | None = None,
        layout: Spectrum2DLayout | None = None,
        gradient: Spectrum2DGradient | None | _Unset = _UNSET,
        style: Spectrum2DStyle | None = None,
        wave: Spectrum2DWave | None | _Unset = _UNSET,
        soft_knee: int | float | None | _Unset = _UNSET,
        response_power: int | float | None = None,
        transform: Transform | None = None,
        position: TransformPoint | None = None,
        anchor: TransformPoint | None = None,
        scale: TransformScale | None = None,
        rotation: int | float | None = None,
        rotation_degrees: int | float | None = None,
    ) -> None:
        super().__init__(
            transform=transform,
            position=position,
            anchor=anchor,
            scale=scale,
            rotation=rotation,
            rotation_degrees=rotation_degrees,
        )
        values = {
            "band_count": band_count,
            "min_hz": min_hz,
            "max_hz": max_hz,
            "sensitivity": sensitivity,
            "attack_seconds": attack_seconds,
            "release_seconds": release_seconds,
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "bar_gap_ratio": bar_gap_ratio,
            "colour": colour,
            "min_bar_height_ratio": min_bar_height_ratio,
            "layout": layout,
            "gradient": gradient,
            "style": style,
            "wave": wave,
            "soft_knee": soft_knee,
            "response_power": response_power,
        }
        self._preset = preset
        overrides_values: dict[str, object] = {
            name: (
                color_to_canonical(cast(str | AuthoringColor, value))
                if name == "colour" and value is not None
                else value
            )
            for name, value in values.items()
            if value is not None and value is not _UNSET
        }
        self._overrides = overrides_values
        overrides = {
            name: value if name in self._overrides else _UNSET
            for name, value in values.items()
        }
        for name in ("gradient", "wave", "soft_knee"):
            if values[name] is None:
                self._overrides[name] = None
                overrides[name] = None
        resolved, _ = _resolve_spectrum2d_source(preset, overrides)
        self._values = dict(resolved)
        self._validate()

    @property
    def preset(self) -> Spectrum2DPreset | None:
        return self._preset

    def _validate(self) -> None:
        values = self._values
        Spectrum2DClip._create(
            _Owner(),
            "spectrum",
            start=0,
            duration=1,
            layer=0,
            visible=True,
            opacity=1,
            band_count=cast(int, values["band_count"]),
            min_hz=cast(float, values["min_hz"]),
            max_hz=cast(float, values["max_hz"]),
            sensitivity=cast(float, values["sensitivity"]),
            attack_seconds=cast(float, values["attack_seconds"]),
            release_seconds=cast(float, values["release_seconds"]),
            x=cast(float, values["x"]),
            y=cast(float, values["y"]),
            width=cast(float, values["width"]),
            height=cast(float, values["height"]),
            bar_gap_ratio=cast(float, values["bar_gap_ratio"]),
            colour=cast(str, values["colour"]),
            min_bar_height_ratio=cast(float, values["min_bar_height_ratio"]),
            layout=values["layout"],
            gradient=values.get("gradient"),
            style=values["style"],
            wave=values["wave"],
            soft_knee=cast(float | None, values["soft_knee"]),
            response_power=cast(float, values["response_power"]),
        )

    def value(self, name: str) -> object:
        if name not in _SPECTRUM_FIELDS:
            raise AttributeError(name)
        return self._values[name]

    @property
    def band_count(self) -> int:
        return cast(int, self._values["band_count"])

    @property
    def min_hz(self) -> float:
        return cast(float, self._values["min_hz"])

    @property
    def max_hz(self) -> float:
        return cast(float, self._values["max_hz"])

    @property
    def sensitivity(self) -> float:
        return cast(float, self._values["sensitivity"])

    @property
    def attack_seconds(self) -> float:
        return cast(float, self._values["attack_seconds"])

    @property
    def release_seconds(self) -> float:
        return cast(float, self._values["release_seconds"])

    @property
    def x(self) -> float:
        return cast(float, self._values["x"])

    @property
    def y(self) -> float:
        return cast(float, self._values["y"])

    @property
    def width(self) -> float:
        return cast(float, self._values["width"])

    @property
    def height(self) -> float:
        return cast(float, self._values["height"])

    @property
    def bar_gap_ratio(self) -> float:
        return cast(float, self._values["bar_gap_ratio"])

    @property
    def colour(self) -> str:
        return cast(str, self._values["colour"])

    @property
    def min_bar_height_ratio(self) -> float:
        return cast(float, self._values["min_bar_height_ratio"])

    @property
    def layout(self) -> Spectrum2DLayout:
        return cast(Spectrum2DLayout, self._values["layout"])

    @property
    def gradient(self) -> Spectrum2DGradient | None:
        return cast(Spectrum2DGradient | None, self._values.get("gradient"))

    @property
    def style(self) -> Spectrum2DStyle:
        return cast(Spectrum2DStyle, self._values["style"])

    @property
    def wave(self) -> Spectrum2DWave | None:
        return cast(Spectrum2DWave | None, self._values["wave"])

    @property
    def soft_knee(self) -> float | None:
        return cast(float | None, self._values["soft_knee"])

    @property
    def response_power(self) -> float:
        return cast(float, self._values["response_power"])

    def lowering_kwargs(self) -> dict[str, object]:
        return dict(self._overrides)

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {
            "type": "spectrum2d", "band_count": self.band_count,
            "min_hz": self.min_hz, "max_hz": self.max_hz,
            "sensitivity": self.sensitivity, "attack_seconds": self.attack_seconds,
            "release_seconds": self.release_seconds, "x": self.x, "y": self.y,
            "width": self.width, "height": self.height,
            "bar_gap_ratio": self.bar_gap_ratio, "colour": self.colour,
            "min_bar_height_ratio": self.min_bar_height_ratio,
        }
        data.update(_canonical_spectrum2d_options(
            self.layout, self.gradient, self.style, self.wave, self.soft_knee, self.response_power,
        ))
        return data




__all__ = [
    "Spectrum2D", "Spectrum2DPreset", "Spectrum2DGradient", "Spectrum2DLayout",
    "Spectrum2DLinearLayout", "Spectrum2DRadialLayout", "Spectrum2DEllipseLayout",
    "Spectrum2DPathLayout", "Spectrum2DStereo", "Spectrum2DStyle", "Spectrum2DWave",
]
