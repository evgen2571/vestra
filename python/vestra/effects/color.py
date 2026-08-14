from __future__ import annotations

from typing import cast

from ..properties import BindableScalarProperty, ScalarProperty
from ..authoring.values import Color
from .base import (
    Effect,
)

class _AmountEffect(Effect):
    __slots__ = ()

    def __init__(
        self, amount: int | float | ScalarProperty, *, id: str | None = None
    ) -> None:
        super().__init__()
        self._init({"amount": amount}, id=id)

    @property
    def amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["amount"])

    @amount.setter
    def amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("amount", value)


class Brightness(_AmountEffect):
    effect_type = "brightness"


class Contrast(_AmountEffect):
    effect_type = "contrast"


class Saturation(_AmountEffect):
    effect_type = "saturation"


class Tint(Effect):
    __slots__ = ()
    effect_type = "tint"

    def __init__(
        self,
        colour: Color | str,
        amount: int | float | ScalarProperty,
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init({"colour": colour, "amount": amount}, id=id)

    @property
    def colour(self) -> str:
        return cast(str, self._values["colour"])

    @colour.setter
    def colour(self, value: Color | str) -> None:
        self._set_value("colour", value)

    @property
    def amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["amount"])

    @amount.setter
    def amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("amount", value)




__all__ = ["Brightness", "Contrast", "Saturation", "Tint"]
