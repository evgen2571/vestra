"""The mutable Python builder that feeds the native canonical parser."""

from collections.abc import Mapping
from math import isfinite
import os
from pathlib import Path
from typing import TypeAlias, cast, overload

from vestra import Editor, FrameRate, Project, ValidationReport

from ._internal import _IdAllocator, _Owner, _number, _require_owner
from .assets import AudioAsset, ImageAsset
from .audio import AudioTimeline
from .clips import ImageClip, SolidColorClip, Spectrum2DClip
from .effects import ClipEffectCollection, PostEffectCollection
from .flashes import FlashCollection
from .spectrum2d import (
    Spectrum2DEffectPreset, Spectrum2DPreset, Spectrum2DValue, Spectrum2DLayout, Spectrum2DGradient, Spectrum2DLinearLayout, _UNSET, _Unset,
    _resolve_spectrum2d_source,
)
from .transitions import TransitionCollection
from .timeline import Timeline
from .values import Color, Crop, DurationMode, Quality, Sizing, color_to_canonical

JsonScalar: TypeAlias = str | int | float | bool | None
JsonValue: TypeAlias = JsonScalar | list["JsonValue"] | dict[str, "JsonValue"]
CanonicalProject: TypeAlias = dict[str, object]


def _integer(value: int, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{name} must be an integer")
    return value


def _path(value: str | os.PathLike[str], name: str) -> str:
    try:
        path = os.fspath(value)
    except TypeError as error:
        raise TypeError(f"{name} must be str or PathLike[str]") from error
    if not isinstance(path, str):
        raise TypeError(f"{name} must be str or PathLike[str]")
    return path


def _json_snapshot(value: JsonValue, path: str = "metadata") -> JsonValue:
    if value is None:
        return None
    if isinstance(value, str | bool):
        return value
    if isinstance(value, int):
        return value
    if isinstance(value, float):
        if not isfinite(value):
            raise ValueError(f"{path} floats must be finite")
        return value
    if isinstance(value, list):
        return [_json_snapshot(item, f"{path}[{index}]") for index, item in enumerate(value)]
    if isinstance(value, Mapping):
        copied: dict[str, JsonValue] = {}
        for key, item in value.items():
            if not isinstance(key, str):
                raise TypeError(f"{path} keys must be strings")
            copied[key] = _json_snapshot(item, f"{path}.{key}")
        return copied
    raise TypeError(f"{path} must contain JSON-compatible values")


class ProjectBuilder:
    """Mutable authoring state that builds immutable native project snapshots.

    ``build()`` only parses canonical data.  ``validate()`` runs deterministic
    native validation and does not perform environment-dependent preflight.
    """

    def __init__(
        self,
        *,
        width: int,
        height: int,
        frame_rate: FrameRate,
        output_path: str | os.PathLike[str],
        duration: int | float | None = None,
        duration_mode: DurationMode | None = None,
        background: Color | str = "#000000",
        quality: Quality = Quality.BALANCED,
        base_directory: str | os.PathLike[str] = ".",
        name: str | None = None,
        metadata: JsonValue | None = None,
        output_audio: bool = False,
    ) -> None:
        self._owner = _Owner()
        self._ids = _IdAllocator()
        self._width = _integer(width, "width")
        self._height = _integer(height, "height")
        self._frame_rate = self._frame_rate_value(frame_rate)
        self._output_path = _path(output_path, "output_path")
        self._background = color_to_canonical(background)
        self._quality = self._quality_value(quality)
        self._base_directory = Path(_path(base_directory, "base_directory"))
        self._name = self._name_value(name)
        self._metadata = None if metadata is None else _json_snapshot(metadata)
        if duration_mode is not None and not isinstance(duration_mode, DurationMode):
            raise TypeError("duration_mode must be DurationMode or None")
        if duration is None:
            if duration_mode is DurationMode.EXPLICIT:
                raise ValueError("explicit duration mode requires duration")
            self._duration_mode = DurationMode.AUTOMATIC
            self._duration: float | None = None
        else:
            if duration_mode is DurationMode.AUTOMATIC:
                raise ValueError("automatic duration mode must not specify duration")
            self._duration_mode = DurationMode.EXPLICIT
            self._duration = _number(duration, "duration")
        self._assets: list[ImageAsset | AudioAsset] = []
        self._clips: list[ImageClip | SolidColorClip | Spectrum2DClip] = []
        if not isinstance(output_audio, bool):
            raise TypeError("output_audio must be a boolean")
        self._output_audio = output_audio
        self._audio = AudioTimeline._create(self._owner, self._ids)
        self._post_effects = PostEffectCollection._create(self._owner, self._ids, self)
        self._transitions = TransitionCollection._create(self._owner, self._ids, self)
        self._flashes = FlashCollection._create(self._owner, self._ids, self)
        self._timeline = Timeline._create(self)

    @staticmethod
    def _frame_rate_value(value: FrameRate) -> FrameRate:
        if not isinstance(value, FrameRate):
            raise TypeError("frame_rate must be vestra.FrameRate")
        return value

    @staticmethod
    def _quality_value(value: Quality) -> Quality:
        if not isinstance(value, Quality):
            raise TypeError("quality must be Quality")
        return value

    @staticmethod
    def _name_value(value: str | None) -> str | None:
        if value is not None and not isinstance(value, str):
            raise TypeError("name must be a string or None")
        return value

    @property
    def width(self) -> int:
        return self._width

    @width.setter
    def width(self, value: int) -> None:
        self._width = _integer(value, "width")

    @property
    def height(self) -> int:
        return self._height

    @height.setter
    def height(self, value: int) -> None:
        self._height = _integer(value, "height")

    @property
    def frame_rate(self) -> FrameRate:
        return self._frame_rate

    @frame_rate.setter
    def frame_rate(self, value: FrameRate) -> None:
        self._frame_rate = self._frame_rate_value(value)

    @property
    def output_path(self) -> str:
        return self._output_path

    @output_path.setter
    def output_path(self, value: str | os.PathLike[str]) -> None:
        self._output_path = _path(value, "output_path")

    @property
    def background(self) -> str:
        return self._background

    @background.setter
    def background(self, value: Color | str) -> None:
        self._background = color_to_canonical(value)

    @property
    def quality(self) -> Quality:
        return self._quality

    @quality.setter
    def quality(self, value: Quality) -> None:
        self._quality = self._quality_value(value)

    @property
    def base_directory(self) -> Path:
        return self._base_directory

    @base_directory.setter
    def base_directory(self, value: str | os.PathLike[str]) -> None:
        self._base_directory = Path(_path(value, "base_directory"))

    @property
    def name(self) -> str | None:
        return self._name

    @name.setter
    def name(self, value: str | None) -> None:
        self._name = self._name_value(value)

    @property
    def metadata(self) -> JsonValue | None:
        return None if self._metadata is None else _json_snapshot(self._metadata)

    @metadata.setter
    def metadata(self, value: JsonValue | None) -> None:
        self._metadata = None if value is None else _json_snapshot(value)

    @property
    def duration(self) -> float | None:
        return self._duration

    @duration.setter
    def duration(self, value: int | float | None) -> None:
        if value is None:
            self._duration = None
            self._duration_mode = DurationMode.AUTOMATIC
        else:
            self._duration = _number(value, "duration")
            self._duration_mode = DurationMode.EXPLICIT

    @property
    def duration_mode(self) -> DurationMode:
        return self._duration_mode

    @duration_mode.setter
    def duration_mode(self, value: DurationMode) -> None:
        if not isinstance(value, DurationMode):
            raise TypeError("duration_mode must be DurationMode")
        if value is DurationMode.EXPLICIT and self._duration is None:
            raise ValueError("explicit duration mode requires duration")
        self._duration_mode = value
        if value is DurationMode.AUTOMATIC:
            self._duration = None

    @property
    def assets(self) -> tuple[ImageAsset | AudioAsset, ...]:
        """Registered assets in canonical registration order."""
        return tuple(self._assets)

    @property
    def clips(self) -> tuple[ImageClip | SolidColorClip | Spectrum2DClip, ...]:
        """Visual clips in canonical creation order."""
        return tuple(self._clips)

    @property
    def audio(self) -> AudioTimeline:
        """Stable builder-owned schema-v2 audio timeline."""
        return self._audio

    @property
    def output_audio(self) -> bool:
        return self._output_audio

    @output_audio.setter
    def output_audio(self, value: bool) -> None:
        if not isinstance(value, bool): raise TypeError("output_audio must be a boolean")
        self._output_audio = value

    @property
    def post_effects(self) -> PostEffectCollection:
        return self._post_effects

    @property
    def transitions(self) -> TransitionCollection:
        """Stable ordered project-level coordinated transitions."""
        return self._transitions

    @property
    def flashes(self) -> FlashCollection:
        """Stable ordered project-level flash overlays."""
        return self._flashes

    @property
    def timeline(self) -> Timeline:
        """Stable non-canonical helpers for explicit timeline mutations."""
        return self._timeline

    @staticmethod
    def _asset_source(value: str | os.PathLike[str]) -> str:
        source = _path(value, "source")
        if not source or source.isspace():
            raise ValueError("source must not be empty")
        return source

    def add_image_asset(self, source: str | os.PathLike[str], *, id: str | None = None) -> ImageAsset:
        """Register an image path without probing or decoding it."""
        normalized_source = self._asset_source(source)
        if id is not None:
            self._ids.validate("asset", id)
        identifier = self._ids.allocate("asset", "image") if id is None else self._ids.reserve("asset", id)
        asset = ImageAsset._create(identifier, normalized_source, self._owner)
        self._assets.append(asset)
        return asset

    def add_audio_asset(self, source: str | os.PathLike[str], *, id: str | None = None) -> AudioAsset:
        """Register an audio path without probing or decoding it."""
        normalized_source = self._asset_source(source)
        if id is not None:
            self._ids.validate("asset", id)
        identifier = self._ids.allocate("asset", "audio") if id is None else self._ids.reserve("asset", id)
        asset = AudioAsset._create(identifier, normalized_source, self._owner)
        self._assets.append(asset)
        return asset

    def add_image_clip(
        self, *, source: ImageAsset, start: int | float, duration: int | float, layer: int,
        visible: bool = True, sizing: Sizing | None = None, crop: Crop | None = None,
        opacity: int | float = 1.0, id: str | None = None,
    ) -> ImageClip:
        """Create a static image clip using a registered image asset."""
        if not isinstance(source, ImageAsset):
            raise TypeError("source must be ImageAsset")
        _require_owner(self._owner, source._owner)
        staged = ImageClip._create(self._owner, "", source, start=start, duration=duration, layer=layer,
                                   visible=visible, sizing=sizing, crop=crop, opacity=opacity)
        if id is not None:
            self._ids.validate("clip", id)
        identifier = self._ids.allocate("clip") if id is None else self._ids.reserve("clip", id)
        staged._id = identifier
        clip = staged
        clip._attach_effects(ClipEffectCollection._create(self._owner, self._ids, clip))
        self._clips.append(clip)
        return clip

    def add_solid_color_clip(
        self, *, colour: Color | str, start: int | float, duration: int | float, layer: int,
        visible: bool = True, opacity: int | float = 1.0, id: str | None = None,
    ) -> SolidColorClip:
        """Create a full-canvas static solid-colour clip."""
        staged = SolidColorClip._create(self._owner, "", colour, start=start, duration=duration,
                                        layer=layer, visible=visible, opacity=opacity)
        if id is not None:
            self._ids.validate("clip", id)
        identifier = self._ids.allocate("clip") if id is None else self._ids.reserve("clip", id)
        staged._id = identifier
        clip = staged
        clip._attach_effects(ClipEffectCollection._create(self._owner, self._ids, clip))
        self._clips.append(clip)
        return clip

    @overload
    def add_spectrum2d_clip(
        self, *, start: int | float, duration: int | float, layer: int, visible: bool = True,
        opacity: int | float = 1.0, id: str | None = None, preset: None = None,
        band_count: int = 24, min_hz: int | float = 40.0, max_hz: int | float = 16_000.0,
        sensitivity: int | float = 8.0, attack_seconds: int | float = 0.020,
        release_seconds: int | float = 0.150, x: int | float = 0.10, y: int | float = 0.70,
        width: int | float = 0.80, height: int | float = 0.25,
        bar_gap_ratio: int | float = 0.20, colour: Color | str = "#ffffff",
        min_bar_height_ratio: int | float = 0.0, layout: Spectrum2DLayout = Spectrum2DLinearLayout(), gradient: Spectrum2DGradient | None = None,
    ) -> Spectrum2DClip: ...

    @overload
    def add_spectrum2d_clip(
        self, *, start: int | float, duration: int | float, layer: int, visible: bool = True,
        opacity: int | float = 1.0, id: str | None = None, preset: Spectrum2DPreset,
        band_count: int = 24, min_hz: int | float = 40.0, max_hz: int | float = 16_000.0,
        sensitivity: int | float = 8.0, attack_seconds: int | float = 0.020,
        release_seconds: int | float = 0.150, x: int | float = 0.10, y: int | float = 0.70,
        width: int | float = 0.80, height: int | float = 0.25,
        bar_gap_ratio: int | float = 0.20, colour: Color | str = "#ffffff",
        min_bar_height_ratio: int | float = 0.0, layout: Spectrum2DLayout = Spectrum2DLinearLayout(), gradient: Spectrum2DGradient | None = None,
    ) -> Spectrum2DClip: ...

    def add_spectrum2d_clip(
        self, *, start: int | float, duration: int | float, layer: int, visible: bool = True,
        opacity: int | float = 1.0, id: str | None = None,
        preset: Spectrum2DPreset | None = None,
        band_count: object = _UNSET, min_hz: object = _UNSET, max_hz: object = _UNSET,
        sensitivity: object = _UNSET, attack_seconds: object = _UNSET,
        release_seconds: object = _UNSET, x: object = _UNSET, y: object = _UNSET,
        width: object = _UNSET, height: object = _UNSET, bar_gap_ratio: object = _UNSET,
        colour: object = _UNSET, min_bar_height_ratio: object = _UNSET,
        layout: object = _UNSET, gradient: object = _UNSET,
    ) -> Spectrum2DClip:
        """Create a Master-audio-driven normalized linear Spectrum2D clip.

        Explicit Spectrum2D arguments override the selected authoring preset.
        """
        source, preset_effects = _resolve_spectrum2d_source(
            preset,
            cast(Mapping[str, Spectrum2DValue | _Unset], {
                "band_count": band_count, "min_hz": min_hz, "max_hz": max_hz,
                "sensitivity": sensitivity, "attack_seconds": attack_seconds,
                "release_seconds": release_seconds, "x": x, "y": y, "width": width,
                "height": height, "bar_gap_ratio": bar_gap_ratio, "colour": colour,
                "min_bar_height_ratio": min_bar_height_ratio, "layout": layout, "gradient": gradient,
            }),
        )
        staged = Spectrum2DClip._create(
            self._owner, "", start=start, duration=duration, layer=layer, visible=visible,
            opacity=opacity, band_count=cast(int, source["band_count"]),
            min_hz=cast(int | float, source["min_hz"]), max_hz=cast(int | float, source["max_hz"]),
            sensitivity=cast(int | float, source["sensitivity"]),
            attack_seconds=cast(int | float, source["attack_seconds"]),
            release_seconds=cast(int | float, source["release_seconds"]),
            x=cast(int | float, source["x"]), y=cast(int | float, source["y"]),
            width=cast(int | float, source["width"]), height=cast(int | float, source["height"]),
            bar_gap_ratio=cast(int | float, source["bar_gap_ratio"]),
            colour=cast(Color | str, source["colour"]),
            min_bar_height_ratio=cast(int | float, source["min_bar_height_ratio"]),
            layout=cast(Spectrum2DLayout, source["layout"]),
            gradient=cast(Spectrum2DGradient | None, source.get("gradient")),
        )
        if id is not None:
            self._ids.validate("clip", id)
        staged._id = self._ids.allocate("clip", "spectrum2d") if id is None else self._ids.reserve("clip", id)
        staged._attach_effects(ClipEffectCollection._create(self._owner, self._ids, staged))
        for effect in preset_effects:
            self._add_spectrum2d_preset_effect(staged, effect)
        self._clips.append(staged)
        return staged

    @staticmethod
    def _add_spectrum2d_preset_effect(clip: Spectrum2DClip, effect: Spectrum2DEffectPreset) -> None:
        parameters = dict(effect.parameters)
        if effect.kind == "glow":
            clip.effects.add_glow(
                threshold=cast(int | float, parameters["threshold"]),
                radius=cast(int | float, parameters["radius"]),
                intensity=cast(int | float, parameters["intensity"]),
                colour=cast(Color | str, parameters["colour"]),
            )
        else:
            clip.effects.add_bloom(
                threshold=cast(int | float, parameters["threshold"]),
                radius=cast(int | float, parameters["radius"]),
                intensity=cast(int | float, parameters["intensity"]),
            )

    def to_dict(self) -> CanonicalProject:
        output: dict[str, object] = {
            "path": self.output_path,
            "width": self.width,
            "height": self.height,
            "frame_rate": f"{self.frame_rate.numerator}/{self.frame_rate.denominator}",
            "background": color_to_canonical(self.background),
            "quality": self.quality.to_canonical(),
            "audio": self._output_audio,
            "duration_mode": self.duration_mode.to_canonical(),
        }
        if self.duration is not None:
            output["duration"] = self.duration
        data: CanonicalProject = {
            "schema_version": 2,
            "output": output,
            "assets": [
                {"id": asset.id, "type": asset.kind, "source": asset.source}
                for asset in self._assets
            ],
            "visual": {
                "clips": [clip.to_canonical() for clip in self._clips],
                "transitions": [transition.to_canonical() for transition in self.transitions.items],
                "flashes": [flash.to_canonical() for flash in self.flashes.items],
                "post_effects": [effect.to_canonical() for effect in self.post_effects.items],
            },
        }
        if self.name is not None:
            data["name"] = self.name
        if self.metadata is not None:
            data["metadata"] = _json_snapshot(self.metadata)
        if self._audio.tracks:
            data["audio"] = self._audio.to_canonical()
        return data

    def build(self) -> Project:
        return Project.from_dict(self.to_dict(), base_directory=self.base_directory)

    def validate(self) -> ValidationReport:
        return Editor().validate(self.build())
