"""Typed schema-v2 audio timeline authoring."""

from dataclasses import dataclass

from ._internal import _IdAllocator, _Owner, _number, _require_owner
from .assets import AudioAsset
from .errors import AuthoringError
from .values import _CanonicalStrEnum
from .signals import MasterAudioSignals


def _nonnegative(value: int | float, name: str) -> float:
    number = _number(value, name)
    if number < 0:
        raise ValueError(f"{name} must be non-negative")
    return number


def _boolean(value: bool, name: str) -> bool:
    if not isinstance(value, bool):
        raise TypeError(f"{name} must be a boolean")
    return value


class AudioGainInterpolation(_CanonicalStrEnum):
    LINEAR = "linear"
    HOLD = "hold"


class AudioFadeCurve(_CanonicalStrEnum):
    LINEAR = "linear"
    EQUAL_POWER = "equal_power"


@dataclass(frozen=True, slots=True)
class AudioGainKeyframe:
    time: float
    gain: float
    interpolation: AudioGainInterpolation = AudioGainInterpolation.LINEAR

    def __post_init__(self) -> None:
        object.__setattr__(self, "time", _nonnegative(self.time, "time"))
        object.__setattr__(self, "gain", _nonnegative(self.gain, "gain"))
        if not isinstance(self.interpolation, AudioGainInterpolation):
            raise TypeError("interpolation must be AudioGainInterpolation")

    def to_canonical(self) -> dict[str, object]:
        return {"time": self.time, "gain": self.gain, "interpolation": self.interpolation.value}


class AudioClip:
    __slots__ = ("_owner", "_id", "_asset", "_start", "_trim_start", "_trim_end", "_mute", "_gain", "_gain_automation", "_fade_in", "_fade_out", "_fade_in_curve", "_fade_out_curve")
    _owner: _Owner
    _id: str
    _asset: AudioAsset
    _start: float
    _trim_start: float
    _trim_end: float | None
    _mute: bool
    _gain: float
    _gain_automation: tuple[AudioGainKeyframe, ...] | None
    _fade_in: float
    _fade_out: float
    _fade_in_curve: AudioFadeCurve
    _fade_out_curve: AudioFadeCurve
    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("AudioClip objects must be created by AudioTrack")

    @classmethod
    def _create(cls, owner: _Owner, identifier: str, asset: AudioAsset, *, start: int | float, trim_start: int | float, trim_end: int | float | None, mute: bool, gain: int | float, fade_in: int | float, fade_out: int | float, fade_in_curve: AudioFadeCurve, fade_out_curve: AudioFadeCurve) -> "AudioClip":
        item = object.__new__(cls)
        item._owner, item._id, item._asset = owner, identifier, asset
        item._start, item._trim_start = _nonnegative(start, "start"), _nonnegative(trim_start, "trim_start")
        item._trim_end = None if trim_end is None else _nonnegative(trim_end, "trim_end")
        if item._trim_end is not None and item._trim_end <= item._trim_start:
            raise ValueError("trim_end must be greater than trim_start")
        item._mute, item._gain = _boolean(mute, "mute"), _nonnegative(gain, "gain")
        item._fade_in, item._fade_out = _nonnegative(fade_in, "fade_in"), _nonnegative(fade_out, "fade_out")
        if not isinstance(fade_in_curve, AudioFadeCurve) or not isinstance(fade_out_curve, AudioFadeCurve):
            raise TypeError("fade curves must be AudioFadeCurve")
        item._fade_in_curve, item._fade_out_curve, item._gain_automation = fade_in_curve, fade_out_curve, None
        return item

    @property
    def id(self) -> str: return self._id
    @property
    def asset(self) -> AudioAsset: return self._asset
    @property
    def start(self) -> float: return self._start
    @start.setter
    def start(self, value: int | float) -> None: self._start = _nonnegative(value, "start")
    @property
    def trim_start(self) -> float: return self._trim_start
    @trim_start.setter
    def trim_start(self, value: int | float) -> None:
        staged = _nonnegative(value, "trim_start")
        if self._trim_end is not None and staged >= self._trim_end:
            raise ValueError("trim_start must be less than trim_end")
        self._trim_start = staged
    @property
    def trim_end(self) -> float | None: return self._trim_end
    @trim_end.setter
    def trim_end(self, value: int | float | None) -> None:
        staged = None if value is None else _nonnegative(value, "trim_end")
        if staged is not None and staged <= self._trim_start:
            raise ValueError("trim_end must be greater than trim_start")
        self._trim_end = staged
    @property
    def mute(self) -> bool: return self._mute
    @mute.setter
    def mute(self, value: bool) -> None: self._mute = _boolean(value, "mute")
    @property
    def gain(self) -> float: return self._gain
    @gain.setter
    def gain(self, value: int | float) -> None: self._gain = _nonnegative(value, "gain")
    @property
    def fade_in(self) -> float: return self._fade_in
    @fade_in.setter
    def fade_in(self, value: int | float) -> None: self._fade_in = _nonnegative(value, "fade_in")
    @property
    def fade_out(self) -> float: return self._fade_out
    @fade_out.setter
    def fade_out(self, value: int | float) -> None: self._fade_out = _nonnegative(value, "fade_out")
    @property
    def fade_in_curve(self) -> AudioFadeCurve: return self._fade_in_curve
    @fade_in_curve.setter
    def fade_in_curve(self, value: AudioFadeCurve) -> None:
        if not isinstance(value, AudioFadeCurve):
            raise TypeError("fade_in_curve must be AudioFadeCurve")
        self._fade_in_curve = value
    @property
    def fade_out_curve(self) -> AudioFadeCurve: return self._fade_out_curve
    @fade_out_curve.setter
    def fade_out_curve(self, value: AudioFadeCurve) -> None:
        if not isinstance(value, AudioFadeCurve):
            raise TypeError("fade_out_curve must be AudioFadeCurve")
        self._fade_out_curve = value
    @property
    def gain_automation(self) -> tuple[AudioGainKeyframe, ...]: return self._gain_automation or ()

    def set_gain_automation(self, keyframes: list[AudioGainKeyframe] | tuple[AudioGainKeyframe, ...]) -> None:
        staged = tuple(keyframes)
        if not staged:
            raise AuthoringError("gain automation must contain at least one keyframe")
        if any(not isinstance(item, AudioGainKeyframe) for item in staged):
            raise TypeError("gain automation items must be AudioGainKeyframe")
        if staged[0].time != 0.0 or any(later.time <= earlier.time for earlier, later in zip(staged, staged[1:])):
            raise AuthoringError("gain automation must start at zero and have strictly increasing times")
        self._gain_automation = staged

    def clear_gain_automation(self) -> None:
        self._gain_automation = None

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {"id": self.id, "asset": self.asset.id, "start": self.start, "trim_start": self.trim_start, "mute": self.mute, "gain": self.gain, "fade_in": self.fade_in, "fade_out": self.fade_out}
        if self.trim_end is not None: data["trim_end"] = self.trim_end
        if self._gain_automation is not None: data["gain_automation"] = {"keyframes": [item.to_canonical() for item in self._gain_automation]}
        if self.fade_in_curve is not AudioFadeCurve.LINEAR: data["fade_in_curve"] = self.fade_in_curve.value
        if self.fade_out_curve is not AudioFadeCurve.LINEAR: data["fade_out_curve"] = self.fade_out_curve.value
        return data

    def _same_identity(self, other: object) -> bool:
        return (
            type(self) is type(other)
            and self._owner is other._owner
            and self._id == other._id
        )

    def __eq__(self, other: object) -> bool: return self._same_identity(other)
    def __hash__(self) -> int: return hash((id(self._owner), type(self), self._id))
    def __repr__(self) -> str: return f"AudioClip(id={self.id!r}, asset={self.asset.id!r})"


