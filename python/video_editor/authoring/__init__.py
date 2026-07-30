"""Typed Python authoring that compiles to the native project schema."""

from .builder import JsonValue, ProjectBuilder
from .assets import AudioAsset, ImageAsset
from .audio import AudioTrack
from .animation import CropKeyframe, PointKeyframe, ScalarKeyframe
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
    "CropKeyframe",
    "CropTrack",
    "CubicBezier",
    "DurationMode",
    "Interpolation",
    "ImageAsset",
    "ImageClip",
    "JsonValue",
    "Point",
    "PointKeyframe",
    "PointTrack",
    "ProjectBuilder",
    "Quality",
    "ScalarTrack",
    "ScalarKeyframe",
    "Sizing",
    "SolidColorClip",
    "Transform",
]
