"""Mutable, track-oriented audio authoring for :class:`vestra.Project`.

The objects in this module are deliberately independent of the native builder.
They retain paths and authored values until a project snapshot is requested.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import TYPE_CHECKING, Iterable, Sequence, TypeAlias, cast

from .authoring._internal import _number as _finite_number
from .authoring.audio import AudioFadeCurve, AudioGainInterpolation, AudioGainKeyframe
from .authoring.audio_effects import AudioFilterKeyframe, AudioFilterMode, _effect_values, audio_effect_definition
from .authoring.signals import MasterAudioSignals
from .authoring.errors import AuthoringError

if TYPE_CHECKING:
    from .editor import Project


def _number(value: int | float, name: str) -> float:
    result = _finite_number(value, name)
    if result < 0:
        raise ValueError(f"{name} must be non-negative")
    return result


def _boolean(value: bool, name: str) -> bool:
    if not isinstance(value, bool):
        raise TypeError(f"{name} must be a boolean")
    return value


def _path(value: str | os.PathLike[str]) -> str:
    try:
        result = os.fspath(value)
    except TypeError as error:
        raise TypeError("path must be str or PathLike[str]") from error
    if not isinstance(result, str):
        raise TypeError("path must be str or PathLike[str]")
    if not result or result.isspace():
        raise ValueError("path must not be empty")
    return result


def _identifier(value: str | None, namespace: str) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{namespace} id must be a string or None")
    if not value or value.isspace():
        raise ValueError(f"{namespace} id must not be empty")
    return value


@dataclass(frozen=True, slots=True)
class ParametricEq:
    """Parametric equalizer effect, valid on clips, tracks, and master."""

    frequency_hz: float
    gain_db: float
    q: float

    def __post_init__(self) -> None:
        values = _effect_values(
            audio_effect_definition("parametric_eq"),
            {
                "frequency_hz": self.frequency_hz,
                "gain_db": self.gain_db,
                "q": self.q,
            },
        )
        for name, value in values.items():
            object.__setattr__(self, name, value)

    @property
    def type(self) -> str:
        return "parametric_eq"

    def parameters(self) -> dict[str, float]:
        return {"frequency_hz": self.frequency_hz, "gain_db": self.gain_db, "q": self.q}


@dataclass(frozen=True, slots=True)
class BassBoost:
    """Bass boost effect, valid on clips, tracks, and master."""

    gain_db: float = 6.0
    frequency_hz: float = 100.0

    def __post_init__(self) -> None:
        values = _effect_values(
            audio_effect_definition("bass_boost"),
            {
                "gain_db": self.gain_db,
                "frequency_hz": self.frequency_hz,
            },
        )
        for name, value in values.items():
            object.__setattr__(self, name, value)

    @property
    def type(self) -> str:
        return "bass_boost"

    def parameters(self) -> dict[str, float]:
        return {"gain_db": self.gain_db, "frequency_hz": self.frequency_hz}


@dataclass(frozen=True, slots=True)
class PlaybackSpeed:
    """Clip playback speed with optional linked pitch."""

    rate: float
    link_pitch: bool = False

    def __post_init__(self) -> None:
        values = _effect_values(
            audio_effect_definition("playback_speed"),
            {"rate": self.rate, "link_pitch": self.link_pitch},
        )
        object.__setattr__(self, "rate", values["rate"])

    @property
    def type(self) -> str:
        return "playback_speed"

    def parameters(self) -> dict[str, float | bool]:
        if self.link_pitch:
            return {"rate": self.rate, "link_pitch": True}
        return {"rate": self.rate}


@dataclass(frozen=True, slots=True)
class RoomReverb:
    """Short room reflections for clip, track, and master stacks."""

    decay_seconds: float
    mix: float

    def __post_init__(self) -> None:
        values = _effect_values(audio_effect_definition(self.type), self.parameters())
        for name, value in values.items():
            object.__setattr__(self, name, value)

    @property
    def type(self) -> str:
        return "room_reverb"

    def parameters(self) -> dict[str, float]:
        return {"decay_seconds": self.decay_seconds, "mix": self.mix}


@dataclass(frozen=True, slots=True)
class FilteredDelay:
    """Low-pass filtered repeats for clip, track, and master stacks."""

    delay_ms: float
    repeats: int
    mix: float
    cutoff_hz: float

    def __post_init__(self) -> None:
        values = _effect_values(audio_effect_definition(self.type), self.parameters())
        for name, value in values.items():
            object.__setattr__(self, name, value)

    @property
    def type(self) -> str:
        return "filtered_delay"

    def parameters(self) -> dict[str, float | int]:
        return {
            "delay_ms": self.delay_ms,
            "repeats": self.repeats,
            "mix": self.mix,
            "cutoff_hz": self.cutoff_hz,
        }


@dataclass(frozen=True, slots=True)
class BeatRepeat:
    """Replace a clip-local window with a repeated slice from the clip."""

    slice_start: float
    slice_duration: float
    output_start: float
    repeats: int

    def __post_init__(self) -> None:
        values = _effect_values(audio_effect_definition(self.type), self.parameters())
        for name, value in values.items():
            object.__setattr__(self, name, value)

    @property
    def type(self) -> str:
        return "beat_repeat"

    def parameters(self) -> dict[str, float | int]:
        return {
            "slice_start": self.slice_start,
            "slice_duration": self.slice_duration,
            "output_start": self.output_start,
            "repeats": self.repeats,
        }


@dataclass(frozen=True, slots=True)
class FilterSweep:
    """Sweep a clip filter's cutoff or center frequency between keyframes."""

    mode: AudioFilterMode
    keyframes: Sequence[AudioFilterKeyframe]

    def __post_init__(self) -> None:
        keyframes = tuple(self.keyframes)
        if any(not isinstance(keyframe, AudioFilterKeyframe) for keyframe in keyframes):
            raise TypeError("keyframes must contain AudioFilterKeyframe values")
        _effect_values(
            audio_effect_definition(self.type), {"mode": self.mode, "keyframes": keyframes},
        )
        object.__setattr__(self, "keyframes", keyframes)

    @property
    def type(self) -> str:
        return "filter_sweep"

    def parameters(self) -> dict[str, object]:
        return {
            "mode": self.mode,
            "keyframes": [keyframe.to_canonical() for keyframe in self.keyframes],
        }


