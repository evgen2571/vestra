"""Layer-owned geometric mask authoring handles."""

from __future__ import annotations

from enum import Enum
from typing import cast

from .properties import BindableScalarProperty, Transform
from .sources import Line, Shape


class MaskOperation(str, Enum):
    REPLACE = "replace"
    INTERSECT = "intersect"
    UNION = "union"
    SUBTRACT = "subtract"


class Mask:
    __slots__ = ("_id", "_input", "_operation", "_invert", "_strength", "_feather", "_transform")

    def __init__(self, identifier: str, source: Shape, operation: MaskOperation, *, feather: int | float = 0.0) -> None:
        if not isinstance(identifier, str):
            raise TypeError("mask id must be a string")
        if not identifier or identifier.isspace():
            raise ValueError("mask id must not be empty or whitespace-only")
        if not isinstance(source, Shape):
            raise TypeError("mask input must be a Shape")
        if isinstance(source, Line):
            raise TypeError("Line is not supported as a mask input")
        if not isinstance(operation, MaskOperation):
            raise TypeError("operation must be MaskOperation")
        self._id = identifier
        self._input = cast(Shape, source.snapshot())
        self._operation = operation
        self._invert = False
        self._strength = BindableScalarProperty(1.0, minimum=0.0, maximum=1.0)
        self._feather = BindableScalarProperty(feather, minimum=0.0)
        self._transform = Transform()

    @property
    def id(self) -> str: return self._id
    @property
    def input(self) -> Shape: return self._input
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

    def to_canonical(self) -> dict[str, object]:
        transform = {
            "position": self.transform.position.to_canonical(),
            "anchor": self.transform.anchor.to_canonical(),
            "scale": self.transform.scale.to_canonical(),
            "rotation_degrees": self.transform.rotation_degrees.to_canonical(),
        }
        components = {
            "position_x": self.transform.position_x.bindings,
            "position_y": self.transform.position_y.bindings,
            "scale_x": self.transform.scale_x.bindings,
            "scale_y": self.transform.scale_y.bindings,
        }
        if any(components.values()):
            transform["component_modifiers"] = {
                name: [
                    {"operation": binding.operation, "signal": binding.signal.to_canonical()}
                    for binding in bindings
                ]
                for name, bindings in components.items()
                if bindings
            }
        return {"id": self.id, "input": self.input.to_canonical(), "operation": self.operation.value,
                "invert": self.invert, "strength": self.strength.to_canonical(),
                "feather": self.feather.to_canonical(), "transform": transform}


class MaskCollection:
    __slots__ = ("_items", "_ids")
    def __init__(self) -> None:
        self._items: list[Mask] = []
        self._ids: set[str] = set()
    @property
    def items(self) -> tuple[Mask, ...]: return tuple(self._items)
    def add(self, source: Shape, *, operation: MaskOperation = MaskOperation.INTERSECT,
            id: str | None = None, feather: int | float = 0.0) -> Mask:
        if id is None:
            number = 1
            while f"mask-{number}" in self._ids:
                number += 1
            identifier = f"mask-{number}"
        else:
            identifier = id
        if identifier in self._ids: raise ValueError(f"duplicate mask id: {identifier!r}")
        mask = Mask(identifier, source, operation, feather=feather)
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


__all__ = ["Mask", "MaskCollection", "MaskOperation"]
