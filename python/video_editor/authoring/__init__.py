"""Typed Python authoring that compiles to the native project schema."""

from .builder import JsonValue, ProjectBuilder
from .assets import AudioAsset, ImageAsset
from .audio import AudioTrack
from .clips import ImageClip, SolidColorClip
from .errors import AuthoringError
from .tracks import CropTrack, PointTrack, ScalarTrack, Transform
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
    "AudioAsset",
    "AudioTrack",
    "BlendMode",
    "Color",
    "Crop",
    "CropTrack",
    "CubicBezier",
    "DurationMode",
    "Interpolation",
    "ImageAsset",
    "ImageClip",
    "JsonValue",
    "Point",
    "PointTrack",
    "ProjectBuilder",
    "Quality",
    "ScalarTrack",
    "Sizing",
    "SolidColorClip",
    "Transform",
]
