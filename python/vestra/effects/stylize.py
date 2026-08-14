from __future__ import annotations

from typing import cast

from ..properties import BindableScalarProperty, ScalarProperty
from ..authoring.values import Color
from .base import (
    Effect,
)

class Bloom(Effect):
    __slots__ = ()
    effect_type = "bloom"

    def __init__(
        self,
        threshold: int | float | ScalarProperty,
        radius: int | float | ScalarProperty,
        intensity: int | float | ScalarProperty,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init(
            {"threshold": threshold, "radius": radius, "intensity": intensity}, id=id
        )

    @property
    def threshold(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["threshold"])

    @threshold.setter
    def threshold(self, value: int | float | ScalarProperty) -> None:
        self._set_property("threshold", value)

    @property
    def radius(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["radius"])

    @radius.setter
    def radius(self, value: int | float | ScalarProperty) -> None:
        self._set_property("radius", value)

    @property
    def intensity(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["intensity"])

    @intensity.setter
    def intensity(self, value: int | float | ScalarProperty) -> None:
        self._set_property("intensity", value)


class Glow(Effect):
    __slots__ = ()
    effect_type = "glow"

    def __init__(
        self,
        threshold: int | float | ScalarProperty,
        radius: int | float | ScalarProperty,
        intensity: int | float | ScalarProperty,
        colour: Color | str,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init(
            {
                "threshold": threshold,
                "radius": radius,
                "intensity": intensity,
                "colour": colour,
            },
            id=id,
        )

    @property
    def threshold(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["threshold"])

    @threshold.setter
    def threshold(self, value: int | float | ScalarProperty) -> None:
        self._set_property("threshold", value)

    @property
    def radius(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["radius"])

    @radius.setter
    def radius(self, value: int | float | ScalarProperty) -> None:
        self._set_property("radius", value)

    @property
    def intensity(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["intensity"])

    @intensity.setter
    def intensity(self, value: int | float | ScalarProperty) -> None:
        self._set_property("intensity", value)

    @property
    def colour(self) -> str:
        return cast(str, self._values["colour"])

    @colour.setter
    def colour(self, value: Color | str) -> None:
        self._set_value("colour", value)


class ChromaticAberration(Effect):
    __slots__ = ()
    effect_type = "chromatic_aberration"

    def __init__(
        self,
        amount: int | float | ScalarProperty,
        angle_degrees: int | float | ScalarProperty,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init({"amount": amount, "angle_degrees": angle_degrees}, id=id)

    @property
    def amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["amount"])

    @amount.setter
    def amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("amount", value)

    @property
    def angle_degrees(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["angle_degrees"])

    @angle_degrees.setter
    def angle_degrees(self, value: int | float | ScalarProperty) -> None:
        self._set_property("angle_degrees", value)


class Vignette(Effect):
    __slots__ = ()
    effect_type = "vignette"

    def __init__(
        self,
        amount: int | float | ScalarProperty,
        radius: int | float | ScalarProperty,
        softness: int | float | ScalarProperty,
        colour: Color | str,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init(
            {
                "amount": amount,
                "radius": radius,
                "softness": softness,
                "colour": colour,
            },
            id=id,
        )

    @property
    def amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["amount"])

    @amount.setter
    def amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("amount", value)

    @property
    def radius(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["radius"])

    @radius.setter
    def radius(self, value: int | float | ScalarProperty) -> None:
        self._set_property("radius", value)

    @property
    def softness(self) -> ScalarProperty:
        return self._properties["softness"]

    @softness.setter
    def softness(self, value: int | float | ScalarProperty) -> None:
        self._set_property("softness", value)

    @property
    def colour(self) -> str:
        return cast(str, self._values["colour"])

    @colour.setter
    def colour(self, value: Color | str) -> None:
        self._set_value("colour", value)


class Sharpen(Effect):
    __slots__ = ()
    effect_type = "sharpen"

    def __init__(
        self,
        amount: int | float | ScalarProperty,
        radius: int | float | ScalarProperty,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init({"amount": amount, "radius": radius}, id=id)

    @property
    def amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["amount"])

    @amount.setter
    def amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("amount", value)

    @property
    def radius(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["radius"])

    @radius.setter
    def radius(self, value: int | float | ScalarProperty) -> None:
        self._set_property("radius", value)


class ColorAdjust(Effect):
    __slots__ = ()
    effect_type = "color_adjust"

    def __init__(
        self,
        exposure: int | float | ScalarProperty,
        gamma: int | float | ScalarProperty,
        black_point: int | float | ScalarProperty,
        white_point: int | float | ScalarProperty,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init(
            {
                "exposure": exposure,
                "gamma": gamma,
                "black_point": black_point,
                "white_point": white_point,
            },
            id=id,
        )

    @property
    def exposure(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["exposure"])

    @exposure.setter
    def exposure(self, value: int | float | ScalarProperty) -> None:
        self._set_property("exposure", value)

    @property
    def gamma(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["gamma"])

    @gamma.setter
    def gamma(self, value: int | float | ScalarProperty) -> None:
        self._set_property("gamma", value)

    @property
    def black_point(self) -> ScalarProperty:
        return self._properties["black_point"]

    @black_point.setter
    def black_point(self, value: int | float | ScalarProperty) -> None:
        self._set_property("black_point", value)

    @property
    def white_point(self) -> ScalarProperty:
        return self._properties["white_point"]

    @white_point.setter
    def white_point(self, value: int | float | ScalarProperty) -> None:
        self._set_property("white_point", value)




__all__ = ["Bloom", "ChromaticAberration", "Glow", "Sharpen", "Vignette"]