class AudioTrack:
    __slots__ = ("_owner", "_ids", "_id", "_mute", "_gain", "_clips")
    _owner: _Owner
    _ids: _IdAllocator
    _id: str
    _mute: bool
    _gain: float
    _clips: list[AudioClip]
    def __init__(self, *args: object, **kwargs: object) -> None: raise TypeError("AudioTrack objects must be created by AudioTimeline")
    @classmethod
    def _create(cls, owner: _Owner, ids: _IdAllocator, identifier: str, *, mute: bool, gain: int | float) -> "AudioTrack":
        item = object.__new__(cls); item._owner, item._ids, item._id = owner, ids, identifier; item._mute, item._gain, item._clips = _boolean(mute, "mute"), _nonnegative(gain, "gain"), []; return item
    @property
    def id(self) -> str: return self._id
    @property
    def mute(self) -> bool: return self._mute
    @mute.setter
    def mute(self, value: bool) -> None: self._mute = _boolean(value, "mute")
    @property
    def gain(self) -> float: return self._gain
    @gain.setter
    def gain(self, value: int | float) -> None: self._gain = _nonnegative(value, "gain")
    @property
    def clips(self) -> tuple[AudioClip, ...]: return tuple(self._clips)
    def add_clip(self, *, asset: AudioAsset, start: int | float, trim_start: int | float = 0.0, trim_end: int | float | None = None, mute: bool = False, gain: int | float = 1.0, fade_in: int | float = 0.0, fade_out: int | float = 0.0, fade_in_curve: AudioFadeCurve = AudioFadeCurve.LINEAR, fade_out_curve: AudioFadeCurve = AudioFadeCurve.LINEAR, id: str | None = None) -> AudioClip:
        if not isinstance(asset, AudioAsset): raise TypeError("asset must be AudioAsset")
        _require_owner(self._owner, asset._owner)
        staged = AudioClip._create(self._owner, "", asset, start=start, trim_start=trim_start, trim_end=trim_end, mute=mute, gain=gain, fade_in=fade_in, fade_out=fade_out, fade_in_curve=fade_in_curve, fade_out_curve=fade_out_curve)
        if id is not None: self._ids.validate("audio-clip", id)
        staged._id = self._ids.allocate("audio-clip") if id is None else self._ids.reserve("audio-clip", id)
        self._clips.append(staged); return staged
    def to_canonical(self) -> dict[str, object]: return {"id": self.id, "mute": self.mute, "gain": self.gain, "clips": [clip.to_canonical() for clip in self._clips]}
    def _same_identity(self, other: object) -> bool:
        return (
            type(self) is type(other)
            and self._owner is other._owner
            and self._id == other._id
        )
    def __eq__(self, other: object) -> bool: return self._same_identity(other)
    def __hash__(self) -> int: return hash((id(self._owner), type(self), self._id))
    def __repr__(self) -> str: return f"AudioTrack(id={self.id!r}, clips={len(self.clips)})"


