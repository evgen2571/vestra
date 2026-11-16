"""Canonical lowering for builder-independent presentation properties."""

from __future__ import annotations

from typing import Any

from .base import ScalarBindingTarget, SignalBinding
from .point import BindablePointProperty, PointProperty
from .scalar import BindableScalarProperty, ScalarProperty
from .transform import CropProperty, Transform


def lower_scalar_property(source: ScalarProperty) -> dict[str, object]:
    """Lower a high-level scalar property to the canonical track shape."""
    data = source.to_canonical()
    if isinstance(source, BindableScalarProperty) and "bindings" in data:
        data["modifiers"] = data.pop("bindings")
    return data


def lower_point_property(source: PointProperty) -> dict[str, object]:
    """Lower a high-level point property and preserve its authored animation."""
    data = source.to_canonical()
    if isinstance(source, BindablePointProperty) and "bindings" in data:
        data["modifiers"] = data.pop("bindings")
    return data


def _canonical_bindings(bindings: tuple[SignalBinding, ...]) -> list[dict[str, object]]:
    return [{"operation": binding.operation, "signal": binding.signal.to_canonical()}
            for binding in bindings]


def _transform_properties(source: Transform) -> tuple[tuple[str, ScalarProperty | PointProperty], ...]:
    return (("position", source.position), ("anchor", source.anchor),
            ("scale", source.scale), ("rotation_degrees", source.rotation_degrees))


def _transform_component_bindings(source: Transform) -> dict[str, tuple[SignalBinding, ...]]:
    # Uniform scale precedes point components, which precede explicit components.
    return {
        "position_x": source.position_x.bindings,
        "position_y": source.position_y.bindings,
        "scale_x": source.scale.bindings + source.scale.x.bindings + source.scale_x.bindings,
        "scale_y": source.scale.bindings + source.scale.y.bindings + source.scale_y.bindings,
    }


def lower_transform(source: Transform) -> dict[str, object]:
    """Lower presentation using the same property and binding order as builder tracks."""
    result: dict[str, object] = {
        name: lower_scalar_property(prop) if isinstance(prop, ScalarProperty) else lower_point_property(prop)
        for name, prop in _transform_properties(source)
    }
    scale = result["scale"]
    assert isinstance(scale, dict)
    scale.pop("modifiers", None)
    scale.pop("component_modifiers", None)
    components = {name: _canonical_bindings(bindings)
                  for name, bindings in _transform_component_bindings(source).items() if bindings}
    if components:
        result["component_modifiers"] = components
    return result


def _copy_property(source: ScalarProperty | PointProperty | CropProperty, target: Any) -> None:
    target.base_value = source.value
    target.clear_keyframes()
    for frame in source.keyframes:
        target.keyframe(time=frame.time, value=frame.value, interpolation=frame.interpolation)


def apply_property(source: ScalarProperty | PointProperty | CropProperty, target: Any) -> None:
    """Adapt typed values/animation and ordered bindings to an owner-bound track."""
    _copy_property(source, target)
    if isinstance(source, BindableScalarProperty | BindablePointProperty):
        bind = target.modulate if isinstance(source, BindableScalarProperty) else target.react_to
        for binding in source.bindings:
            bind(binding.signal, mode=binding.operation)


def apply_component_bindings(source: ScalarBindingTarget, target: Any) -> None:
    for binding in source.bindings:
        target.modulate(binding.signal, mode=binding.operation)


def apply_transform(source: Transform, target: Any) -> None:
    """Adapt the shared presentation rules to the builder's transform handles."""
    for name, prop in _transform_properties(source):
        if name == "scale":
            _copy_property(prop, target.scale)
        else:
            apply_property(prop, getattr(target, name))
    for name, bindings in _transform_component_bindings(source).items():
        for binding in bindings:
            getattr(target, name).modulate(binding.signal, mode=binding.operation)


def lower_effect(source: Any) -> dict[str, object]:
    """Effect.to_canonical already lowers every animated property."""
    return source.to_canonical()


__all__ = [
    "lower_effect",
    "lower_point_property",
    "lower_scalar_property",
    "lower_transform",
]
