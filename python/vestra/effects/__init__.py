"""Public visual effect descriptors for the high-level editing API."""

from ..authoring.effects import ActiveInterval, ZoomBlurDirection
from .base import Effect, EffectStack, available_effects
from .blur import DirectionalBlur, GaussianBlur, MotionTile, RadialBlur, ZoomBlur
from .camera import CameraShake
from .color import Brightness, Contrast, Saturation, Tint
from .motion import MotionBlur
from .stylize import Bloom, ChromaticAberration, ColorAdjust, Glow, Sharpen, Vignette

__all__ = [
    "ActiveInterval", "Effect", "EffectStack", "Brightness", "Contrast",
    "Saturation", "Tint", "GaussianBlur", "DirectionalBlur", "MotionTile", "ZoomBlur", "RadialBlur",
    "ZoomBlurDirection", "Glow", "Bloom", "ChromaticAberration", "Vignette",
    "Sharpen", "ColorAdjust", "CameraShake", "MotionBlur", "available_effects",
]

for _name in __all__:
    _value = globals()[_name]
    if isinstance(_value, type) and _value.__module__.startswith(f"{__name__}."):
        _value.__module__ = __name__
