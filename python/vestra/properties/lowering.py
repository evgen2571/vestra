"""Canonical lowering for builder-independent presentation properties."""

from __future__ import annotations

from typing import Any

from .base import ScalarBindingTarget
from .point import BindablePointProperty, PointProperty
from .scalar import BindableScalarProperty, ScalarProperty
from .transform import Transform


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


def _lower_component_bindings(source: ScalarBindingTarget) -> list[dict[str, object]]:
    return [
        {"operation": binding.operation, "signal": binding.signal.to_canonical()}
        for binding in source.bindings
    ]


def lower_transform(source: Transform) -> dict[str, object]:
    """Lower a high-level transform using normal Clip component semantics."""
    scale = lower_point_property(source.scale)
    uniform = _lower_component_bindings(source.scale)
    scale.pop("modifiers", None)
    components = {
        "position_x": _lower_component_bindings(source.position_x),
        "position_y": _lower_component_bindings(source.position_y),
        "scale_x": uniform + _lower_component_bindings(source.scale_x),
        "scale_y": uniform + _lower_component_bindings(source.scale_y),
    }
    result: dict[str, object] = {
        "position": lower_point_property(source.position),
        "anchor": lower_point_property(source.anchor),
        "scale": scale,
        "rotation_degrees": lower_scalar_property(source.rotation_degrees),
    }
    if any(components.values()):
        result["component_modifiers"] = {
            name: values for name, values in components.items() if values
        }
    return result


def lower_effect(source: Any) -> dict[str, object]:
    """Lower a high-level effect, replacing property bindings with modifiers."""
    data = source.to_canonical()
    for name, property_value in source._property_items():
        data[name] = lower_scalar_property(property_value)
    return data


__all__ = [
    "lower_effect",
    "lower_point_property",
    "lower_scalar_property",
    "lower_transform",
]
