"""Descriptor-driven audio-effect authoring."""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
from math import floor
from types import MappingProxyType
from typing import Literal, Mapping, Sequence, TypeAlias, cast

from ._internal import _IdAllocator, _Owner, _number
from vestra._native import audio_effect_definitions as _native_audio_effect_definitions


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


AudioFilterMode: TypeAlias = Literal["low_pass", "high_pass", "band_pass"]


@dataclass(frozen=True, slots=True)
class AudioFilterKeyframe:
    """A filter cutoff or center frequency at a processed clip-local time."""

    time: float
    frequency_hz: float

    def __post_init__(self) -> None:
        time = _number(self.time, "time")
        frequency_hz = _number(self.frequency_hz, "frequency_hz")
        if not 0 <= time <= 2**64 / 48_000:
            raise ValueError("time must be non-negative and representable at 48 kHz")
        if not 20 <= frequency_hz <= 20_000:
            raise ValueError("frequency_hz must be between 20 and 20000")
        object.__setattr__(self, "time", time)
        object.__setattr__(self, "frequency_hz", frequency_hz)

    def to_canonical(self) -> dict[str, float]:
        return {"time": self.time, "frequency_hz": self.frequency_hz}


def _filter_keyframes(raw: object) -> list[dict[str, float]]:
    if not isinstance(raw, Sequence) or isinstance(raw, (str, bytes)):
        raise TypeError("keyframes must be a sequence")
    if not 1 <= len(raw) <= 256:
        raise ValueError("keyframes must contain between 1 and 256 entries")
    keyframes: list[dict[str, float]] = []
    previous = -1
    for item in raw:
        if isinstance(item, AudioFilterKeyframe):
            keyframe = item
        elif isinstance(item, Mapping) and set(item) == {"time", "frequency_hz"}:
            keyframe = AudioFilterKeyframe(item["time"], item["frequency_hz"])
        else:
            raise TypeError("keyframes must contain AudioFilterKeyframe values or canonical objects")
        exact = keyframe.time * 48_000
        whole = floor(exact)
        sample = whole + (exact - whole >= 0.5)
        if (not keyframes and keyframe.time != 0) or sample <= previous:
            raise ValueError("keyframes must start at zero and strictly increase at 48 kHz")
        keyframes.append(keyframe.to_canonical())
        previous = sample
    return keyframes


def _effect_values(
    definition: Mapping[str, object], parameters: Mapping[str, object]
) -> dict[str, object]:
    parameter_definitions = cast(tuple[Mapping[str, object], ...], definition["parameters"])
    descriptors = {str(parameter["name"]): parameter for parameter in parameter_definitions}
    raw_values = dict(parameters)
    for name, descriptor in descriptors.items():
        if name not in raw_values and descriptor["default"] is not None:
            raw_values[name] = descriptor["default"]
    if set(raw_values) != set(descriptors):
        unknown = set(raw_values) - set(descriptors)
        missing = set(descriptors) - set(raw_values)
        raise TypeError(
            f"invalid parameters for {definition['id']!r}: unknown={unknown}, missing={missing}"
        )
    values: dict[str, object] = {}
    for name, raw in raw_values.items():
        descriptor = descriptors[name]
        kind = descriptor["kind"]
        if kind == "boolean":
            if not isinstance(raw, bool):
                raise TypeError(f"{name} must be a boolean")
            values[name] = raw
            continue
        if kind == "filter_mode":
            if not isinstance(raw, str) or raw not in {"low_pass", "high_pass", "band_pass"}:
                raise ValueError("mode must be low_pass, high_pass, or band_pass")
            values[name] = raw
            continue
        if kind == "filter_keyframes":
            values[name] = _filter_keyframes(raw)
            continue
        value: float | int
        if kind == "integer":
            if isinstance(raw, bool) or not isinstance(raw, int):
                raise TypeError(f"{name} must be an integer")
            value = raw
        else:
            value = _number(raw, name)
        minimum = cast(float | None, descriptor["minimum"])
        maximum = cast(float | None, descriptor["maximum"])
        minimum_exclusive = cast(bool, descriptor["minimum_exclusive"])
        maximum_exclusive = cast(bool, descriptor["maximum_exclusive"])
        if (
            (minimum is not None and (value <= minimum if minimum_exclusive else value < minimum))
            or (maximum is not None and (value >= maximum if maximum_exclusive else value > maximum))
        ):
            raise ValueError(f"{name} is outside its authored range")
        values[name] = value
    return values


class AudioEffect:
    __slots__ = ("_owner", "_id", "_type", "_data")
    _owner: _Owner
    _id: str
    _type: str
    _data: dict[str, object]

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

    def to_canonical(self) -> dict[str, object]: return deepcopy(self._data)


class ParametricEqAudioEffect(AudioEffect):
    pass


class PlaybackSpeedAudioEffect(AudioEffect):
    pass


class BassBoostAudioEffect(AudioEffect):
    pass


class RoomReverbAudioEffect(AudioEffect):
    pass


class FilteredDelayAudioEffect(AudioEffect):
    pass