@dataclass(frozen=True, slots=True)
class PitchShift:
    """Shift a clip's pitch in semitones at its original tempo."""

    semitones: float

    def __post_init__(self) -> None:
        values = _effect_values(audio_effect_definition(self.type), self.parameters())
        object.__setattr__(self, "semitones", values["semitones"])

    @property
    def type(self) -> str:
        return "pitch_shift"

    def parameters(self) -> dict[str, float]:
        return {"semitones": self.semitones}


@dataclass(frozen=True, slots=True)
class TapeStop:
    """Slow down a clip to silence during a clip-local interval."""

    start: float
    duration: float

    def __post_init__(self) -> None:
        values = _effect_values(audio_effect_definition(self.type), self.parameters())
        for name, value in values.items():
            object.__setattr__(self, name, value)

    @property
    def type(self) -> str:
        return "tape_stop"

    def parameters(self) -> dict[str, float]:
        return {"start": self.start, "duration": self.duration}


AudioEffectValue: TypeAlias = (
    ParametricEq | BassBoost | PlaybackSpeed | RoomReverb | FilteredDelay
    | BeatRepeat | FilterSweep | PitchShift | TapeStop
)

class AudioEffectStack:
    """Ordered, scope-aware collection of high-level audio effect values."""

    __slots__ = ("_scope", "_items")

    def __init__(self, scope: str) -> None:
        if scope not in {"clip", "track", "master"}:
            raise ValueError("scope must be clip, track, or master")
        self._scope = scope
        self._items: list[AudioEffectValue] = []

    @property
    def items(self) -> tuple[AudioEffectValue, ...]:
        return tuple(self._items)

    def _validate(self, effect: AudioEffectValue) -> None:
        if not isinstance(
            effect, (ParametricEq, BassBoost, PlaybackSpeed, RoomReverb, FilteredDelay,
                     BeatRepeat, FilterSweep, PitchShift, TapeStop)
        ):
            raise TypeError("effect must be an audio effect value")
        definition = audio_effect_definition(effect.type)
        scopes = cast(Sequence[str], definition["scopes"])
        if self._scope not in scopes:
            raise ValueError(f"audio effect {effect.type!r} is not valid at this scope")

    def add(self, effect: AudioEffectValue) -> AudioEffectValue:
        self._validate(effect)
        self._items.append(effect)
        return effect

    def extend(self, effects: Iterable[AudioEffectValue]) -> None:
        staged = tuple(effects)
        for effect in staged:
            self._validate(effect)
        self._items.extend(staged)


