"""Mutable static visual clips owned by a project builder."""

from ._internal import _Owner, _number
from .assets import ImageAsset
from .effects import ClipEffectCollection
from .tracks import CropTrack, ModulatableScalarTrack, Transform
from .presets import PresetCollection
from .spectrum2d import Spectrum2DGradient, Spectrum2DLinearLayout, Spectrum2DLayout, Spectrum2DRadialLayout
from .values import BlendMode, Color, Crop, Sizing, color_to_canonical
from .particles import ParticleSystem


def _timing(value: int | float, name: str, *, positive: bool = False) -> float:
    number = _number(value, name)
    invalid = number <= 0.0 if positive else number < 0.0
    if invalid:
        raise ValueError(f"{name} must be {'positive' if positive else 'non-negative'}")
    return number


def _layer(value: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError("layer must be an integer")
    return value


def _visible(value: bool) -> bool:
    if not isinstance(value, bool):
        raise TypeError("visible must be a boolean")
    return value


class _OpacityTrack(ModulatableScalarTrack):
    def _validate(self, value: float) -> float:
        number = super()._validate(value)
        if not 0.0 <= number <= 1.0:
            raise ValueError("opacity must be between 0 and 1")
        return number


class _Clip:
    __slots__ = ("_owner", "_id", "_start", "_duration", "_layer", "_visible", "_opacity", "_effects", "_blend_mode")

    def _initialize(self, owner: _Owner, identifier: str, *, start: int | float, duration: int | float,
                    layer: int, visible: bool, opacity: int | float) -> None:
        self._owner = owner
        self._id = identifier
        self._start = _timing(start, "start")
        self._duration = _timing(duration, "duration", positive=True)
        self._layer = _layer(layer)
        self._visible = _visible(visible)
        self._opacity = _OpacityTrack._create(owner, opacity)
        self._blend_mode = BlendMode.NORMAL

    def _attach_effects(self, effects: ClipEffectCollection) -> None:
        self._effects = effects

    @property
    def id(self) -> str:
        return self._id

    @property
    def start(self) -> float:
        return self._start

    @start.setter
    def start(self, value: int | float) -> None:
        self._start = _timing(value, "start")

    @property
    def duration(self) -> float:
        return self._duration

    @duration.setter
    def duration(self, value: int | float) -> None:
        self._duration = _timing(value, "duration", positive=True)

    @property
    def layer(self) -> int:
        return self._layer

    @layer.setter
    def layer(self, value: int) -> None:
        self._layer = _layer(value)

    @property
    def visible(self) -> bool:
        return self._visible

    @visible.setter
    def visible(self, value: bool) -> None:
        self._visible = _visible(value)

    @property
    def opacity(self) -> ModulatableScalarTrack:
        return self._opacity

    @property
    def effects(self) -> ClipEffectCollection:
        return self._effects

    @property
    def blend_mode(self) -> BlendMode:
        return self._blend_mode

    @blend_mode.setter
    def blend_mode(self, value: BlendMode) -> None:
        if not isinstance(value, BlendMode):
            raise TypeError("blend_mode must be BlendMode")
        self._blend_mode = value

    def _canonical_common(self) -> dict[str, object]:
        data: dict[str, object] = {"id": self.id, "start": self.start, "duration": self.duration,
                "layer": self.layer, "visible": self.visible,
                "opacity": self.opacity.to_canonical(), "effects": [effect.to_canonical() for effect in self.effects.items]}
        if self.blend_mode is not BlendMode.NORMAL:
            data["blend_mode"] = self.blend_mode.to_canonical()
        return data


class ImageClip(_Clip):
    __slots__ = ("_source", "_sizing", "_transform", "_crop", "_has_crop", "_presets")

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("ImageClip objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls, owner: _Owner, identifier: str, source: ImageAsset, *, start: int | float,
                duration: int | float, layer: int, visible: bool, sizing: Sizing | None,
                crop: Crop | None, opacity: int | float) -> "ImageClip":
        instance = object.__new__(cls)
        instance._initialize_image(owner, identifier, source, start=start, duration=duration, layer=layer,
                                   visible=visible, sizing=sizing, crop=crop, opacity=opacity)
        return instance

    def _initialize_image(self, owner: _Owner, identifier: str, source: ImageAsset, *, start: int | float,
                          duration: int | float, layer: int, visible: bool, sizing: Sizing | None,
                          crop: Crop | None, opacity: int | float) -> None:
        super()._initialize(owner, identifier, start=start, duration=duration, layer=layer,
                            visible=visible, opacity=opacity)
        self._source = source
        self._sizing = self._sizing_value(sizing)
        self._transform = Transform._create(owner)
        self._crop = CropTrack._create(owner, crop if crop is not None else Crop(0, 0, 1, 1))
        self._has_crop = crop is not None
        self._presets = PresetCollection._create(self)

    @property
    def source(self) -> ImageAsset:
        return self._source

    @staticmethod
    def _sizing_value(value: Sizing | None) -> Sizing | None:
        if value is not None and not isinstance(value, Sizing):
            raise TypeError("sizing must be Sizing or None")
        return value

    @property
    def sizing(self) -> Sizing | None:
        return self._sizing

    @sizing.setter
    def sizing(self, value: Sizing | None) -> None:
        self._sizing = self._sizing_value(value)

    @property
    def transform(self) -> Transform:
        return self._transform

    @property
    def crop(self) -> CropTrack:
        return self._crop

    @property
    def has_crop(self) -> bool:
        return self._has_crop

    @property
    def presets(self) -> PresetCollection:
        return self._presets

    def set_crop(self, value: Crop) -> CropTrack:
        self._crop.base_value = value
        self._has_crop = True
        return self._crop

    def clear_crop(self) -> None:
        self._has_crop = False

    def to_canonical(self) -> dict[str, object]:
        data = self._canonical_common()
        data["source"] = {"type": "image", "asset": self.source.id}
        data["transform"] = self.transform.to_canonical()
        if self.sizing is not None:
            data["sizing"] = self.sizing.to_canonical()
        if self.has_crop:
            data["crop"] = self.crop.to_canonical()
        if self.presets.current is not None:
            data["preset"] = self.presets.current.to_canonical()
        return data

    def __repr__(self) -> str:
        return f"ImageClip(id={self.id!r}, source={self.source.id!r})"


class SolidColorClip(_Clip):
    __slots__ = ("_colour",)

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("SolidColorClip objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls, owner: _Owner, identifier: str, colour: Color | str, *, start: int | float,
                duration: int | float, layer: int, visible: bool, opacity: int | float) -> "SolidColorClip":
        instance = object.__new__(cls)
        instance._initialize_solid(owner, identifier, colour, start=start, duration=duration, layer=layer,
                                   visible=visible, opacity=opacity)
        return instance

    def _initialize_solid(self, owner: _Owner, identifier: str, colour: Color | str, *, start: int | float,
                          duration: int | float, layer: int, visible: bool, opacity: int | float) -> None:
        super()._initialize(owner, identifier, start=start, duration=duration, layer=layer,
                            visible=visible, opacity=opacity)
        self._colour = color_to_canonical(colour)

    @property
    def colour(self) -> str:
        return self._colour

    @colour.setter
    def colour(self, value: Color | str) -> None:
        self._colour = color_to_canonical(value)

    def to_canonical(self) -> dict[str, object]:
        data = self._canonical_common()
        data["source"] = {"type": "solid_color", "colour": self.colour}
        return data

    def __repr__(self) -> str:
        return f"SolidColorClip(id={self.id!r}, colour={self.colour!r})"


class ParticleSystemClip(_Clip):
    """A CPU-rendered procedural particle source."""

    __slots__ = ("_particle_system",)
    _particle_system: ParticleSystem

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("ParticleSystemClip objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls, owner: _Owner, identifier: str, particle_system: ParticleSystem, *, start: int | float,
                duration: int | float, layer: int, visible: bool, opacity: int | float) -> "ParticleSystemClip":
        if not isinstance(particle_system, ParticleSystem):
            raise TypeError("particle_system must be ParticleSystem")
        instance = object.__new__(cls)
        instance._initialize(owner, identifier, start=start, duration=duration, layer=layer,
                             visible=visible, opacity=opacity)
        instance._particle_system = particle_system
        return instance

    @property
    def particle_system(self) -> ParticleSystem:
        return self._particle_system

    def to_canonical(self) -> dict[str, object]:
        data = self._canonical_common()
        data["source"] = self.particle_system.to_canonical()
        return data


class Spectrum2DClip(_Clip):
    """An audio-reactive 2D spectrum source driven by authored Master audio.

    It supports linear and radial layouts; the default is linear, bottom,
    forward. Normal visual effects, opacity, and blend modes apply.
    """

    __slots__ = (
        "_band_count", "_min_hz", "_max_hz", "_sensitivity", "_attack_seconds",
        "_release_seconds", "_x", "_y", "_width", "_height", "_bar_gap_ratio", "_colour", "_min_bar_height_ratio", "_layout", "_gradient",
    )

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("Spectrum2DClip objects must be created by ProjectBuilder")

    @classmethod
    def _create(
        cls, owner: _Owner, identifier: str, *, start: int | float, duration: int | float, layer: int,
        visible: bool, opacity: int | float, band_count: int, min_hz: int | float, max_hz: int | float,
        sensitivity: int | float, attack_seconds: int | float, release_seconds: int | float,
        x: int | float, y: int | float, width: int | float, height: int | float,
        bar_gap_ratio: int | float, colour: Color | str,
        min_bar_height_ratio: int | float, layout: object, gradient: object,
    ) -> "Spectrum2DClip":
        instance = object.__new__(cls)
        instance._initialize(owner, identifier, start=start, duration=duration, layer=layer,
                             visible=visible, opacity=opacity)
        instance.band_count = band_count
        instance.min_hz = min_hz
        instance.max_hz = max_hz
        instance.sensitivity = sensitivity
        instance.attack_seconds = attack_seconds
        instance.release_seconds = release_seconds
        instance.x = x
        instance.y = y
        instance.width = width
        instance.height = height
        instance.bar_gap_ratio = bar_gap_ratio
        instance.colour = colour
        instance.min_bar_height_ratio = min_bar_height_ratio
        instance.layout = layout
        if gradient is not None and not isinstance(gradient, Spectrum2DGradient):
            raise TypeError("gradient must be Spectrum2DGradient or None")
        instance.gradient = gradient
        return instance

    @staticmethod
    def _bounded(value: int | float, name: str, *, minimum: float = 0.0, maximum: float = 1.0,
                 minimum_inclusive: bool = True, maximum_inclusive: bool = True) -> float:
        number = _number(value, name)
        lower_ok = number >= minimum if minimum_inclusive else number > minimum
        upper_ok = number <= maximum if maximum_inclusive else number < maximum
        if not lower_ok or not upper_ok:
            raise ValueError(f"{name} is outside its authored range")
        return number

    @property
    def band_count(self) -> int:
        return self._band_count

    @band_count.setter
    def band_count(self, value: int) -> None:
        if isinstance(value, bool) or not isinstance(value, int):
            raise TypeError("band_count must be an integer")
        if not 1 <= value <= 48:
            raise ValueError("band_count must be between 1 and 48")
        self._band_count = value

    @property
    def min_hz(self) -> float:
        return self._min_hz

    @min_hz.setter
    def min_hz(self, value: int | float) -> None:
        self._min_hz = self._bounded(
            value, "min_hz", minimum=0.0, maximum=float("inf"), minimum_inclusive=False,
        )

    @property
    def max_hz(self) -> float:
        return self._max_hz

    @max_hz.setter
    def max_hz(self, value: int | float) -> None:
        self._max_hz = self._bounded(
            value, "max_hz", minimum=0.0, maximum=24_000.0, minimum_inclusive=False,
        )

    @property
    def sensitivity(self) -> float:
        return self._sensitivity

    @sensitivity.setter
    def sensitivity(self, value: int | float) -> None:
        self._sensitivity = self._bounded(value, "sensitivity", minimum=0.0, maximum=float("inf"), maximum_inclusive=True, minimum_inclusive=False)

    @property
    def attack_seconds(self) -> float:
        return self._attack_seconds

    @attack_seconds.setter
    def attack_seconds(self, value: int | float) -> None:
        self._attack_seconds = self._bounded(value, "attack_seconds", maximum=float("inf"))

    @property
    def release_seconds(self) -> float:
        return self._release_seconds

    @release_seconds.setter
    def release_seconds(self, value: int | float) -> None:
        self._release_seconds = self._bounded(value, "release_seconds", maximum=float("inf"))

    @property
    def x(self) -> float:
        return self._x

    @x.setter
    def x(self, value: int | float) -> None:
        self._x = self._bounded(value, "x")

    @property
    def y(self) -> float:
        return self._y

    @y.setter
    def y(self, value: int | float) -> None:
        self._y = self._bounded(value, "y")

    @property
    def width(self) -> float:
        return self._width

    @width.setter
    def width(self, value: int | float) -> None:
        self._width = self._bounded(value, "width", minimum=0.0, minimum_inclusive=False)

    @property
    def height(self) -> float:
        return self._height

    @height.setter
    def height(self, value: int | float) -> None:
        self._height = self._bounded(value, "height", minimum=0.0, minimum_inclusive=False)

    @property
    def bar_gap_ratio(self) -> float:
        return self._bar_gap_ratio

    @bar_gap_ratio.setter
    def bar_gap_ratio(self, value: int | float) -> None:
        self._bar_gap_ratio = self._bounded(value, "bar_gap_ratio", maximum_inclusive=False)

    @property
    def colour(self) -> str:
        return self._colour

    @colour.setter
    def colour(self, value: Color | str) -> None:
        self._colour = color_to_canonical(value)

    @property
    def min_bar_height_ratio(self) -> float:
        return self._min_bar_height_ratio

    @min_bar_height_ratio.setter
    def min_bar_height_ratio(self, value: int | float) -> None:
        self._min_bar_height_ratio = self._bounded(value, "min_bar_height_ratio")

    @property
    def layout(self) -> Spectrum2DLayout:
        return self._layout

    @layout.setter
    def layout(self, value: object) -> None:
        if value is None:
            value = Spectrum2DLinearLayout()
        if not isinstance(value, (Spectrum2DLinearLayout, Spectrum2DRadialLayout)):
            raise TypeError("layout must be Spectrum2DLinearLayout, Spectrum2DRadialLayout, or None")
        self._layout = value

    @property
    def gradient(self) -> Spectrum2DGradient | None:
        return self._gradient

    @gradient.setter
    def gradient(self, value: object) -> None:
        if value is not None and not isinstance(value, Spectrum2DGradient):
            raise TypeError("gradient must be Spectrum2DGradient or None")
        self._gradient = value

    def to_canonical(self) -> dict[str, object]:
        data = self._canonical_common()
        source: dict[str, object] = {
            "type": "spectrum2d", "band_count": self.band_count, "min_hz": self.min_hz,
            "max_hz": self.max_hz, "sensitivity": self.sensitivity,
            "attack_seconds": self.attack_seconds, "release_seconds": self.release_seconds,
            "x": self.x, "y": self.y, "width": self.width, "height": self.height,
            "bar_gap_ratio": self.bar_gap_ratio, "colour": self.colour,
            "min_bar_height_ratio": self.min_bar_height_ratio,
        }
        data["source"] = source
        if not (
            isinstance(self.layout, Spectrum2DLinearLayout)
            and self.layout.anchor == "bottom"
            and self.layout.band_mapping == "forward"
        ):
            source["layout"] = self.layout.to_canonical()
        if self.gradient is not None:
            source["gradient"] = self.gradient.to_canonical()
        return data

    def __repr__(self) -> str:
        return f"Spectrum2DClip(id={self.id!r}, band_count={self.band_count})"
