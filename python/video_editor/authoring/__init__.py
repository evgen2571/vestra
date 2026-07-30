"""Typed Python authoring that compiles to the native project schema."""

from .builder import JsonValue, ProjectBuilder
from .errors import AuthoringError
from .values import (
    BlendMode,
    Color,
    Crop,
    CubicBezier,
    DurationMode,
    Interpolation,
    Point,
    Quality,
    Sizing,
)

__all__ = [
    "AuthoringError",
    "BlendMode",
    "Color",
    "Crop",
    "CubicBezier",
    "DurationMode",
    "Interpolation",
    "JsonValue",
    "Point",
    "ProjectBuilder",
    "Quality",
    "Sizing",
]
