"""Project snapshots and native execution for the high-level authoring API."""

from __future__ import annotations

import os
from collections.abc import Callable, Sequence
from pathlib import Path
from typing import TYPE_CHECKING, TypeAlias

if TYPE_CHECKING:
    import numpy as np
    from numpy.typing import NDArray

from . import _native
from ._composition import Composition as Composition, CompositionLayer as CompositionLayer, Layer as Layer
from ._composition import _real, _integer, _count
from .authoring.builder import ProjectBuilder
from .authoring.values import Color as AuthoringColor
from .authoring.values import DurationMode, Quality, color_to_canonical
from .lowering import LoweringContext
from .audio import AudioTimeline
from .effects import EffectStack
from .flashes import FlashCollection
from .sources import Source as Source
from .sources import Color as SourceColor


JsonValue: TypeAlias = (
    str | int | float | bool | None | list["JsonValue"] | dict[str, "JsonValue"]
)


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


def _quality_override(value: Quality | None) -> str | None:
    if value is None:
        return None
    if not isinstance(value, Quality):
        raise TypeError("quality must be Quality or None")
    return value.value


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


def _frame_rate_text(rate: _native.FrameRate) -> str:
    return (
        str(rate.numerator)
        if rate.denominator == 1
        else f"{rate.numerator}/{rate.denominator}"
    )


