"""Typed, animatable property descriptors for the editing API."""

from ..authoring.values import BlendMode, Crop, CubicBezier, Interpolation, Point
from .base import PropertyKeyframe, ScalarBindingTarget, SignalBinding, SignalOperation
from .point import BindablePointProperty, PointProperty
from .scalar import BindableScalarProperty, ScalarProperty
from .transform import CropProperty, Transform

__all__ = [
    "ScalarProperty", "PointProperty", "CropProperty", "PropertyKeyframe",
    "Transform", "BindableScalarProperty", "BindablePointProperty",
    "ScalarBindingTarget", "SignalBinding", "SignalOperation", "Interpolation",
    "CubicBezier", "Point", "Crop", "BlendMode",
]

for _name in __all__:
    _value = globals()[_name]
    if isinstance(_value, type) and _value.__module__.startswith(f"{__name__}."):
        _value.__module__ = __name__