class AudioClip:
    """A path-based audio placement owned by one high-level audio track."""

    __slots__ = (
        "_track",
        "_id",
        "_path",
        "_start",
        "_trim_start",
        "_trim_end",
        "_mute",
        "_gain",
        "_fade_in",
        "_fade_out",
        "_fade_in_curve",
        "_fade_out_curve",
        "_gain_automation",
        "_effects",
    )

    def __init__(
        self,
        track: AudioTrack,
        identifier: str,
        path: str,
        *,
        start: int | float,
        trim_start: int | float,
        trim_end: int | float | None,
        mute: bool,
        gain: int | float,
        fade_in: int | float,
        fade_out: int | float,
        fade_in_curve: AudioFadeCurve,
        fade_out_curve: AudioFadeCurve,
    ) -> None:
        self._track, self._id, self._path = track, identifier, path
        self._start, self._trim_start = (
            _number(start, "start"),
            _number(trim_start, "trim_start"),
        )
        self._trim_end = None if trim_end is None else _number(trim_end, "trim_end")
        if self._trim_end is not None and self._trim_end <= self._trim_start:
            raise ValueError("trim_end must be greater than trim_start")
        self._mute, self._gain = _boolean(mute, "mute"), _number(gain, "gain")
        self._fade_in, self._fade_out = (
            _number(fade_in, "fade_in"),
            _number(fade_out, "fade_out"),
        )
        if not isinstance(fade_in_curve, AudioFadeCurve) or not isinstance(
            fade_out_curve, AudioFadeCurve
        ):
            raise TypeError("fade curves must be AudioFadeCurve")
        self._fade_in_curve, self._fade_out_curve = fade_in_curve, fade_out_curve
        self._gain_automation: tuple[AudioGainKeyframe, ...] | None = None
        self._effects = AudioEffectStack("clip")

    @property
    def id(self) -> str:
        return self._id

    @property
    def path(self) -> str:
        return self._path

    @property
    def track(self) -> AudioTrack:
        return self._track

    @property
    def start(self) -> float:
        return self._start

    @start.setter
    def start(self, value: int | float) -> None:
        self._start = _number(value, "start")

    @property
    def trim_start(self) -> float:
        return self._trim_start

    @trim_start.setter
    def trim_start(self, value: int | float) -> None:
        staged = _number(value, "trim_start")
        if self._trim_end is not None and staged >= self._trim_end:
            raise ValueError("trim_start must be less than trim_end")
        self._trim_start = staged

    @property
    def trim_end(self) -> float | None:
        return self._trim_end

    @trim_end.setter
    def trim_end(self, value: int | float | None) -> None:
        staged = None if value is None else _number(value, "trim_end")
        if staged is not None and staged <= self._trim_start:
            raise ValueError("trim_end must be greater than trim_start")
        self._trim_end = staged

    @property
    def mute(self) -> bool:
        return self._mute

    @mute.setter
    def mute(self, value: bool) -> None:
        self._mute = _boolean(value, "mute")

    @property
    def gain(self) -> float:
        return self._gain

    @gain.setter
    def gain(self, value: int | float) -> None:
        self._gain = _number(value, "gain")

    @property
    def fade_in(self) -> float:
        return self._fade_in

    @fade_in.setter
    def fade_in(self, value: int | float) -> None:
        self._fade_in = _number(value, "fade_in")

    @property
    def fade_out(self) -> float:
        return self._fade_out

    @fade_out.setter
    def fade_out(self, value: int | float) -> None:
        self._fade_out = _number(value, "fade_out")

    @property
    def fade_in_curve(self) -> AudioFadeCurve:
        return self._fade_in_curve

    @fade_in_curve.setter
    def fade_in_curve(self, value: AudioFadeCurve) -> None:
        if not isinstance(value, AudioFadeCurve):
            raise TypeError("fade_in_curve must be AudioFadeCurve")
        self._fade_in_curve = value

    @property
    def fade_out_curve(self) -> AudioFadeCurve:
        return self._fade_out_curve

    @fade_out_curve.setter
    def fade_out_curve(self, value: AudioFadeCurve) -> None:
        if not isinstance(value, AudioFadeCurve):
            raise TypeError("fade_out_curve must be AudioFadeCurve")
        self._fade_out_curve = value

    @property
    def gain_automation(self) -> tuple[AudioGainKeyframe, ...]:
        return self._gain_automation or ()

    @property
    def effects(self) -> AudioEffectStack:
        return self._effects

    def set_gain_automation(self, keyframes: Iterable[AudioGainKeyframe]) -> None:
        staged = tuple(keyframes)
        if not staged:
            raise AuthoringError("gain automation must contain at least one keyframe")
        if any(not isinstance(item, AudioGainKeyframe) for item in staged):
            raise TypeError("gain automation items must be AudioGainKeyframe")
        if staged[0].time != 0.0 or any(
            later.time <= earlier.time for earlier, later in zip(staged, staged[1:])
        ):
            raise AuthoringError(
                "gain automation must start at zero and have strictly increasing times"
            )
        self._gain_automation = staged

    def clear_gain_automation(self) -> None:
        self._gain_automation = None