class BeatRepeatAudioEffect(AudioEffect):
    pass


class FilterSweepAudioEffect(AudioEffect):
    pass


class PitchShiftAudioEffect(AudioEffect):
    pass


class TapeStopAudioEffect(AudioEffect):
    pass


class AudioEffectCollection:
    __slots__ = ("_owner", "_ids", "_scope", "_scope_kind", "_items")
    _owner: _Owner
    _ids: _IdAllocator
    _scope: object
    _scope_kind: str
    _items: list[AudioEffect]

    @classmethod
    def _create(cls, owner: _Owner, ids: _IdAllocator, scope: object, scope_kind: str) -> "AudioEffectCollection":
        item = object.__new__(cls)
        item._owner, item._ids, item._scope, item._scope_kind, item._items = owner, ids, scope, scope_kind, []
        return item

    @property
    def items(self) -> tuple[AudioEffect, ...]: return tuple(self._items)

    def _add(self, effect_type: str, parameters: Mapping[str, object], identifier: str | None, effect_class: type[AudioEffect] = AudioEffect) -> AudioEffect:
        definition = audio_effect_definition(effect_type)
        scopes = cast(tuple[str, ...], definition["scopes"])
        if self._scope_kind not in scopes:
            raise ValueError(f"audio effect {effect_type!r} is not valid at this scope")
        values = _effect_values(definition, parameters)
        # Keep default-speed effects in their original canonical form.
        if values.get("link_pitch") is False:
            del values["link_pitch"]
        effect_id = self._ids.allocate("audio-effect", scope=self._scope) if identifier is None else self._ids.reserve("audio-effect", identifier, scope=self._scope)
        effect = effect_class._create(self._owner, effect_id, effect_type, values)
        self._items.append(effect)
        return effect

    def add_effect(self, effect_type: str, /, *, id: str | None = None, **parameters: object) -> AudioEffect:
        return self._add(effect_type, parameters, id)

    def add_parametric_eq(self, *, frequency_hz: int | float, gain_db: int | float, q: int | float, id: str | None = None) -> ParametricEqAudioEffect:
        return self._add("parametric_eq", {"frequency_hz": frequency_hz, "gain_db": gain_db, "q": q}, id, ParametricEqAudioEffect)  # type: ignore[return-value]

    def add_playback_speed(
        self, *, rate: int | float, link_pitch: bool = False, id: str | None = None
    ) -> PlaybackSpeedAudioEffect:
        return self._add("playback_speed", {"rate": rate, "link_pitch": link_pitch}, id, PlaybackSpeedAudioEffect)  # type: ignore[return-value]

    def add_bass_boost(self, *, gain_db: int | float = 6.0, frequency_hz: int | float = 100.0, id: str | None = None) -> BassBoostAudioEffect:
        return self._add("bass_boost", {"gain_db": gain_db, "frequency_hz": frequency_hz}, id, BassBoostAudioEffect)  # type: ignore[return-value]

    def add_room_reverb(
        self, *, decay_seconds: int | float, mix: int | float, id: str | None = None
    ) -> RoomReverbAudioEffect:
        return self._add("room_reverb", {"decay_seconds": decay_seconds, "mix": mix}, id, RoomReverbAudioEffect)  # type: ignore[return-value]

    def add_filtered_delay(
        self,
        *,
        delay_ms: int | float,
        repeats: int,
        mix: int | float,
        cutoff_hz: int | float,
        id: str | None = None,
    ) -> FilteredDelayAudioEffect:
        return self._add(  # type: ignore[return-value]
            "filtered_delay",
            {"delay_ms": delay_ms, "repeats": repeats, "mix": mix, "cutoff_hz": cutoff_hz},
            id,
            FilteredDelayAudioEffect,
        )

    def add_beat_repeat(
        self,
        *,
        slice_start: float,
        slice_duration: float,
        output_start: float,
        repeats: int,
        id: str | None = None,
    ) -> BeatRepeatAudioEffect:
        return self._add(  # type: ignore[return-value]
            "beat_repeat",
            {
                "slice_start": slice_start, "slice_duration": slice_duration,
                "output_start": output_start, "repeats": repeats,
            },
            id,
            BeatRepeatAudioEffect,
        )

    def add_filter_sweep(
        self,
        *,
        mode: AudioFilterMode,
        keyframes: Sequence[AudioFilterKeyframe],
        id: str | None = None,
    ) -> FilterSweepAudioEffect:
        return self._add(  # type: ignore[return-value]
            "filter_sweep", {"mode": mode, "keyframes": keyframes}, id, FilterSweepAudioEffect,
        )

    def add_pitch_shift(self, *, semitones: float, id: str | None = None) -> PitchShiftAudioEffect:
        return self._add(  # type: ignore[return-value]
            "pitch_shift", {"semitones": semitones}, id, PitchShiftAudioEffect,
        )

    def add_tape_stop(self, *, start: float, duration: float, id: str | None = None) -> TapeStopAudioEffect:
        return self._add(  # type: ignore[return-value]
            "tape_stop", {"start": start, "duration": duration}, id, TapeStopAudioEffect,
        )
