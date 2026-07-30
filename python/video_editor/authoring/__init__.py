"""Typed Python authoring that compiles to the native project schema."""

from .builder import JsonValue, ProjectBuilder
from .assets import AudioAsset, ImageAsset
from .audio import AudioTrack
from .animation import CropKeyframe, PointKeyframe, ScalarKeyframe
from .clips import ImageClip, SolidColorClip
from .errors import AuthoringError
from .effects import (
    ActiveInterval, BrightnessEffect, CameraShakeEffect, ChromaticAberrationEffect,
    ClipEffectCollection, ColorAdjustEffect, ContrastEffect, DirectionalBlurEffect,
    Effect, GaussianBlurEffect, GlowEffect, MotionBlurEffect, PostEffectCollection,
    SaturationEffect, SharpenEffect, TintEffect, VignetteEffect, ZoomBlurDirection,
    ZoomBlurEffect,
)
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
    "ActiveInterval",
    "AudioAsset",
    "AudioTrack",
    "BlendMode",
    "BrightnessEffect",
    "CameraShakeEffect",
    "ChromaticAberrationEffect",
    "ClipEffectCollection",
    "ColorAdjustEffect",
    "ContrastEffect",
    "Color",
    "Crop",
    "CropKeyframe",
    "CropTrack",
    "CubicBezier",
    "DurationMode",
    "DirectionalBlurEffect",
    "Effect",
    "GaussianBlurEffect",
    "GlowEffect",
    "Interpolation",
    "ImageAsset",
    "ImageClip",
    "JsonValue",
    "Point",
    "PointKeyframe",
    "PointTrack",
    "MotionBlurEffect",
    "PostEffectCollection",
    "ProjectBuilder",
    "Quality",
    "ScalarTrack",
    "SaturationEffect",
    "SharpenEffect",
    "ScalarKeyframe",
    "Sizing",
    "SolidColorClip",
    "Transform",
    "TintEffect",
    "VignetteEffect",
    "ZoomBlurDirection",
    "ZoomBlurEffect",
]
