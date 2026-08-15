"""Public source descriptors for the high-level editing API."""

from ..authoring.values import Crop, Point, Sizing
from .base import ParticleAudioReactive, Source, SizingValue
from .color import Color, SolidColor
from .image import Image
from .shapes import Circle, Ellipse, Line, Polygon, Rectangle, Shape
from .text import Text
from .particles import (
    CircleEmitter,
    ColourLifetimeStop,
    ParticleBlendMode,
    ParticleBurst,
    ParticleLifetimeStyle,
    ParticlePrimitive,
    ParticleSystem,
    PointEmitter,
    RectangleEmitter,
    ScalarLifetimeStop,
    ScalarRange,
)
from .spectrum import (
    Spectrum2D,
    Spectrum2DGradient,
    Spectrum2DLayout,
    Spectrum2DLinearLayout,
    Spectrum2DPreset,
    Spectrum2DRadialLayout,
)

__all__ = [
    "Image",
    "Shape",
    "Rectangle",
    "Ellipse",
    "Circle",
    "Line",
    "Polygon",
    "Text",
    "Color",
    "SolidColor",
    "Source",
    "SizingValue",
    "Sizing",
    "Crop",
    "Point",
    "ParticleSystem",
    "PointEmitter",
    "RectangleEmitter",
    "CircleEmitter",
    "ParticleBurst",
    "ScalarRange",
    "ParticlePrimitive",
    "ParticleBlendMode",
    "ParticleLifetimeStyle",
    "ScalarLifetimeStop",
    "ColourLifetimeStop",
    "ParticleAudioReactive",
    "Spectrum2D",
    "Spectrum2DPreset",
    "Spectrum2DGradient",
    "Spectrum2DLayout",
    "Spectrum2DLinearLayout",
    "Spectrum2DRadialLayout",
]

# Keep the established public module identity for classes that used to live in
# ``vestra.sources``. This preserves reprs and pickle import paths while the
# implementation is organized below the package façade.
for _name in __all__:
    _value = globals()[_name]
    if isinstance(_value, type) and _value.__module__.startswith(f"{__name__}."):
        _value.__module__ = __name__
