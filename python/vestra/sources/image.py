"""Raster-image source descriptors."""

from __future__ import annotations

import os

from ..authoring.values import Crop
from ..properties import CropProperty
from ..properties.transform import Transform, TransformPoint, TransformScale
from .base import Source, Sizing, SizingValue, _sizing

class Image(Source):
    """An image file reference with image-only sizing and crop values."""

    __slots__ = ("_path", "_sizing", "_crop")

    _path: str
    _sizing: Sizing | None
    _crop: CropProperty

    def __init__(
        self,
        path: str | os.PathLike[str],
        *,
        sizing: SizingValue | None = None,
        crop: Crop | CropProperty | None = None,
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
        try:
            value = os.fspath(path)
        except TypeError as error:
            raise TypeError("image path must be str or PathLike[str]") from error
        if not isinstance(value, str):
            raise TypeError("image path must be str or PathLike[str]")
        if not value or value.isspace():
            raise ValueError("image path must not be empty")
        if crop is not None and not isinstance(crop, Crop | CropProperty):
            raise TypeError("crop must be Crop, CropProperty, or None")

        self._path = value
        self._sizing = _sizing(sizing)
        self._crop = (
            CropProperty()
            if crop is None
            else (crop if isinstance(crop, CropProperty) else CropProperty(crop))
        )

    @property
    def path(self) -> str:
        return self._path

    @path.setter
    def path(self, value: str | os.PathLike[str]) -> None:
        try:
            candidate = os.fspath(value)
        except TypeError as error:
            raise TypeError("image path must be str or PathLike[str]") from error
        if not isinstance(candidate, str):
            raise TypeError("image path must be str or PathLike[str]")
        if not candidate or candidate.isspace():
            raise ValueError("image path must not be empty")
        self._path = candidate

    @property
    def sizing(self) -> Sizing | None:
        return self._sizing

    @sizing.setter
    def sizing(self, value: SizingValue | None) -> None:
        self._sizing = _sizing(value)

    @property
    def crop(self) -> CropProperty:
        """Mutable crop property; assigning a :class:`Crop` remains supported."""
        return self._crop

    @crop.setter
    def crop(self, value: Crop | CropProperty | None) -> None:
        if value is None:
            self._crop = CropProperty()
        elif isinstance(value, CropProperty):
            self._crop = value
        elif isinstance(value, Crop):
            self._crop = CropProperty(value)
        else:
            raise TypeError("crop must be Crop, CropProperty, or None")

    def to_canonical(self) -> dict[str, object]:
        return {"type": "image", "asset": self.path}



__all__ = ["Image"]
