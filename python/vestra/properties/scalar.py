"""Scalar properties and signal bindings."""

from __future__ import annotations

from typing import Self

from ..authoring.signals import ScalarSignal
from .base import (
    SignalBinding,
    SignalOperation,
    _Property,
    _number,
)

class ScalarProperty(_Property[float]):
    """A finite scalar value with optional scalar keyframes."""

    __slots__ = ("_minimum", "_maximum", "_minimum_exclusive", "_maximum_exclusive")

    def __init__(
        self,
        value: int | float = 0.0,
        *,
        minimum: float | None = None,
        maximum: float | None = None,
        minimum_exclusive: bool = False,
        maximum_exclusive: bool = False,
    ) -> None:
        self._minimum = minimum
        self._maximum = maximum
        self._minimum_exclusive = minimum_exclusive
        self._maximum_exclusive = maximum_exclusive
        super().__init__(value)

    def _validate(self, value: float | int) -> float:
        result = _number(value, "value")
        if self._minimum is not None and (
            result <= self._minimum
            if self._minimum_exclusive
            else result < self._minimum
        ):
            raise ValueError("value is below its minimum")
        if self._maximum is not None and (
            result >= self._maximum
            if self._maximum_exclusive
            else result > self._maximum
        ):
            raise ValueError("value is above its maximum")
        return result


class BindableScalarProperty(ScalarProperty):
    """A scalar property that accepts ordered immutable audio bindings."""

    __slots__ = ("_bindings",)

    def __init__(
        self,
        value: int | float = 0.0,
        *,
        minimum: float | None = None,
        maximum: float | None = None,
        minimum_exclusive: bool = False,
        maximum_exclusive: bool = False,
    ) -> None:
        self._bindings: list[SignalBinding] = []
        super().__init__(
            value,
            minimum=minimum,
            maximum=maximum,
            minimum_exclusive=minimum_exclusive,
            maximum_exclusive=maximum_exclusive,
        )

    @property
    def bindings(self) -> tuple[SignalBinding, ...]:
        return tuple(self._bindings)

    def bind(self, signal: ScalarSignal, *, operation: SignalOperation = "add") -> Self:
        binding = SignalBinding(signal, operation)
        self._bindings.append(binding)
        return self

    def clear_bindings(self) -> None:
        self._bindings.clear()

    def to_canonical(self) -> dict[str, object]:
        data = super().to_canonical()
        if self._bindings:
            data["bindings"] = [
                {"operation": item.operation, "signal": item.signal.to_canonical()}
                for item in self._bindings
            ]
        return data

    def _copy_to(self, other: "_Property[float]") -> None:
        super()._copy_to(other)
        if isinstance(other, BindableScalarProperty):
            other._bindings = list(self._bindings)




__all__ = ["ScalarProperty", "BindableScalarProperty"]
