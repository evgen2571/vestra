"""Solid-colour source descriptors."""

from __future__ import annotations

from ..authoring.values import Color as AuthoringColor
from .base import Source

class Color(Source):
    """A solid ``#RRGGBB`` or ``#RRGGBBAA`` source value."""

    __slots__ = ("_value",)

    _value: str

    def __init__(self, value: str | AuthoringColor) -> None:
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
