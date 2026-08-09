"""Typed Python authoring that compiles to the native project schema."""

from .builder import JsonValue, ProjectBuilder
from .assets import AudioAsset, ImageAsset
from .audio import AudioClip, AudioFadeCurve, AudioGainInterpolation, AudioGainKeyframe, AudioTimeline, AudioTrack
from .signals import ScalarSignal
from .animation import CropKeyframe, PointKeyframe, ScalarKeyframe
from .clips import ImageClip, SolidColorClip
from .presets import Preset, PresetCollection
from .timeline import Timeline
from .errors import AuthoringError
from .effects import (
    ActiveInterval, BrightnessEffect, CameraShakeEffect, ChromaticAberrationEffect,
    ClipEffectCollection, ColorAdjustEffect, ContrastEffect, DirectionalBlurEffect,
    Effect, GaussianBlurEffect, GlowEffect, MotionBlurEffect, PostEffectCollection,
    SaturationEffect, SharpenEffect, TintEffect, VignetteEffect, ZoomBlurDirection,
    ZoomBlurEffect,
)
from .flashes import Flash, FlashCollection
from .transitions import (
    CrossfadeTransition, DirectionalPushTransition, FlashCutTransition,
    Transition, TransitionCollection, ZoomBlurTransition, ZoomCrossfadeTransition,
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
    "AudioClip",
    "AudioFadeCurve",
    "AudioGainInterpolation",
    "AudioGainKeyframe",
    "AudioTimeline",
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
    "Preset",
    "PresetCollection",
    "Quality",
    "ScalarTrack",
    "ScalarSignal",
    "SaturationEffect",
    "SharpenEffect",
    "ScalarKeyframe",
    "Sizing",
    "SolidColorClip",
    "Transform",
    "Timeline",
    "TintEffect",
    "VignetteEffect",
    "ZoomBlurDirection",
    "ZoomBlurEffect",
    "CrossfadeTransition",
    "DirectionalPushTransition",
    "Flash",
    "FlashCollection",
    "FlashCutTransition",
    "Transition",
    "TransitionCollection",
    "ZoomBlurTransition",
    "ZoomCrossfadeTransition",
]
