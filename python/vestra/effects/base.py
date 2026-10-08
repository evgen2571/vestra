"""Mutable, builder-independent visual effects for the high-level editor."""

from __future__ import annotations

from enum import Enum
from functools import lru_cache
from types import MappingProxyType
from typing import Iterable, Mapping, Self, TypeVar, cast

from ..authoring.effects import ActiveInterval, ZoomBlurDirection, PaletteMode, DitherMatrix, _palette
from ..authoring.effects import available_effects as _native_effects
from ..authoring.effects import effect_definition
from ..authoring.values import Color, Point, color_to_canonical
from ..properties import (
    BindablePointProperty,
    BindableScalarProperty,
    PointProperty,
    ScalarProperty,
)


def _freeze(value: object) -> object:
    if isinstance(value, dict):
        return MappingProxyType({key: _freeze(item) for key, item in value.items()})
    if isinstance(value, (list, tuple)):
        return tuple(_freeze(item) for item in value)
    return value


@lru_cache(maxsize=1)
def available_effects() -> tuple[Mapping[str, object], ...]:
    """Return the native visual-effect catalog as immutable metadata."""
    return cast(
        tuple[Mapping[str, object], ...],
        tuple(_freeze(item) for item in _native_effects()),
    )


def _descriptor(effect_type: str) -> Mapping[str, object]:
    return effect_definition(effect_type)


def _parameter(effect_type: str, name: str) -> Mapping[str, object]:
    for item in cast(
        tuple[Mapping[str, object], ...], _descriptor(effect_type)["parameters"]
    ):
        if item["name"] == name:
            return item
    raise TypeError(f"unknown parameter {name!r} for effect {effect_type!r}")


