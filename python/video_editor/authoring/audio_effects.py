"""Descriptor-driven audio-effect authoring."""

from __future__ import annotations

from types import MappingProxyType
from typing import Mapping, cast

from ._internal import _IdAllocator, _Owner, _number
from video_editor._native import audio_effect_definitions as _native_audio_effect_definitions


def _freeze(value: object) -> object:
    if isinstance(value, dict):
        return MappingProxyType({key: _freeze(item) for key, item in value.items()})
    if isinstance(value, list):
        return tuple(_freeze(item) for item in value)
    return value


def available_audio_effects() -> tuple[Mapping[str, object], ...]:
    return tuple(cast(Mapping[str, object], _freeze(item)) for item in _native_audio_effect_definitions())


def audio_effect_definition(effect_type: str) -> Mapping[str, object]:
    for definition in available_audio_effects():
        if definition["id"] == effect_type:
            return definition
    raise ValueError(f"unknown audio effect type: {effect_type!r}")


class AudioEffect:
    __slots__ = ("_owner", "_id", "_type", "_data")

    @classmethod
    def _create(cls, owner: _Owner, identifier: str, effect_type: str, data: Mapping[str, object]) -> "AudioEffect":
        item = object.__new__(cls)
        item._owner, item._id, item._type = owner, identifier, effect_type
        item._data = {"id": identifier, "type": effect_type, **data}
        return item

    @property
    def id(self) -> str: return self._id
    @property
    def type(self) -> str: return self._type

    def to_canonical(self) -> dict[str, object]: return dict(self._data)


class ParametricEqAudioEffect(AudioEffect):
    pass


class PlaybackSpeedAudioEffect(AudioEffect):
    pass


class BassBoostAudioEffect(AudioEffect):
    pass


class AudioEffectCollection:
    __slots__ = ("_owner", "_ids", "_scope", "_scope_kind", "_items")

    @classmethod
    def _create(cls, owner: _Owner, ids: _IdAllocator, scope: object, scope_kind: str) -> "AudioEffectCollection":
        item = object.__new__(cls)
        item._owner, item._ids, item._scope, item._scope_kind, item._items = owner, ids, scope, scope_kind, []
        return item

    @property
    def items(self) -> tuple[AudioEffect, ...]: return tuple(self._items)

    def _add(self, effect_type: str, parameters: Mapping[str, object], identifier: str | None, effect_class: type[AudioEffect] = AudioEffect) -> AudioEffect:
        definition = audio_effect_definition(effect_type)
        if self._scope_kind not in definition["scopes"]:
            raise ValueError(f"audio effect {effect_type!r} is not valid at this scope")
        descriptors = {str(parameter["name"]): parameter for parameter in definition["parameters"]}
        values = dict(parameters)
        for name, descriptor in descriptors.items():
            if name not in values and descriptor.get("default") is not None:
                values[name] = descriptor["default"]
        if set(values) != set(descriptors):
            unknown = set(values) - set(descriptors)
            missing = set(descriptors) - set(values)
            raise TypeError(f"invalid parameters for {effect_type!r}: unknown={unknown}, missing={missing}")
        raw_values = values
        values: dict[str, float] = {}
        for name, raw in raw_values.items():
            value = _number(raw, name)  # type: ignore[arg-type]
            descriptor = descriptors[name]
            minimum, maximum = descriptor["minimum"], descriptor["maximum"]
            if (
                (minimum is not None and (value <= minimum if descriptor["minimum_exclusive"] else value < minimum))
                or (maximum is not None and (value >= maximum if descriptor["maximum_exclusive"] else value > maximum))
            ):
                raise ValueError(f"{name} is outside its authored range")
            values[name] = value
        effect_id = self._ids.allocate("audio-effect", scope=self._scope) if identifier is None else self._ids.reserve("audio-effect", identifier, scope=self._scope)
        effect = effect_class._create(self._owner, effect_id, effect_type, values)
        self._items.append(effect)
        return effect

    def add_effect(self, effect_type: str, /, *, id: str | None = None, **parameters: object) -> AudioEffect:
        return self._add(effect_type, parameters, id)

    def add_parametric_eq(self, *, frequency_hz: int | float, gain_db: int | float, q: int | float, id: str | None = None) -> ParametricEqAudioEffect:
        return self._add("parametric_eq", {"frequency_hz": frequency_hz, "gain_db": gain_db, "q": q}, id, ParametricEqAudioEffect)  # type: ignore[return-value]

    def add_playback_speed(self, *, rate: int | float, id: str | None = None) -> PlaybackSpeedAudioEffect:
        return self._add("playback_speed", {"rate": rate}, id, PlaybackSpeedAudioEffect)  # type: ignore[return-value]

    def add_bass_boost(self, *, gain_db: int | float = 6.0, frequency_hz: int | float = 100.0, id: str | None = None) -> BassBoostAudioEffect:
        return self._add("bass_boost", {"gain_db": gain_db, "frequency_hz": frequency_hz}, id, BassBoostAudioEffect)  # type: ignore[return-value]