class AudioTimeline:
    __slots__ = ("_owner", "_ids", "_tracks", "_master")
    _owner: _Owner
    _ids: _IdAllocator
    _tracks: list[AudioTrack]
    _master: MasterAudioSignals
    def __init__(self, *args: object, **kwargs: object) -> None: raise TypeError("AudioTimeline objects must be created by ProjectBuilder")
    @classmethod
    def _create(cls, owner: _Owner, ids: _IdAllocator) -> "AudioTimeline":
        item = object.__new__(cls); item._owner, item._ids, item._tracks, item._master = owner, ids, [], MasterAudioSignals(); return item
    @property
    def tracks(self) -> tuple[AudioTrack, ...]: return tuple(self._tracks)
    @property
    def master(self) -> MasterAudioSignals: return self._master
    def add_track(self, *, id: str | None = None, mute: bool = False, gain: int | float = 1.0) -> AudioTrack:
        staged = AudioTrack._create(self._owner, self._ids, "", mute=mute, gain=gain)
        if id is not None: self._ids.validate("audio-track", id)
        staged._id = self._ids.allocate("audio-track") if id is None else self._ids.reserve("audio-track", id)
        self._tracks.append(staged); return staged
    def crossfade(self, outgoing: AudioClip, incoming: AudioClip, *, curve: AudioFadeCurve = AudioFadeCurve.EQUAL_POWER) -> None:
        if not isinstance(outgoing, AudioClip) or not isinstance(incoming, AudioClip): raise TypeError("crossfade clips must be AudioClip")
        _require_owner(self._owner, outgoing._owner); _require_owner(self._owner, incoming._owner)
        if outgoing is incoming or outgoing.start >= incoming.start: raise AuthoringError("crossfade requires an earlier distinct outgoing clip")
        if outgoing.trim_end is None: raise AuthoringError("crossfade needs an outgoing clip with an explicit trim_end")
        if not isinstance(curve, AudioFadeCurve): raise TypeError("curve must be AudioFadeCurve")
        overlap = outgoing.start + outgoing.trim_end - outgoing.trim_start - incoming.start
        if overlap <= 0: raise AuthoringError("crossfade requires an existing positive overlap")
        if incoming.trim_end is not None and incoming.trim_end - incoming.trim_start < overlap: raise AuthoringError("incoming clip is shorter than the crossfade overlap")
        outgoing_conflict = (
            (outgoing.fade_out == 0.0 and outgoing.fade_out_curve is not AudioFadeCurve.LINEAR)
            or (outgoing.fade_out != 0.0 and (outgoing.fade_out != overlap or outgoing.fade_out_curve is not curve))
        )
        incoming_conflict = (
            (incoming.fade_in == 0.0 and incoming.fade_in_curve is not AudioFadeCurve.LINEAR)
            or (incoming.fade_in != 0.0 and (incoming.fade_in != overlap or incoming.fade_in_curve is not curve))
        )
        if outgoing_conflict or incoming_conflict:
            raise AuthoringError("crossfade would overwrite conflicting fade configuration")
        outgoing._fade_out, outgoing._fade_out_curve = overlap, curve
        incoming._fade_in, incoming._fade_in_curve = overlap, curve
    def to_canonical(self) -> dict[str, object]: return {"tracks": [track.to_canonical() for track in self._tracks]}
    def __repr__(self) -> str: return f"AudioTimeline(tracks={len(self.tracks)})"
