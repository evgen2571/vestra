"""Mutable static visual clips owned by a project builder."""

from ._internal import _Owner
from .assets import ImageAsset
from .tracks import CropTrack, ScalarTrack, Transform, _number
from .values import Color, Crop, Sizing, color_to_canonical


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
    def __init__(self, owner: _Owner, identifier: str, *, start: int | float, duration: int | float,
                 layer: int, visible: bool, opacity: int | float) -> None:
        self._owner = owner
        self._id = identifier
        self._start = _timing(start, "start")
        self._duration = _timing(duration, "duration", positive=True)
        self._layer = _layer(layer)
        self._visible = _visible(visible)
        self.opacity = _OpacityTrack(owner, opacity)

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

    def _canonical_common(self) -> dict[str, object]:
        return {"id": self.id, "start": self.start, "duration": self.duration,
                "layer": self.layer, "visible": self.visible,
                "opacity": self.opacity.to_canonical()}


class ImageClip(_Clip):
    def __init__(self, owner: _Owner, identifier: str, source: ImageAsset, *, start: int | float,
                 duration: int | float, layer: int, visible: bool, sizing: Sizing | None,
                 crop: Crop | None, opacity: int | float) -> None:
        super().__init__(owner, identifier, start=start, duration=duration, layer=layer,
                         visible=visible, opacity=opacity)
        self._source = source
        self._sizing = self._sizing_value(sizing)
        self.transform = Transform(owner)
        self._crop = CropTrack(owner, crop) if crop is not None else None

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
    def crop(self) -> CropTrack | None:
        return self._crop

    def set_crop(self, value: Crop) -> CropTrack:
        self._crop = CropTrack(self._owner, value)
        return self._crop

    def clear_crop(self) -> None:
        self._crop = None

    def to_canonical(self) -> dict[str, object]:
        data = self._canonical_common()
        data["source"] = {"type": "image", "asset": self.source.id}
        data["transform"] = self.transform.to_canonical()
        if self.sizing is not None:
            data["sizing"] = self.sizing.to_canonical()
        if self.crop is not None:
            data["crop"] = self.crop.to_canonical()
        return data


class SolidColorClip(_Clip):
    def __init__(self, owner: _Owner, identifier: str, colour: Color | str, *, start: int | float,
                 duration: int | float, layer: int, visible: bool, opacity: int | float) -> None:
        super().__init__(owner, identifier, start=start, duration=duration, layer=layer,
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