class AudioTrack:
    """An ordered collection of path-based audio clips."""

    __slots__ = ("_audio", "_id", "_mute", "_gain", "_clips", "_effects")

    def __init__(
        self, audio: AudioTimeline, identifier: str, *, mute: bool, gain: int | float
    ) -> None:
        self._audio, self._id = audio, identifier
        self._mute, self._gain = _boolean(mute, "mute"), _number(gain, "gain")
        self._clips: list[AudioClip] = []
        self._effects = AudioEffectStack("track")

    @property
    def id(self) -> str:
        return self._id

    @property
    def project(self) -> Project:
        return self._audio.project

    @property
    def mute(self) -> bool:
        return self._mute

    @mute.setter
    def mute(self, value: bool) -> None:
        self._mute = _boolean(value, "mute")

    @property
    def gain(self) -> float:
        return self._gain

    @gain.setter
    def gain(self, value: int | float) -> None:
        self._gain = _number(value, "gain")

    @property
    def clips(self) -> tuple[AudioClip, ...]:
        return tuple(self._clips)

    @property
    def effects(self) -> AudioEffectStack:
        return self._effects

    def add(
        self,
        path: str | os.PathLike[str],
        *,
        start: int | float = 0.0,
        trim_start: int | float = 0.0,
        trim_end: int | float | None = None,
        mute: bool = False,
        gain: int | float = 1.0,
        fade_in: int | float = 0.0,
        fade_out: int | float = 0.0,
        fade_in_curve: AudioFadeCurve = AudioFadeCurve.LINEAR,
        fade_out_curve: AudioFadeCurve = AudioFadeCurve.LINEAR,
        id: str | None = None,
    ) -> AudioClip:
        source = _path(path)
        requested = _identifier(id, "audio clip")
        if requested is not None and requested in self._audio._clip_ids:
            raise ValueError(f"duplicate audio clip ID: {requested!r}")
        identifier = requested or self._audio._allocate(
            "audio-clip", self._audio._clip_ids
        )
        staged = AudioClip(
            self,
            identifier,
            source,
            start=start,
            trim_start=trim_start,
            trim_end=trim_end,
            mute=mute,
            gain=gain,
            fade_in=fade_in,
            fade_out=fade_out,
            fade_in_curve=fade_in_curve,
            fade_out_curve=fade_out_curve,
        )
        self._audio._clip_ids.add(identifier)
        self._clips.append(staged)
        return staged

    add_clip = add


