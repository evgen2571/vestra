"""Source values for the high-level editor API.

The classes in this module are intentionally small authoring handles.  They
hold source-specific values, while ``vestra.authoring`` remains the canonical
implementation used during lowering.
"""

from __future__ import annotations

from copy import deepcopy
from typing import TYPE_CHECKING, Literal, TypeAlias, cast

from ..authoring.particles import (
    ParticleAudioReactive as AuthoringParticleAudioReactive,
)
from ..authoring.tracks import ModulatableScalarTrack
from ..authoring.values import (
    Sizing,
)
from ..properties import BindableScalarProperty, ScalarProperty, SignalOperation
from ..authoring.signals import ScalarSignal

if TYPE_CHECKING:
    pass


def _copy_bindable_property(value: ScalarProperty) -> BindableScalarProperty:
    copied = BindableScalarProperty(value.value)
    for frame in value.keyframes:
        copied.keyframe(frame.time, frame.value, interpolation=frame.interpolation)
    if isinstance(value, BindableScalarProperty):
        for binding in value.bindings:
            copied.bind(binding.signal, operation=binding.operation)
    return copied


def _authoring_property(value: object) -> BindableScalarProperty:
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return BindableScalarProperty(value)
    if isinstance(value, ScalarProperty):
        return _copy_bindable_property(value)
    if isinstance(value, ModulatableScalarTrack):
        copied = BindableScalarProperty(value.base_value)
        for frame in value.keyframes:
            copied.keyframe(frame.time, frame.value, interpolation=frame.interpolation)
        for modifier in value._modifiers:
            copied.bind(
                cast(ScalarSignal, modifier["signal"]),
                operation=cast(SignalOperation, modifier["operation"]),
            )
        return copied
    raise TypeError(
        "particle audio-reactive values must be scalar properties, numbers, or None"
    )


ReactiveProperty: TypeAlias = ScalarProperty | ModulatableScalarTrack | int | float


class ParticleAudioReactive:
    """Builder-independent particle appearance bindings."""

    __slots__ = ("_size", "_opacity", "_intensity")

    def __init__(
        self,
        *,
        size: ReactiveProperty | None = None,
        opacity: ReactiveProperty | None = None,
        intensity: ReactiveProperty | None = None,
    ) -> None:
        self._size = None if size is None else _authoring_property(size)
        self._opacity = None if opacity is None else _authoring_property(opacity)
        self._intensity = None if intensity is None else _authoring_property(intensity)

    @property
    def size(self) -> BindableScalarProperty | None:
        return self._size

    @property
    def opacity(self) -> BindableScalarProperty | None:
        return self._opacity

    @property
    def intensity(self) -> BindableScalarProperty | None:
        return self._intensity

    @classmethod
    def from_authoring(
        cls, value: AuthoringParticleAudioReactive
    ) -> "ParticleAudioReactive":
        return cls(size=value.size, opacity=value.opacity, intensity=value.intensity)

    def to_canonical(self) -> dict[str, object]:
        result: dict[str, object] = {}
        for name in ("size", "opacity", "intensity"):
            value = getattr(self, name)
            if value is not None:
                canonical = value.to_canonical()
                if "bindings" in canonical:
                    canonical["modifiers"] = canonical.pop("bindings")
                result[name] = canonical
        return result

    def copy(self) -> "ParticleAudioReactive":
        return ParticleAudioReactive(
            size=None if self.size is None else _copy_bindable_property(self.size),
            opacity=None
            if self.opacity is None
            else _copy_bindable_property(self.opacity),
            intensity=None
            if self.intensity is None
            else _copy_bindable_property(self.intensity),
        )


class Source:
    """Extensible source value base class.

    Source values are copied when placed in a composition. Subclasses may
    override ``snapshot`` when a normal deep copy is not appropriate.
    """

    __slots__ = ()

    def snapshot(self) -> "Source":
        return deepcopy(self)


SizingValue: TypeAlias = Sizing | Literal["original", "fit", "cover"]


def _sizing(value: SizingValue | None) -> Sizing | None:
    if value is None:
        return None
    if isinstance(value, Sizing):
        return value
    if isinstance(value, str) and value in {"original", "fit", "cover"}:
        return Sizing(value)
    raise TypeError("sizing must be Sizing, 'original', 'fit', 'cover', or None")


__all__ = ["Source", "SizingValue", "Sizing", "ParticleAudioReactive"]
