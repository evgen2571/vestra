"""Video source descriptors."""

from __future__ import annotations

import os

from ..authoring.values import Crop
from ..properties import CropProperty
from .base import Source, Sizing, SizingValue, _sizing


class Video(Source):
    """A visual video asset with Image-compatible sizing and crop."""

    __slots__ = ("_path", "_sizing", "_crop")

    def __init__(self, path: str | os.PathLike[str], *, sizing: SizingValue | None = None,
                 crop: Crop | CropProperty | None = None) -> None:
        try:
            value = os.fspath(path)
        except TypeError as error:
            raise TypeError("video path must be str or PathLike[str]") from error
        if not isinstance(value, str) or not value or value.isspace():
            raise ValueError("video path must be a non-empty string")
        if crop is not None and not isinstance(crop, Crop | CropProperty):
            raise TypeError("crop must be Crop, CropProperty, or None")
        self._path = value
        self._sizing = _sizing(sizing)
        self._crop = CropProperty() if crop is None else (
            crop if isinstance(crop, CropProperty) else CropProperty(crop)
        )

    @property
    def path(self) -> str:
        return self._path

    @property
    def sizing(self) -> Sizing | None:
        return self._sizing

    @sizing.setter
    def sizing(self, value: SizingValue | None) -> None:
        self._sizing = _sizing(value)

    @property
    def crop(self) -> CropProperty:
        return self._crop

    def to_canonical(self) -> dict[str, object]:
        return {"type": "video", "asset": self.path}


__all__ = ["Video"]
