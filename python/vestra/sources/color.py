"""Solid-colour source descriptors."""

from __future__ import annotations

from ..authoring.values import Color as AuthoringColor
from ..properties.transform import Transform, TransformPoint, TransformScale
from .base import Source

class Color(Source):
    """A solid ``#RRGGBB`` or ``#RRGGBBAA`` source value."""

    __slots__ = ("_value",)

    _value: str

    def __init__(
        self,
        value: str | AuthoringColor,
        *,
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
        canonical = value.to_canonical() if isinstance(value, AuthoringColor) else value
        validated = AuthoringColor(canonical)
        self._value = validated.value

    @property
    def value(self) -> str:
        return self._value

    @value.setter
    def value(self, value: str | AuthoringColor) -> None:
        canonical = value.to_canonical() if isinstance(value, AuthoringColor) else value
        self._value = AuthoringColor(canonical).value

    def to_canonical(self) -> str:
        return self.value


SolidColor = Color


__all__ = ["Color", "SolidColor"]
