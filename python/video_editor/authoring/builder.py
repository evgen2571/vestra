"""The mutable Python builder that feeds the native canonical parser."""

from collections.abc import Mapping
from math import isfinite
import os
from pathlib import Path
from typing import TypeAlias

from video_editor import Editor, FrameRate, Project, ValidationReport

from ._internal import _IdAllocator, _Owner, _require_owner
from .assets import AudioAsset, ImageAsset
from .audio import AudioTrack
from .clips import ImageClip, SolidColorClip
from .values import Color, Crop, DurationMode, Quality, Sizing, color_to_canonical

JsonScalar: TypeAlias = str | int | float | bool | None
JsonValue: TypeAlias = JsonScalar | list["JsonValue"] | dict[str, "JsonValue"]
CanonicalProject: TypeAlias = dict[str, object]


def _integer(value: int, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{name} must be an integer")
    return value


def _number(value: int | float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    number = float(value)
    if not isfinite(number):
        raise ValueError(f"{name} must be finite")
    return number


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
        self._clips: list[ImageClip | SolidColorClip] = []
        self._audio: AudioTrack | None = None
        self._has_audio = False

    @staticmethod
    def _frame_rate_value(value: FrameRate) -> FrameRate:
        if not isinstance(value, FrameRate):
            raise TypeError("frame_rate must be video_editor.FrameRate")
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
    def clips(self) -> tuple[ImageClip | SolidColorClip, ...]:
        """Visual clips in canonical creation order."""
        return tuple(self._clips)

    @property
    def audio(self) -> AudioTrack | None:
        """The stable global audio node, once one has been configured.

        Use :attr:`has_audio` to determine whether it is currently serialized.
        """
        return self._audio

    @property
    def has_audio(self) -> bool:
        """Whether the stable global audio node is enabled for serialization."""
        return self._has_audio

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
        self._clips.append(clip)
        return clip

    def set_audio(
        self, *, asset: AudioAsset, timeline_start: int | float, trim_start: int | float,
        trim_end: int | float | None = None, volume: int | float = 1.0,
        fade_in: int | float = 0.0, fade_out: int | float = 0.0, mute: bool = False,
    ) -> AudioTrack:
        """Replace the project's one global audio track."""
        if not isinstance(asset, AudioAsset):
            raise TypeError("asset must be AudioAsset")
        _require_owner(self._owner, asset._owner)
        staged = AudioTrack._create(self._owner, asset, timeline_start=timeline_start,
                                    trim_start=trim_start, trim_end=trim_end, volume=volume,
                                    fade_in=fade_in, fade_out=fade_out, mute=mute)
        if self._audio is None:
            self._audio = staged
        else:
            self._audio._replace_from(staged)
        self._has_audio = True
        return self._audio

    def clear_audio(self) -> None:
        """Remove the configured global audio track."""
        self._has_audio = False

    def to_dict(self) -> CanonicalProject:
        output: dict[str, object] = {
            "path": self.output_path,
            "width": self.width,
            "height": self.height,
            "frame_rate": f"{self.frame_rate.numerator}/{self.frame_rate.denominator}",
            "background": color_to_canonical(self.background),
            "quality": self.quality.to_canonical(),
            "audio": self._has_audio,
            "duration_mode": self.duration_mode.to_canonical(),
        }
        if self.duration is not None:
            output["duration"] = self.duration
        data: CanonicalProject = {
            "schema_version": 1,
            "output": output,
            "assets": [
                {"id": asset.id, "type": asset.kind, "source": asset.source}
                for asset in self._assets
            ],
            "visual": {
                "clips": [clip.to_canonical() for clip in self._clips],
                "transitions": [],
                "flashes": [],
                "post_effects": [],
            },
        }
        if self.name is not None:
            data["name"] = self.name
        if self.metadata is not None:
            data["metadata"] = _json_snapshot(self.metadata)
        if self._has_audio:
            assert self._audio is not None
            data["audio"] = self._audio.to_canonical()
        return data

    def build(self) -> Project:
        return Project.from_dict(self.to_dict(), base_directory=self.base_directory)

    def validate(self) -> ValidationReport:
        return Editor().validate(self.build())