class AudioTimeline:
    """The single high-level audio timeline owned by a project."""

    __slots__ = (
        "_project",
        "_tracks",
        "_track_ids",
        "_clip_ids",
        "_effects",
        "_signal",
    )

    def __init__(self, project: Project) -> None:
        self._project = project
        self._tracks: list[AudioTrack] = []
        self._track_ids: set[str] = set()
        self._clip_ids: set[str] = set()
        self._effects = AudioEffectStack("master")
        self._signal = MasterAudioSignals()

    @property
    def project(self) -> Project:
        return self._project

    @property
    def tracks(self) -> tuple[AudioTrack, ...]:
        return tuple(self._tracks)

    @property
    def effects(self) -> AudioEffectStack:
        return self._effects

    @property
    def signal(self) -> MasterAudioSignals:
        """Stable factory for immutable Master-audio analysis signals."""
        return self._signal

    def _allocate(self, prefix: str, used: set[str]) -> str:
        index = 1
        while f"{prefix}-{index:06d}" in used:
            index += 1
        return f"{prefix}-{index:06d}"

    def track(
        self,
        identifier: str | None = None,
        *,
        id: str | None = None,
        mute: bool = False,
        gain: int | float = 1.0,
    ) -> AudioTrack:
        if identifier is not None and id is not None:
            raise TypeError("track accepts the identifier either positionally or as id")
        requested = _identifier(
            identifier if identifier is not None else id, "audio track"
        )
        if requested is None:
            requested = self._allocate("audio-track", self._track_ids)
        if requested in self._track_ids:
            raise ValueError(f"duplicate audio track ID: {requested!r}")
        staged = AudioTrack(self, requested, mute=mute, gain=gain)
        self._track_ids.add(requested)
        self._tracks.append(staged)
        return staged

    add_track = track

    def crossfade(
        self,
        outgoing: AudioClip,
        incoming: AudioClip,
        *,
        curve: AudioFadeCurve = AudioFadeCurve.EQUAL_POWER,
    ) -> None:
        if not isinstance(outgoing, AudioClip) or not isinstance(incoming, AudioClip):
            raise TypeError("crossfade clips must be AudioClip")
        if outgoing._track._audio is not self or incoming._track._audio is not self:
            raise AuthoringError("audio clips belong to a different project")
        if outgoing is incoming or outgoing.start >= incoming.start:
            raise AuthoringError("crossfade requires an earlier distinct outgoing clip")
        if outgoing.trim_end is None:
            raise AuthoringError(
                "crossfade needs an outgoing clip with an explicit trim_end"
            )
        if not isinstance(curve, AudioFadeCurve):
            raise TypeError("curve must be AudioFadeCurve")
        overlap = (
            outgoing.start + outgoing.trim_end - outgoing.trim_start - incoming.start
        )
        if overlap <= 0:
            raise AuthoringError("crossfade requires an existing positive overlap")
        incoming_length = (
            None
            if incoming.trim_end is None
            else incoming.trim_end - incoming.trim_start
        )
        if incoming_length is not None and incoming_length < overlap:
            raise AuthoringError("incoming clip is shorter than the crossfade overlap")
        outgoing_conflict = (
            outgoing.fade_out == 0
            and outgoing.fade_out_curve is not AudioFadeCurve.LINEAR
        ) or (
            outgoing.fade_out != 0
            and (outgoing.fade_out != overlap or outgoing.fade_out_curve is not curve)
        )
        incoming_conflict = (
            incoming.fade_in == 0
            and incoming.fade_in_curve is not AudioFadeCurve.LINEAR
        ) or (
            incoming.fade_in != 0
            and (incoming.fade_in != overlap or incoming.fade_in_curve is not curve)
        )
        if outgoing_conflict or incoming_conflict:
            raise AuthoringError(
                "crossfade would overwrite conflicting fade configuration"
            )
        outgoing._fade_out, outgoing._fade_out_curve = overlap, curve
        incoming._fade_in, incoming._fade_in_curve = overlap, curve

    # Alias makes the master scope explicit without adding another owner object.
    @property
    def master_effects(self) -> AudioEffectStack:
        return self.effects


__all__ = [
    "AudioClip",
    "AudioEffectStack",
    "AudioFadeCurve",
    "AudioGainInterpolation",
    "AudioGainKeyframe",
    "AudioTimeline",
    "AudioTrack",
    "AudioEffectValue",
    "BassBoost",
    "FilteredDelay",
    "ParametricEq",
    "PlaybackSpeed",
    "RoomReverb",
    "AudioFilterKeyframe",
    "AudioFilterMode",
    "BeatRepeat",
    "FilterSweep",
    "PitchShift",
    "TapeStop",
]