def _number(value: object, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    result = float(value)
    if result != result or result in (float("inf"), float("-inf")):
        raise ValueError(f"{name} must be finite")
    return result


def _integer(
    value: object, name: str, *, minimum: int | None = None, maximum: int | None = None
) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{name} must be an integer")
    if (
        minimum is not None
        and value < minimum
        or maximum is not None
        and value > maximum
    ):
        raise ValueError(f"{name} is outside its authored range")
    return value


def _validate_number(parameter: Mapping[str, object], value: object) -> float:
    result = _number(value, str(parameter["name"]))
    minimum = cast(float | None, parameter["minimum"])
    maximum = cast(float | None, parameter["maximum"])
    if minimum is not None and (
        result <= minimum if parameter["minimum_exclusive"] else result < minimum
    ):
        raise ValueError(f"{parameter['name']} is outside its authored range")
    if maximum is not None and (
        result >= maximum if parameter["maximum_exclusive"] else result > maximum
    ):
        raise ValueError(f"{parameter['name']} is outside its authored range")
    return result


def _property(parameter: Mapping[str, object], value: object) -> ScalarProperty:
    minimum = cast(float | None, parameter["minimum"])
    maximum = cast(float | None, parameter["maximum"])
    property_type: type[ScalarProperty] = (
        BindableScalarProperty
        if parameter["kind"] == "scalar_property"
        else ScalarProperty
    )
    result = property_type(
        _validate_number(parameter, value.value if isinstance(value, ScalarProperty) else value),
        minimum=minimum,
        maximum=maximum,
        minimum_exclusive=bool(parameter["minimum_exclusive"]),
        maximum_exclusive=bool(parameter["maximum_exclusive"]),
    )
    if isinstance(value, ScalarProperty):
        _copy_property(value, result)
    return result


def _copy_property(source: ScalarProperty, target: ScalarProperty) -> None:
    target._validate(source.value)
    for frame in source.keyframes:
        target._validate(frame.value)
    source._copy_to(target)
    if isinstance(target, BindableScalarProperty) and not isinstance(
        source, BindableScalarProperty
    ):
        target.clear_bindings()


def _point(value: object, name: str) -> Point:
    if isinstance(value, tuple) and len(value) == 2:
        value = Point(_number(value[0], f"{name}.x"), _number(value[1], f"{name}.y"))
    if not isinstance(value, Point):
        raise TypeError(f"{name} must be Point")
    if not 0 <= value.x <= 1 or not 0 <= value.y <= 1:
        raise ValueError(f"{name} must be within unit space")
    return Point(value.x, value.y)


def _enum(
    value: object, name: str, enum_type: type[Enum], values: tuple[object, ...]
) -> Enum:
    candidate = value.value if isinstance(value, Enum) else value
    if not isinstance(candidate, str):
        raise TypeError(f"{name} must be a string or enum value")
    if candidate not in values:
        raise ValueError(
            f"{name} must be one of: {', '.join(str(item) for item in values)}"
        )
    return enum_type(candidate)


class Effect:
    """A mutable visual-effect descriptor independent of native builder ownership."""

    __slots__ = ("_values", "_properties", "_id")
    effect_type: str

    def __init__(self) -> None:
        self._values: dict[str, object] = {}
        self._properties: dict[str, ScalarProperty | PointProperty] = {}
        self._id: str | None = None

    @property
    def type(self) -> str:
        return self.effect_type

    @property
    def kind(self) -> str:
        return self.effect_type

    @property
    def id(self) -> str | None:
        return self._id

    def _set_id(self, value: str | None) -> None:
        if value is not None and (
            not isinstance(value, str) or not value or value.isspace()
        ):
            raise ValueError("id must be a non-empty string or None")
        self._id = value

    def _set_property(self, name: str, value: object) -> None:
        target = self._properties[name]
        if isinstance(value, ScalarProperty):
            _copy_property(value, target)
        else:
            target.value = cast(float, value)

    def _set_value(self, name: str, value: object) -> None:
        parameter = _parameter(self.effect_type, name)
        kind = parameter["kind"]
        if kind in {"scalar_property", "plain_track"}:
            self._set_property(name, value)
            return
        if kind == "colour":
            self._values[name] = color_to_canonical(cast(Color | str, value))
            return
        if kind == "palette":
            self._values[name] = _palette(parameter, value)
            return
        if kind == "period":
            self._values[name] = None if value is None else _validate_number(parameter, value)
            return
        if kind == "integer":
            self._values[name] = _integer(
                value,
                name,
                minimum=cast(int, parameter["integer_minimum"]),
                maximum=cast(int, parameter["integer_maximum"]),
            )
            return
        if kind == "number":
            self._values[name] = _validate_number(parameter, value)
            return
        if kind in {"point2d", "point_property"}:
            if name in {"tile_center", "center"}:
                target = self._properties[name]
                if isinstance(value, PointProperty):
                    value._copy_to(target)
                else:
                    target.value = _point(value, name)
                return
            self._values[name] = _point(value, name)
            return
        if kind == "boolean":
            if not isinstance(value, bool):
                raise TypeError(f"{name} must be a boolean")
            self._values[name] = value
            return
        if kind == "enum":
            self._values[name] = _enum(
                value,
                name,
                PaletteMode if name == "mode" else DitherMatrix if name == "matrix" else ZoomBlurDirection,
                tuple(cast(tuple[object, ...], parameter["enum_values"])),
            )
            return
        if kind == "active_interval":
            if not isinstance(value, ActiveInterval):
                raise TypeError(f"{name} must be ActiveInterval")
            self._values[name] = value
            return
        raise TypeError(f"unsupported effect parameter kind: {kind!r}")

    def _init(self, values: Mapping[str, object], *, id: str | None = None) -> None:
        # Stage all values before publishing any state, so constructor failures
        # cannot leave a partially initialized descriptor.
        staged_values: dict[str, object] = {}
        staged_properties: dict[str, ScalarProperty | PointProperty] = {}
        parameters = cast(
            tuple[Mapping[str, object], ...],
            _descriptor(self.effect_type)["parameters"],
        )
        definitions = {str(item["name"]): item for item in parameters}
        unknown = set(values) - set(definitions)
        missing = {
            name
            for name, item in definitions.items()
            if item["required"] and name not in values
        }
        if unknown or missing:
            raise TypeError(
                f"invalid parameters for {self.effect_type!r}: unknown={unknown}, missing={missing}"
            )
        for name, parameter in definitions.items():
            if name not in values:
                default = parameter["default"]
                if default is None:
                    if parameter["kind"] == "active_interval":
                        default = ActiveInterval()
                    else:
                        continue
                values = {**values, name: default}
            kind = parameter["kind"]
            if kind in {"scalar_property", "plain_track"}:
                staged_properties[name] = _property(parameter, values[name])
            elif kind in {"point2d", "point_property"} and name in {
                "tile_center",
                "center",
            }:
                staged_properties[name] = BindablePointProperty(_point(values[name], name))
            else:
                # Use a temporary descriptor state for the shared validators.
                self._values = staged_values
                self._properties = staged_properties
                self._set_value(name, values[name])
                staged_values = self._values
                staged_properties = self._properties
        if id is not None and (not isinstance(id, str) or not id or id.isspace()):
            raise ValueError("id must be a non-empty string or None")
        self._values = staged_values
        self._properties = staged_properties
        self._id = id

    def _native_parameters(self) -> dict[str, object]:
        result = dict(self._values)
        for name, property_value in self._properties.items():
            result[name] = property_value.value
        for name, value in list(result.items()):
            if isinstance(value, Enum):
                result[name] = value.value
            elif name == "palette":
                result[name] = list(cast(tuple[str, ...], value))
        return result

    def _property_items(self) -> tuple[tuple[str, ScalarProperty | PointProperty], ...]:
        return tuple(
            (name, value)
            for name, value in self._properties.items()
            if isinstance(value, ScalarProperty)
        )

    def _point_property_items(self) -> tuple[tuple[str, PointProperty], ...]:
        return tuple(
            (name, value)
            for name, value in self._properties.items()
            if isinstance(value, PointProperty)
        )

    def to_canonical(self) -> dict[str, object]:
        from ..properties.lowering import lower_point_property, lower_scalar_property

        data: dict[str, object] = {"type": self.type}
        if self.id is not None:
            data["id"] = self.id
        data.update(self._values)
        for name, property_value in self._properties.items():
            data[name] = (
                lower_point_property(property_value)
                if isinstance(property_value, PointProperty)
                else lower_scalar_property(property_value)
            )
        active = data.pop("active_interval", None)
        if isinstance(active, ActiveInterval):
            data.update(active.to_canonical())
        for name, value in list(data.items()):
            if isinstance(value, Point | Enum):
                data[name] = (
                    value.to_canonical() if isinstance(value, Point) else value.value
                )
            elif name == "palette":
                data[name] = list(cast(tuple[str, ...], value))
            elif name == "period" and value is None:
                data.pop(name)
        return data

    def copy(self) -> Self:
        result = object.__new__(type(self))
        result._values = dict(self._values)
        result._properties = {}
        for name, source in self._properties.items():
            if isinstance(source, PointProperty):
                target = (
                    BindablePointProperty(source.value)
                    if isinstance(source, BindablePointProperty)
                    else PointProperty(source.value)
                )
            else:
                property_type: type[ScalarProperty] = (
                    BindableScalarProperty
                    if isinstance(source, BindableScalarProperty)
                    else ScalarProperty
                )
                target = property_type(
                    source.value,
                    minimum=source._minimum,
                    maximum=source._maximum,
                    minimum_exclusive=source._minimum_exclusive,
                    maximum_exclusive=source._maximum_exclusive,
                )
            source._copy_to(target)
            result._properties[name] = target
        result._id = self._id
        return result


class EffectStack:
    """Ordered high-level effects with scope checks and copy-on-add ownership."""

    __slots__ = ("_scope", "_items")

    def __init__(self, scope: str) -> None:
        if scope not in {"layer", "post"}:
            raise ValueError("scope must be layer or post")
        self._scope = scope
        self._items: list[Effect] = []

    @property
    def items(self) -> tuple[Effect, ...]:
        return tuple(self._items)

    def _validate(self, effect: Effect) -> None:
        if not isinstance(effect, Effect):
            raise TypeError("effect must be a visual Effect")
        if self._scope == "post" and _descriptor(effect.type)["scope"] == "clip_only":
            raise ValueError(f"effect {effect.type!r} is only valid on layers")
        if effect.id is not None and any(item.id == effect.id for item in self._items):
            raise ValueError(f"duplicate effect ID: {effect.id!r}")

    def add(self, effect: "EffectType") -> "EffectType":
        self._validate(effect)
        copied = effect.copy()
        self._items.append(copied)
        return copied

    def extend(self, effects: Iterable[Effect]) -> None:
        staged = tuple(effects)
        for effect in staged:
            self._validate(effect)
        identifiers = [effect.id for effect in staged if effect.id is not None]
        if len(identifiers) != len(set(identifiers)):
            raise ValueError("duplicate effect ID in extension")
        copied = tuple(effect.copy() for effect in staged)
        self._items.extend(copied)


EffectType = TypeVar("EffectType", bound=Effect)



__all__ = ["ActiveInterval", "Effect", "EffectStack", "available_effects"]
