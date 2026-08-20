"""Layer-owned mask authoring handles."""

from __future__ import annotations

from enum import Enum
from typing import cast

from .properties import BindableScalarProperty, Transform
from .properties.lowering import lower_scalar_property, lower_transform
from .sources import Color, Group, Image, ParticleSystem, Shape, Source, Spectrum2D, Text, Video


class MaskOperation(str, Enum):
    REPLACE = "replace"
    INTERSECT = "intersect"
    UNION = "union"
    SUBTRACT = "subtract"


class MaskCoverageMode(str, Enum):
    ALPHA = "alpha"
    LUMA = "luma"


ImageMaskMode = MaskCoverageMode
MaskSource = Shape | Image | Color | Text | Video | Spectrum2D | ParticleSystem | Group


class Mask:
    __slots__ = ("_id", "_input", "_mode", "_operation", "_invert", "_strength", "_feather", "_transform")

    def __init__(self, identifier: str, source: MaskSource, operation: MaskOperation, *, feather: int | float = 0.0, mode: MaskCoverageMode | str | None = None) -> None:
        if not isinstance(identifier, str):
            raise TypeError("mask id must be a string")
        if not identifier or identifier.isspace():
            raise ValueError("mask id must not be empty or whitespace-only")
        if not isinstance(source, (Shape, Image, Color, Text, Video, Spectrum2D, ParticleSystem, Group)):
            raise TypeError("mask input must be a supported owned Source")
        if isinstance(source, Image) and (source.sizing is not None or source.crop.active):
            raise ValueError(
                "Image masks currently use intrinsic image dimensions. "
                "Image sizing/crop settings are not supported in mask context; "
                "use mask.transform to position and scale the mask."
            )
        if isinstance(source, Video) and (source.sizing is not None or source.crop.active):
            raise ValueError(
                "Video masks currently use the source's intrinsic dimensions. "
                "Video sizing/crop settings are not supported in mask context; "
                "use mask.transform to position and scale the mask."
            )
        if isinstance(source, Shape) and mode is not None:
            raise TypeError("mode is only valid for non-Shape source masks")
        if isinstance(source, Source):
            try:
                mode = MaskCoverageMode(MaskCoverageMode.ALPHA if mode is None else mode)
            except ValueError as error:
                raise ValueError("mask coverage mode must be 'alpha' or 'luma'") from error
        if not isinstance(operation, MaskOperation):
            raise TypeError("operation must be MaskOperation")
        self._id = identifier
        self._input = cast(MaskSource, source.snapshot())
        self._mode = mode
        self._operation = operation
        self._invert = False
        self._strength = BindableScalarProperty(1.0, minimum=0.0, maximum=1.0)
        self._feather = BindableScalarProperty(feather, minimum=0.0, maximum=256.0)
        self._transform = Transform()

    @property
    def id(self) -> str: return self._id
    @property
    def input(self) -> MaskSource: return self._input
    @property
    def operation(self) -> MaskOperation: return self._operation
    @operation.setter
    def operation(self, value: MaskOperation) -> None:
        if not isinstance(value, MaskOperation): raise TypeError("operation must be MaskOperation")
        self._operation = value
    @property
    def invert(self) -> bool: return self._invert
    @invert.setter
    def invert(self, value: bool) -> None:
        if not isinstance(value, bool): raise TypeError("invert must be a boolean")
        self._invert = value
    @property
    def strength(self) -> BindableScalarProperty: return self._strength
    @strength.setter
    def strength(self, value: int | float | BindableScalarProperty) -> None:
        if isinstance(value, BindableScalarProperty):
            value._copy_to(self._strength)
        else:
            self._strength.value = value
    @property
    def feather(self) -> BindableScalarProperty: return self._feather
    @feather.setter
    def feather(self, value: int | float | BindableScalarProperty) -> None:
        if isinstance(value, BindableScalarProperty):
            value._copy_to(self._feather)
        else:
            self._feather.value = value
    @property
    def transform(self) -> Transform: return self._transform

    def to_canonical(self, *, asset_id: str | None = None, owner_duration: float | None = None) -> dict[str, object]:
        transform = lower_transform(self.transform)
        if isinstance(self.input, Image):
            if self._mode is None:
                raise ValueError("image mask lowering requires a coverage mode")
            input_value: dict[str, object] = {
                "type": "image",
                "asset": self.input.path if asset_id is None else asset_id,
                "mode": self._mode.value,
            }
        elif isinstance(self.input, Shape):
            input_value = self.input.to_canonical()
        else:
            source_value = (
                self.input.to_canonical(inherited_duration=owner_duration)
                if isinstance(self.input, Group)
                else self.input.to_canonical()
            )
            if isinstance(self.input, Color):
                source_value = {"type": "solid_color", "colour": source_value}
            input_value = {"type": "source", "source": source_value, "mode": self._mode.value}
        return {"id": self.id, "input": input_value, "operation": self.operation.value,
                "invert": self.invert, "strength": lower_scalar_property(self.strength),
                "feather": lower_scalar_property(self.feather), "transform": transform}


class MaskCollection:
    __slots__ = ("_items", "_ids")
    def __init__(self) -> None:
        self._items: list[Mask] = []
        self._ids: set[str] = set()
    @property
    def items(self) -> tuple[Mask, ...]: return tuple(self._items)
    def add(self, source: MaskSource, *, operation: MaskOperation = MaskOperation.INTERSECT,
            id: str | None = None, feather: int | float = 0.0,
            mode: MaskCoverageMode | str | None = None) -> Mask:
        if id is None:
            number = 1
            while f"mask-{number}" in self._ids:
                number += 1
            identifier = f"mask-{number}"
        else:
            identifier = id
        if identifier in self._ids: raise ValueError(f"duplicate mask id: {identifier!r}")
        mask = Mask(identifier, source, operation, feather=feather, mode=mode)
        self._items.append(mask)
        self._ids.add(identifier)
        return mask
    def remove(self, mask: Mask | str) -> None:
        identifier = mask.id if isinstance(mask, Mask) else mask
        for index, item in enumerate(self._items):
            if item.id == identifier:
                self._items.pop(index)
                self._ids.remove(identifier)
                return
        raise KeyError(identifier)
    def clear(self) -> None:
        self._items.clear()
        self._ids.clear()


__all__ = ["ImageMaskMode", "MaskCoverageMode", "Mask", "MaskCollection", "MaskOperation"]