class Project:
    """A video project: output settings, a root composition and an audio timeline.

    ::

        project = Project(size=(1280, 720), fps=30, duration=5)
        project.root.add(Color("#202830"))
        project.render("out.mp4")

    Relative media paths resolve against ``base_directory``. Size, frame rate,
    duration, background, quality and paths are fixed at construction because
    existing layer timing and asset paths depend on them; create a new
    ``Project`` to change them. ``output_audio`` can be changed at any time.
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
        "_resource_limits",
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
        resource_limits: _native.ResourceLimits | None = None,
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
        if resource_limits is not None and not isinstance(resource_limits, _native.ResourceLimits):
            raise TypeError("resource_limits must be ResourceLimits or None")
        self._resource_limits = resource_limits

    def __repr__(self) -> str:
        width, height = self.size
        duration = "auto" if self._duration is None else f"{self._duration:g}s"
        return (
            f"<Project {width}x{height} @ {_frame_rate_text(self._frame_rate)} fps, "
            f"duration={duration}, {_count(len(self._root.layers), 'layer')}>"
        )

    @property
    def root(self) -> Composition:
        """The top-level composition; add layers here."""
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
        """Return an immutable canonical copy of the project.

        Later edits do not affect the snapshot. Use ``to_json()`` on the result
        to obtain the canonical JSON accepted by the ``ve`` CLI.
        """
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
        """Check timing, ranges and references without touching media or GPUs.

        Media files, FFmpeg and graphics adapters are checked by ``prepare()``
        and ``render()``.
        """
        return _native.Editor(resource_limits=self._resource_limits).validate(self.snapshot())

    def prepare(
        self,
        *,
        backend: str | _native.BackendPreference = "auto",
        resolution: tuple[int, int] | None = None,
        fps: int | tuple[int, int] | _native.FrameRate | None = None,
        quality: Quality | None = None,
    ) -> _native.PreparedProject:
        """Decode media and initialize a renderer for repeated frame or video renders."""
        return _native.Editor(resource_limits=self._resource_limits).prepare(
            self.snapshot(),
            _native.PrepareOptions(
                backend=_backend(backend),
                resolution=resolution,
                fps=None if fps is None else _frame_rate(fps),
                quality=_quality_override(quality),
            )
        )

    def render_frame(
        self,
        seconds: float | None = None,
        output: str | os.PathLike[str] | None = None,
        *,
        frame_number: int | None = None,
        backend: str | _native.BackendPreference = "auto",
        resolution: tuple[int, int] | None = None,
        fps: int | tuple[int, int] | _native.FrameRate | None = None,
        overwrite: bool = False,
    ) -> _native.Frame:
        """Render one frame by time or index, optionally saving PNG/JPEG/WebP.

        Times select the containing project frame. Supply exactly one of
        ``seconds`` and ``frame_number``. Use ``prepare()`` or ``render_frames()``
        to share preparation across multiple frames.
        """
        if (seconds is None) == (frame_number is None):
            raise ValueError("supply exactly one of seconds or frame_number")
        prepared = self.prepare(backend=backend, resolution=resolution, fps=fps)
        if seconds is not None:
            frame = prepared.render_frame_seconds(seconds)
        else:
            assert frame_number is not None
            frame = prepared.render_frame_number(frame_number)
        if output is not None:
            frame.save(_path(output, "output"), overwrite=overwrite)
        return frame

    def get_frame(
        self,
        seconds: float | None = None,
        *,
        frame_number: int | None = None,
        backend: str | _native.BackendPreference = "auto",
        resolution: tuple[int, int] | None = None,
        fps: int | tuple[int, int] | _native.FrameRate | None = None,
    ) -> NDArray[np.uint8]:
        """Return an owned writable ``uint8[height, width, 4]`` RGBA array.

        Install ``vestra[numpy]``. Rows run top to bottom, with straight alpha
        and strides ``(width * 4, 4, 1)``. Changing the array cannot change
        another frame or prepared renderer.
        """
        return self.render_frame(
            seconds, frame_number=frame_number, backend=backend, resolution=resolution, fps=fps
        ).to_numpy()

    def render_frames(
        self,
        *,
        directory: str | os.PathLike[str],
        times: Sequence[float] | None = None,
        indices: Sequence[int] | None = None,
        start: float | None = None,
        end: float | None = None,
        format: str = "png",
        backend: str | _native.BackendPreference = "auto",
        resolution: tuple[int, int] | None = None,
        fps: int | tuple[int, int] | _native.FrameRate | None = None,
        overwrite: bool = False,
    ) -> int:
        """Save ``frame_000000.ext``, … and return the number of images.

        Supply ``times``, ``indices``, or a continuous ``[start, end)`` range
        (the whole timeline by default). Selection order and duplicates are
        retained. Range boundaries round up to the next project frame. Frames
        are rendered and saved one at a time using one preparation.
        """
        if times is not None and indices is not None:
            raise ValueError("supply times or indices, not both")
        if (times is not None or indices is not None) and (start is not None or end is not None):
            raise ValueError("start/end cannot be combined with times or indices")
        if not isinstance(format, str) or format.lower() not in {"png", "jpg", "jpeg", "webp"}:
            raise ValueError("format must be png, jpg, jpeg or webp")
        format = format.lower()
        target = Path(_path(directory, "directory"))
        prepared = self.prepare(backend=backend, resolution=resolution, fps=fps)
        report = prepared.preparation_report
        if times is not None:
            for seconds in times:
                if _real(seconds, "time") >= report.duration_ns / 1_000_000_000:
                    raise ValueError("time is outside the project timeline")
            count = len(times)
        elif indices is not None:
            for index in indices:
                if not 0 <= _integer(index, "index") < report.frame_count:
                    raise ValueError("index is outside the project timeline")
            count = len(indices)
        else:
            first, stop = prepared._frame_range_seconds(0 if start is None else start, end)
            indices = range(first, stop)
            count = len(indices)
        if not overwrite:
            for number in range(count):
                path = target / f"frame_{number:06d}.{format}"
                if path.exists():
                    raise FileExistsError(path)
        if count:
            target.mkdir(parents=True, exist_ok=True)
        for number in range(count):
            if times is not None:
                frame = prepared.render_frame_seconds(times[number])
            else:
                assert indices is not None
                frame = prepared.render_frame_number(indices[number])
            frame.save(target / f"frame_{number:06d}.{format}", overwrite=overwrite)
            del frame
        return count

    def render(
        self,
        output: str | os.PathLike[str],
        *,
        backend: str | _native.BackendPreference = "auto",
        overwrite: bool = False,
        preview: bool = False,
        resolution: tuple[int, int] | None = None,
        fps: int | tuple[int, int] | _native.FrameRate | None = None,
        quality: Quality | None = None,
        start: float = 0,
        end: float | None = None,
        show_progress: bool = True,
        on_progress: Callable[[_native.RenderEvent], object] | None = None,
        cancellation: _native.CancellationToken | None = None,
    ) -> _native.RenderResult:
        """Render the project to MP4, GIF or animated WebP and return a ``RenderResult``.

        ``backend`` is ``"auto"`` (WGPU when an adapter is available, otherwise
        CPU), ``"cpu"`` or ``"wgpu"``. An existing ``output`` is replaced only
        with ``overwrite=True``. ``preview=True`` caps the longest side at
        640 px, caps FPS at 15, and selects fast encoding. Explicit ``resolution``,
        ``fps`` and ``quality`` override those defaults without editing the project.
        A progress bar is shown on interactive terminals unless
        ``show_progress=False``; ``on_progress`` receives ``RenderEvent``
        values instead. ``start``/``end`` select project sample times in
        ``[start, end)``; the encoded clip and trimmed audio start at zero.
        An omitted end uses the project duration. FFmpeg is installed automatically; set
        ``IMAGEIO_FFMPEG_EXE`` to select a different executable.
        """
        request = _native.RenderRequest(
            _path(output, "output"),
            backend=_backend(backend),
            overwrite=overwrite,
            preview=preview,
            resolution=resolution,
            fps=None if fps is None else _frame_rate(fps),
            quality=_quality_override(quality),
            start=start,
            end=end,
        )
        return _native.Editor(resource_limits=self._resource_limits).render(
            self.snapshot(),
            request,
            show_progress=show_progress,
            on_progress=on_progress,
            cancellation=cancellation,
        )

    def render_preview(
        self,
        output: str | os.PathLike[str],
        *,
        backend: str | _native.BackendPreference = "auto",
        overwrite: bool = False,
        resolution: tuple[int, int] | None = None,
        fps: int | tuple[int, int] | _native.FrameRate | None = None,
        quality: Quality | None = None,
        start: float = 0,
        end: float | None = None,
        show_progress: bool = True,
        on_progress: Callable[[_native.RenderEvent], object] | None = None,
        cancellation: _native.CancellationToken | None = None,
    ) -> _native.RenderResult:
        """Render with the 640 px, at most 15 FPS, fast-encoding preset."""
        return self.render(
            output,
            backend=backend,
            overwrite=overwrite,
            preview=True,
            resolution=resolution,
            fps=fps,
            quality=quality,
            start=start,
            end=end,
            show_progress=show_progress,
            on_progress=on_progress,
            cancellation=cancellation,
        )


__all__ = ["Project", "Composition", "CompositionLayer", "Layer", "Source"]
