from __future__ import annotations

from typing import cast

from ..properties import BindableScalarProperty, ScalarProperty
from ..authoring.effects import ActiveInterval
from .base import (
    Effect,
)

class CameraShake(Effect):
    __slots__ = ()
    effect_type = "camera_shake"

    def __init__(
        self,
        position_amount: int | float | ScalarProperty,
        rotation_degrees: int | float | ScalarProperty,
        scale_amount: int | float | ScalarProperty,
        frequency: int | float | ScalarProperty,
        seed: int,
        attack: int | float,
        decay: int | float,
        active_interval: ActiveInterval = ActiveInterval(),
        *,
        id: str | None = None,
    ) -> None:
        super().__init__()
        self._init(
            {
                "active_interval": active_interval,
                "position_amount": position_amount,
                "rotation_degrees": rotation_degrees,
                "scale_amount": scale_amount,
                "frequency": frequency,
                "seed": seed,
                "attack": attack,
                "decay": decay,
            },
            id=id,
        )

    @property
    def active_interval(self) -> ActiveInterval:
        return cast(ActiveInterval, self._values["active_interval"])

    @active_interval.setter
    def active_interval(self, value: ActiveInterval) -> None:
        self._set_value("active_interval", value)

    @property
    def position_amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["position_amount"])

    @position_amount.setter
    def position_amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("position_amount", value)

    @property
    def rotation_degrees(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["rotation_degrees"])

    @rotation_degrees.setter
    def rotation_degrees(self, value: int | float | ScalarProperty) -> None:
        self._set_property("rotation_degrees", value)

    @property
    def scale_amount(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["scale_amount"])

    @scale_amount.setter
    def scale_amount(self, value: int | float | ScalarProperty) -> None:
        self._set_property("scale_amount", value)

    @property
    def frequency(self) -> BindableScalarProperty:
        return cast(BindableScalarProperty, self._properties["frequency"])

    @frequency.setter
    def frequency(self, value: int | float | ScalarProperty) -> None:
        self._set_property("frequency", value)

    @property
    def seed(self) -> int:
        return cast(int, self._values["seed"])

    @seed.setter
    def seed(self, value: int) -> None:
        self._set_value("seed", value)

    @property
    def attack(self) -> float:
        return cast(float, self._values["attack"])

    @attack.setter
    def attack(self, value: int | float) -> None:
        self._set_value("attack", value)

    @property
    def decay(self) -> float:
        return cast(float, self._values["decay"])

    @decay.setter
    def decay(self, value: int | float) -> None:
        self._set_value("decay", value)




__all__ = ["CameraShake"]
