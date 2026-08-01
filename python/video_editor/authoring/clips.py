"""Mutable static visual clips owned by a project builder."""

from ._internal import _Owner, _number
from .assets import ImageAsset
from .effects import ClipEffectCollection
from .tracks import CropTrack, ScalarTrack, Transform
from .presets import PresetCollection
from .values import BlendMode, Color, Crop, Sizing, color_to_canonical


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


class _OpacityTrack(ScalarTrack):
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
    def opacity(self) -> ScalarTrack:
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
