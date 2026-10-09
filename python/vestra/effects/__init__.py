"""Public visual effect descriptors for the high-level editing API."""

from ..authoring.effects import ActiveInterval, ZoomBlurDirection, PaletteMode, DitherMatrix
from .ascii import Ascii, PseudoAscii, CHARACTER_SETS
from ..authoring.effects import AsciiGlyphStyle, AsciiMode, AsciiColorMode
from .base import Effect, EffectStack, available_effects
from .blur import DirectionalBlur, GaussianBlur, MotionTile, RadialBlur, ZoomBlur
from .camera import CameraShake
from .color import Brightness, Contrast, Saturation, Tint
from .motion import MotionBlur
from .palette import PaletteMap, OrderedDither
from .analog import Halftone, PixelSort, Crt
from ..authoring.effects import HalftoneMode, PixelSortDirection, PixelSortOrder
from .stylize import Bloom, ChromaticAberration, ColorAdjust, Glow, Sharpen, Vignette

__all__ = [
    "Ascii", "PseudoAscii", "CHARACTER_SETS", "AsciiGlyphStyle", "AsciiMode", "AsciiColorMode",
    "ActiveInterval", "Effect", "EffectStack", "Brightness", "Contrast",
    "Saturation", "Tint", "GaussianBlur", "DirectionalBlur", "MotionTile", "ZoomBlur", "RadialBlur",
    "ZoomBlurDirection", "Glow", "Bloom", "ChromaticAberration", "Vignette",
    "Halftone", "PixelSort", "Crt", "HalftoneMode", "PixelSortDirection", "PixelSortOrder",
    "PaletteMap", "OrderedDither", "PaletteMode", "DitherMatrix", "Sharpen", "ColorAdjust", "CameraShake", "MotionBlur", "available_effects",
]

for _name in __all__:
    _value = globals()[_name]
    if isinstance(_value, type) and _value.__module__.startswith(f"{__name__}."):
        _value.__module__ = __name__
