"""Layer and composition ownership for high-level authoring."""
from __future__ import annotations

from pathlib import Path
from . import _native, editor as _editor
from .authoring.values import BlendMode
from .effects import EffectStack
from .masks import MaskCollection, MatteMode
from .presets import Preset, PresetCollection
from .properties import BindableScalarProperty, ScalarProperty, Transform
from .properties.transform import TransformPoint, TransformScale
from .sources import Source, Video
from .transitions import TransitionCollection

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
        "_matte",
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
        self._transform = source._placement_transform(transform=transform)
        if not isinstance(blend_mode, BlendMode):
            raise TypeError("blend_mode must be BlendMode")
        self._blend_mode = blend_mode
        self._effects = EffectStack("layer")
        self._masks = MaskCollection()
        self._matte: tuple[Layer, MatteMode, bool] | None = None
        self._presets = PresetCollection(self)

    def __repr__(self) -> str:
        return (
            f"<{type(self).__name__} {self._id!r} {type(self._source).__name__}, "
            f"start={self._start:g}s, duration={self._duration:g}s, z={self._z}>"
        )

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
        copied.position_x._copy_to(self._transform.position_x)
        copied.position_y._copy_to(self._transform.position_y)
        copied.scale_x._copy_to(self._transform.scale_x)
        copied.scale_y._copy_to(self._transform.scale_y)

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

    def set_matte(
        self, matte: "Layer", *, mode: MatteMode | str = MatteMode.ALPHA, invert: bool = False
    ) -> None:
        if not isinstance(matte, Layer):
            raise TypeError("matte must be a Layer")
        if matte.composition is not self.composition:
            raise ValueError("matte layer must belong to the same composition")
        if matte is self:
            raise ValueError("a layer cannot use itself as a matte")
        try:
            resolved = MatteMode(mode)
        except ValueError as error:
            raise ValueError("matte mode must be 'alpha' or 'luma'") from error
        if not isinstance(invert, bool):
            raise TypeError("invert must be a boolean")
        self._matte = (matte, resolved, invert)

    def clear_matte(self) -> None:
        self._matte = None

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

    def __repr__(self) -> str:
        return (
            f"<CompositionLayer {self.id!r} with "
            f"{_count(len(self._child.layers), 'layer')}, start={self.start:g}s, "
            f"duration={self.duration:g}s, z={self.z}>"
        )

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
        transform: Transform | None = None,
        position: TransformPoint | None = None,
        anchor: TransformPoint | None = None,
        scale: TransformScale | None = None,
        rotation: int | float | None = None,
        rotation_degrees: int | float | None = None,
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
            transform=transform, position=position, anchor=anchor, scale=scale, rotation=rotation, rotation_degrees=rotation_degrees,
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
        self, project: _editor.Project, parent_layer: CompositionLayer | None = None
    ) -> None:
        self._project = project
        self._layers: list[Layer] = []
        self._ids: set[str] = set()
        self._parent_layer = parent_layer
        self._transitions = TransitionCollection(self)

    def __repr__(self) -> str:
        owner = "root" if self._parent_layer is None else repr(self._parent_layer.id)
        return f"<Composition {owner}, {_count(len(self._layers), 'layer')}>"

    @property
    def project(self) -> _editor.Project:
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
        transform: Transform | None = None,
        position: TransformPoint | None = None,
        anchor: TransformPoint | None = None,
        scale: TransformScale | None = None,
        rotation: int | float | None = None,
        rotation_degrees: int | float | None = None,
    ) -> Layer:
        """Place a copy of ``source`` in this composition and return its layer.

        Times are in seconds, local to this composition. ``duration`` defaults
        to the remaining media length for ``Video`` and otherwise to the
        composition's duration. Higher ``z`` values are drawn on top. Later
        changes to ``source`` do not affect the layer; configure the returned
        ``Layer`` instead.
        """
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
            transform=copied._placement_transform(transform=transform, position=position, anchor=anchor, scale=scale, rotation=rotation, rotation_degrees=rotation_degrees),
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


def _count(value: int, noun: str) -> str:
    return f"{value} {noun}" if value == 1 else f"{value} {noun}s"


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


# Keep public type identities and pickle/import paths stable.
Layer.__module__ = "vestra.editor"
CompositionLayer.__module__ = "vestra.editor"
Composition.__module__ = "vestra.editor"
