"""The high-level, mutable editor model."""

from __future__ import annotations

import os
from collections.abc import Callable
from pathlib import Path
from typing import TypeAlias

from . import _native
from .authoring.builder import ProjectBuilder
from .authoring.values import Color as AuthoringColor
from .authoring.values import BlendMode, DurationMode, Quality, color_to_canonical
from .lowering import LoweringContext
from .audio import AudioTimeline
from .effects import EffectStack
from .masks import MaskCollection
from .flashes import FlashCollection
from .presets import Preset, PresetCollection
from .properties import BindableScalarProperty, ScalarProperty, Transform
from .sources import Color as SourceColor
from .sources import Source, Video
from .transitions import TransitionCollection


JsonValue: TypeAlias = (
    str | int | float | bool | None | list["JsonValue"] | dict[str, "JsonValue"]
)


def _real(value: object, name: str, *, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    number = float(value)
    if number != number or number in (float("inf"), float("-inf")):
        raise ValueError(f"{name} must be finite")
    if positive and number <= 0:
        raise ValueError(f"{name} must be positive")
    if not positive and number < 0:
        raise ValueError(f"{name} must be non-negative")
    return number


def _integer(value: object, name: str, *, positive: bool = False) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{name} must be an integer")
    if positive and value <= 0:
        raise ValueError(f"{name} must be positive")
    return value


def _path(value: str | os.PathLike[str], name: str) -> str:
    try:
        path = os.fspath(value)
    except TypeError as error:
        raise TypeError(f"{name} must be str or PathLike[str]") from error
    if not isinstance(path, str):
        raise TypeError(f"{name} must be str or PathLike[str]")
    return path


def _frame_rate(value: int | tuple[int, int] | _native.FrameRate) -> _native.FrameRate:
    if isinstance(value, _native.FrameRate):
        return value
    if isinstance(value, bool):
        raise TypeError(
            "fps must be a positive integer, (numerator, denominator), or FrameRate"
        )
    if isinstance(value, int):
        _integer(value, "fps", positive=True)
        return _native.FrameRate(value, 1)
    if isinstance(value, tuple) and len(value) == 2:
        numerator = _integer(value[0], "fps numerator", positive=True)
        denominator = _integer(value[1], "fps denominator", positive=True)
        return _native.FrameRate(numerator, denominator)
    raise TypeError(
        "fps must be a positive integer, (numerator, denominator), or FrameRate"
    )


def _backend(value: str | _native.BackendPreference) -> _native.BackendPreference:
    if isinstance(value, _native.BackendPreference):
        return value
    if not isinstance(value, str):
        raise TypeError("backend must be 'auto', 'cpu', 'wgpu', or BackendPreference")
    choices = {
        "auto": _native.BackendPreference.AUTO,
        "cpu": _native.BackendPreference.CPU,
        "wgpu": _native.BackendPreference.WGPU,
    }
    try:
        return choices[value.lower()]
    except KeyError as error:
        raise ValueError("backend must be 'auto', 'cpu', or 'wgpu'") from error


class Layer:
    """A source placement in one composition."""

    __slots__ = (
        "_composition",
        "_source",
        "_id",
        "_name",
        "_start",
        "_duration",
        "_source_start",
        "_playback_rate",
        "_z",
        "_visible",
        "_opacity",
        "_transform",
        "_blend_mode",
        "_effects",
        "_masks",
        "_presets",
    )

    def __init__(
        self,
        composition: Composition,
        source: Source,
        *,
        identifier: str,
        name: str | None,
        start: int | float,
        duration: int | float,
        source_start: int | float = 0.0,
        playback_rate: int | float = 1.0,
        z: int,
        visible: bool,
        opacity: int | float,
        transform: Transform | None = None,
        blend_mode: BlendMode = BlendMode.NORMAL,
    ) -> None:
        self._composition = composition
        self._source = source
        self._id = identifier
        self._name = name
        self._start = _real(start, "start")
        self._duration = _real(duration, "duration", positive=True)
        self._source_start = _real(source_start, "source_start")
        self._playback_rate = _real(playback_rate, "playback_rate", positive=True)
        self._z = _integer(z, "z")
        if not isinstance(visible, bool):
            raise TypeError("visible must be a boolean")
        self._visible = visible
        self._opacity = BindableScalarProperty(
            _opacity(opacity), minimum=0.0, maximum=1.0
        )
        self._transform = transform.copy() if transform is not None else Transform()
        if not isinstance(blend_mode, BlendMode):
            raise TypeError("blend_mode must be BlendMode")
        self._blend_mode = blend_mode
        self._effects = EffectStack("layer")
        self._masks = MaskCollection()
        self._presets = PresetCollection(self)

    @property
    def composition(self) -> Composition:
        return self._composition

    @property
    def source(self) -> Source:
        return self._source

    @property
    def id(self) -> str:
        return self._id

    @property
    def name(self) -> str | None:
        return self._name

    @name.setter
    def name(self, value: str | None) -> None:
        if value is not None and not isinstance(value, str):
            raise TypeError("name must be a string or None")
        self._name = value

    @property
    def start(self) -> float:
        return self._start

    @start.setter
    def start(self, value: int | float) -> None:
        self._start = _real(value, "start")

    @property
    def duration(self) -> float:
        return self._duration

    @duration.setter
    def duration(self, value: int | float) -> None:
        self._duration = _real(value, "duration", positive=True)

    @property
    def source_start(self) -> float:
        return self._source_start

    @source_start.setter
    def source_start(self, value: int | float) -> None:
        self._source_start = _real(value, "source_start")

    @property
    def playback_rate(self) -> float:
        return self._playback_rate

    @playback_rate.setter
    def playback_rate(self, value: int | float) -> None:
        self._playback_rate = _real(value, "playback_rate", positive=True)

    @property
    def z(self) -> int:
        return self._z

    @z.setter
    def z(self, value: int) -> None:
        self._z = _integer(value, "z")

    @property
    def visible(self) -> bool:
        return self._visible

    @visible.setter
    def visible(self, value: bool) -> None:
        if not isinstance(value, bool):
            raise TypeError("visible must be a boolean")
        self._visible = value

    @property
    def opacity(self) -> BindableScalarProperty:
        return self._opacity

    @opacity.setter
    def opacity(self, value: int | float | ScalarProperty) -> None:
        if isinstance(value, ScalarProperty):
            if value.value < 0 or value.value > 1:
                raise ValueError("opacity must be between 0 and 1")
            if any(frame.value < 0 or frame.value > 1 for frame in value.keyframes):
                raise ValueError("opacity keyframes must be between 0 and 1")
            self._opacity.value = value.value
            self._opacity._keyframes = list(value.keyframes)
            self._opacity._bindings = (
                list(value.bindings)
                if isinstance(value, BindableScalarProperty)
                else []
            )
            return
        self._opacity.value = _opacity(value)

    @property
    def transform(self) -> Transform:
        return self._transform

    @transform.setter
    def transform(self, value: Transform) -> None:
        if not isinstance(value, Transform):
            raise TypeError("transform must be Transform")
        copied = value.copy()
        self._transform.position = copied.position
        self._transform.anchor = copied.anchor
        self._transform.scale = copied.scale
        self._transform.rotation_degrees = copied.rotation_degrees

    @property
    def blend_mode(self) -> BlendMode:
        return self._blend_mode

    @blend_mode.setter
    def blend_mode(self, value: BlendMode) -> None:
        if not isinstance(value, BlendMode):
            raise TypeError("blend_mode must be BlendMode")
        self._blend_mode = value

    @property
    def effects(self) -> EffectStack:
        """The ordered visual effects applied after source adaptation."""
        return self._effects

    @property
    def masks(self) -> MaskCollection:
        return self._masks

    @property
    def presets(self) -> PresetCollection:
        """The optional image-only cinematic preset for this layer."""
        return self._presets

    @property
    def preset(self) -> Preset | None:
        """The current cinematic preset, if one is applied."""
        return self._presets.current


def _opacity(value: int | float) -> float:
    result = _real(value, "opacity")
    if result > 1:
        raise ValueError("opacity must be between 0 and 1")
    return result


class _CompositionSource(Source):
    """Private marker used to keep the Layer base class total."""

    __slots__ = ()


class CompositionLayer(Layer):
    """A layer placement that owns a child :class:`Composition`."""

    __slots__ = ("_child",)

    def __init__(
        self,
        composition: Composition,
        *,
        identifier: str,
        name: str | None,
        start: int | float,
        duration: int | float,
        z: int,
        visible: bool,
        opacity: int | float,
        transform: Transform | None = None,
        blend_mode: BlendMode = BlendMode.NORMAL,
    ) -> None:
        super().__init__(
            composition,
            _CompositionSource(),
            identifier=identifier,
            name=name,
            start=start,
            duration=duration,
            z=z,
            visible=visible,
            opacity=opacity,
            transform=transform,
            blend_mode=blend_mode,
        )
        self._child = Composition(composition.project, self)

    @property
    def child(self) -> Composition:
        """The child composition containing this group's authored layers."""
        return self._child

    @property
    def contents(self) -> Composition:
        return self._child

    @property
    def layers(self) -> tuple[Layer, ...]:
        return self._child.layers

    def add(
        self,
        source: Source,
        *,
        start: int | float = 0,
        duration: int | float | None = None,
        source_start: int | float = 0.0,
        playback_rate: int | float = 1.0,
        z: int = 0,
        visible: bool = True,
        opacity: int | float = 1.0,
        id: str | None = None,
        name: str | None = None,
        blend_mode: BlendMode = BlendMode.NORMAL,
    ) -> Layer:
        return self._child.add(
            source,
            start=start,
            duration=duration,
            source_start=source_start,
            playback_rate=playback_rate,
            z=z,
            visible=visible,
            opacity=opacity,
            id=id,
            name=name,
            blend_mode=blend_mode,
        )

    def group(
        self,
        name: str | None = None,
        *,
        start: int | float = 0,
        duration: int | float | None = None,
        z: int = 0,
        visible: bool = True,
        opacity: int | float = 1,
        id: str | None = None,
        blend_mode: BlendMode = BlendMode.NORMAL,
    ) -> "CompositionLayer":
        return self._child.group(
            name,
            start=start,
            duration=duration,
            z=z,
            visible=visible,
            opacity=opacity,
            id=id,
            blend_mode=blend_mode,
        )


class Composition:
    """An ordered collection of layers local to one composition."""

    __slots__ = ("_project", "_layers", "_ids", "_parent_layer", "_transitions")

    def __init__(
        self, project: Project, parent_layer: CompositionLayer | None = None
    ) -> None:
        self._project = project
        self._layers: list[Layer] = []
        self._ids: set[str] = set()
        self._parent_layer = parent_layer
        self._transitions = TransitionCollection(self)

    @property
    def project(self) -> Project:
        return self._project

    @property
    def layers(self) -> tuple[Layer, ...]:
        return tuple(self._layers)

    @property
    def duration(self) -> float | None:
        return (
            self._parent_layer.duration
            if self._parent_layer is not None
            else self._project.duration
        )

    def _default_duration(self) -> float:
        duration = self.duration
        if duration is None:
            raise ValueError(
                "duration is required when the composition has no explicit duration"
            )
        return duration

    @property
    def parent_layer(self) -> CompositionLayer | None:
        return self._parent_layer

    @property
    def transitions(self) -> TransitionCollection:
        """Stable ordered transitions between sibling layers in this composition."""
        return self._transitions

    def add(
        self,
        source: Source,
        *,
        start: int | float = 0,
        duration: int | float | None = None,
        source_start: int | float = 0.0,
        playback_rate: int | float = 1.0,
        z: int = 0,
        visible: bool = True,
        opacity: int | float = 1.0,
        id: str | None = None,
        name: str | None = None,
        blend_mode: BlendMode = BlendMode.NORMAL,
    ) -> Layer:
        if not isinstance(source, Source):
            raise TypeError("source must be a vestra.sources.Source")
        if duration is None:
            if isinstance(source, Video):
                media_path = Path(source.path)
                if not media_path.is_absolute():
                    media_path = self._project._base_directory / media_path
                media_duration = _native.video_duration(media_path)
                available = media_duration - _real(source_start, "source_start")
                if available <= 0.0:
                    raise ValueError("source_start must be before the video duration")
                duration = available / _real(playback_rate, "playback_rate", positive=True)
            else:
                duration = self._default_duration()
        identifier = _layer_id(id, len(self._layers), self._ids)
        if name is not None and not isinstance(name, str):
            raise TypeError("name must be a string or None")
        # Validate every value and copy the source before changing graph state.
        copied = source.snapshot()
        if not isinstance(copied, Source):
            raise TypeError("source.snapshot() must return a Source")
        layer = Layer(
            self,
            copied,
            identifier=identifier,
            name=name,
            start=start,
            duration=duration,
            source_start=source_start,
            playback_rate=playback_rate,
            z=z,
            visible=visible,
            opacity=opacity,
            blend_mode=blend_mode,
        )
        self._ids.add(identifier)
        self._layers.append(layer)
        return layer

    def group(
        self,
        name: str | None = None,
        *,
        start: int | float = 0,
        duration: int | float | None = None,
        z: int = 0,
        visible: bool = True,
        opacity: int | float = 1,
        id: str | None = None,
        blend_mode: BlendMode = BlendMode.NORMAL,
    ) -> CompositionLayer:
        """Create a child composition placed in this composition.

        The placement's timing is local to this parent. Child layers retain
        their own local start and duration when lowered into a GroupClip.
        """
        if name is not None and not isinstance(name, str):
            raise TypeError("name must be a string or None")
        if duration is None:
            duration = self._default_duration()
        identifier = _layer_id(id, len(self._layers), self._ids)
        # Construct and validate the entire placement before touching graph
        # state. The child composition is only reachable through this layer.
        layer = CompositionLayer(
            self,
            identifier=identifier,
            name=name,
            start=start,
            duration=duration,
            z=z,
            visible=visible,
            opacity=opacity,
            blend_mode=blend_mode,
        )
        self._ids.add(identifier)
        self._layers.append(layer)
        return layer


def _layer_id(value: str | None, index: int, used: set[str]) -> str:
    if value is not None:
        if not isinstance(value, str):
            raise TypeError("id must be a string or None")
        if not value or value.isspace():
            raise ValueError("id must not be empty")
        if value in used:
            raise ValueError(f"duplicate layer ID: {value!r}")
        return value
    candidate = f"layer-{index + 1:06d}"
    while candidate in used:
        index += 1
        candidate = f"layer-{index + 1:06d}"
    return candidate


class Project:
    """Mutable normal-user project with one root composition.

    ``output_audio`` is the only project-level configuration with editor-style
    mutation. Size, frame rate, duration, background, quality, base directory,
    name, metadata, and the default output path are construction-time
    configuration; changing them after layers or prepared snapshots exist would
    make their existing timing, validation, or asset-path assumptions unclear.
    Pass new values to a new ``Project`` instead. ``snapshot(output=...)`` is
    the supported per-render output-path override.
    """

    __slots__ = (
        "_width",
        "_height",
        "_frame_rate",
        "_duration",
        "_background",
        "_quality",
        "_base_directory",
        "_name",
        "_metadata",
        "_output_audio",
        "_output_path",
        "_root",
        "_audio",
        "_post_effects",
        "_flashes",
    )

    def __init__(
        self,
        *,
        size: tuple[int, int],
        fps: int | tuple[int, int] | _native.FrameRate,
        duration: int | float | None = None,
        background: AuthoringColor | SourceColor | str = "#000000",
        quality: Quality = Quality.BALANCED,
        base_directory: str | os.PathLike[str] = ".",
        name: str | None = None,
        metadata: JsonValue | None = None,
        output_audio: bool | None = None,
        output_path: str | os.PathLike[str] | None = None,
    ) -> None:
        if not isinstance(size, tuple) or len(size) != 2:
            raise TypeError("size must be a (width, height) tuple")
        self._width = _integer(size[0], "width", positive=True)
        self._height = _integer(size[1], "height", positive=True)
        self._frame_rate = _frame_rate(fps)
        self._duration = (
            None if duration is None else _real(duration, "duration", positive=True)
        )
        self._background = (
            background.value
            if isinstance(background, SourceColor)
            else color_to_canonical(background)
        )
        if not isinstance(quality, Quality):
            raise TypeError("quality must be Quality")
        self._quality = quality
        self._base_directory = Path(_path(base_directory, "base_directory"))
        if name is not None and not isinstance(name, str):
            raise TypeError("name must be a string or None")
        self._name = name
        if not isinstance(metadata, (type(None), str, int, float, bool, list, dict)):
            raise TypeError("metadata must be JSON-compatible")
        self._metadata = metadata
        if output_audio is not None and not isinstance(output_audio, bool):
            raise TypeError("output_audio must be a boolean or None")
        self._output_audio = output_audio
        self._output_path = (
            None if output_path is None else _path(output_path, "output_path")
        )
        self._root = Composition(self)
        self._audio = AudioTimeline(self)
        self._post_effects = EffectStack("post")
        self._flashes = FlashCollection(self)

    @property
    def root(self) -> Composition:
        return self._root

    @property
    def audio(self) -> AudioTimeline:
        """Stable track-oriented audio timeline owned by this project."""
        return self._audio

    @property
    def post_effects(self) -> EffectStack:
        """The ordered visual effects applied to the rendered project output."""
        return self._post_effects

    @property
    def flashes(self) -> FlashCollection:
        """Stable root-only flash overlays."""
        return self._flashes

    @property
    def output_audio(self) -> bool | None:
        """Audio output policy; ``None`` follows whether authored clips exist."""
        return self._output_audio

    @output_audio.setter
    def output_audio(self, value: bool | None) -> None:
        if value is not None and not isinstance(value, bool):
            raise TypeError("output_audio must be a boolean or None")
        self._output_audio = value

    @property
    def duration(self) -> float | None:
        return self._duration

    @property
    def size(self) -> tuple[int, int]:
        return self._width, self._height

    @property
    def fps(self) -> _native.FrameRate:
        return self._frame_rate

    def snapshot(
        self, *, output: str | os.PathLike[str] | None = None
    ) -> _native.Project:
        output_path = self._output_path if output is None else _path(output, "output")
        if output_path is None:
            output_path = "output.mp4"
        has_audio = any(track.clips for track in self._audio.tracks)
        effective_output_audio = (
            has_audio if self._output_audio is None else self._output_audio
        )
        builder = ProjectBuilder(
            width=self._width,
            height=self._height,
            frame_rate=self._frame_rate,
            output_path=output_path,
            duration=self._duration,
            duration_mode=DurationMode.EXPLICIT if self._duration is not None else None,
            background=self._background,
            quality=self._quality,
            base_directory=self._base_directory,
            name=self._name,
            metadata=self._metadata,
            output_audio=effective_output_audio,
        )
        context = LoweringContext(builder)
        context.lower_composition(self._root)
        context.lower_overlays(self._root.transitions, self._flashes)
        context.lower_post_effects(self._post_effects)
        context.lower_audio(self._audio)
        return builder.build()

    def validate(self) -> _native.ValidationReport:
        return _native.Editor().validate(self.snapshot())

    def prepare(
        self, *, backend: str | _native.BackendPreference = "auto"
    ) -> _native.PreparedProject:
        return _native.Editor().prepare(
            self.snapshot(), _native.PrepareOptions(backend=_backend(backend))
        )

    def render_frame(
        self, seconds: float, *, backend: str | _native.BackendPreference = "auto"
    ) -> _native.Frame:
        return self.prepare(backend=backend).render_frame_seconds(seconds)

    def render(
        self,
        output: str | os.PathLike[str],
        *,
        backend: str | _native.BackendPreference = "auto",
        overwrite: bool = False,
        preview: bool = False,
        progress: Callable[[_native.RenderEvent], object] | None = None,
        cancellation: _native.CancellationToken | None = None,
    ) -> _native.RenderResult:
        request = _native.RenderRequest(
            _path(output, "output"),
            backend=_backend(backend),
            overwrite=overwrite,
            preview=preview,
        )
        return _native.Editor().render(
            self.snapshot(output=output),
            request,
            progress=progress,
            cancellation=cancellation,
        )


__all__ = ["Project", "Composition", "CompositionLayer", "Layer", "Source"]
