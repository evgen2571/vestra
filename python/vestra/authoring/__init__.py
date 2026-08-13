"""Typed Python authoring that compiles to the native project schema."""

from .builder import JsonValue, ProjectBuilder
from .assets import AudioAsset, ImageAsset
from .audio import AudioClip, AudioFadeCurve, AudioGainInterpolation, AudioGainKeyframe, AudioTimeline, AudioTrack
from .audio_effects import AudioEffect, AudioEffectCollection, BassBoostAudioEffect, ParametricEqAudioEffect, PlaybackSpeedAudioEffect, available_audio_effects, audio_effect_definition
from .signals import ScalarSignal
from .animation import CropKeyframe, PointKeyframe, ScalarKeyframe
from .clips import ImageClip, ParticleSystemClip, SolidColorClip, Spectrum2DClip
from .particles import (
    CircleEmitter, ColourLifetimeStop, ParticleAudioReactive, ParticleBlendMode,
    ParticleBurst, ParticleLifetimeStyle, ParticlePrimitive, ParticleSystem, PointEmitter,
    RectangleEmitter, ScalarLifetimeStop, ScalarRange,
)
from .particle_presets import ambient_stars, embers, radial_burst, snow, sparks
from .spectrum2d import (
    Spectrum2DPreset, Spectrum2DGradient, Spectrum2DLinearLayout, Spectrum2DRadialLayout,
)
from .presets import Preset, PresetCollection
from .timeline import Timeline
from .errors import AuthoringError
from .effects import (
    ActiveInterval, BloomEffect, BrightnessEffect, CameraShakeEffect, ChromaticAberrationEffect,
    ClipEffectCollection, ColorAdjustEffect, ContrastEffect, DirectionalBlurEffect,
    Effect, GenericEffect, GaussianBlurEffect, GlowEffect, MotionBlurEffect, PostEffectCollection,
    SaturationEffect, SharpenEffect, TintEffect, VignetteEffect, ZoomBlurDirection,
    ZoomBlurEffect,
    available_effects, effect_definition,
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
    "AudioEffect",
    "AudioEffectCollection",
    "BassBoostAudioEffect",
    "ParametricEqAudioEffect",
    "PlaybackSpeedAudioEffect",
    "BlendMode",
    "BrightnessEffect",
    "BloomEffect",
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
    "GenericEffect",
    "GaussianBlurEffect",
    "GlowEffect",
    "Interpolation",
    "ImageAsset",
    "ImageClip",
    "ParticleSystemClip", "ParticleSystem", "PointEmitter", "RectangleEmitter", "CircleEmitter",
    "ParticleBurst", "ScalarRange", "ParticlePrimitive", "ParticleBlendMode",
    "ParticleLifetimeStyle", "ScalarLifetimeStop", "ColourLifetimeStop", "ParticleAudioReactive",
    "ambient_stars", "snow", "embers", "sparks", "radial_burst",
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
    "Spectrum2DClip",
    "Spectrum2DPreset",
    "Spectrum2DGradient", "Spectrum2DLinearLayout", "Spectrum2DRadialLayout",
    "Transform",
    "Timeline",
    "TintEffect",
    "VignetteEffect",
    "ZoomBlurDirection",
    "ZoomBlurEffect",
    "available_effects",
    "effect_definition",
    "available_audio_effects",
    "audio_effect_definition",
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
