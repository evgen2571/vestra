"""Typed schema-v2 audio timeline authoring."""

from ._internal import _IdAllocator, _Owner, _number, _require_owner
from .assets import AudioAsset


def _nonnegative(value: int | float, name: str) -> float:
    value = _number(value, name)
    if value < 0: raise ValueError(f"{name} must be non-negative")
    return value

def _boolean(value: bool, name: str) -> bool:
    if not isinstance(value, bool): raise TypeError(f"{name} must be a boolean")
    return value

class AudioClip:
    __slots__ = ("_owner", "_id", "_asset", "_start", "_trim_start", "_trim_end", "_mute", "_gain", "_fade_in", "_fade_out")
    _owner: _Owner; _id: str; _asset: AudioAsset; _start: float; _trim_start: float
    _trim_end: float | None; _mute: bool; _gain: float; _fade_in: float; _fade_out: float
    def __init__(self, *args: object, **kwargs: object) -> None: raise TypeError("AudioClip objects must be created by AudioTrack")
    @classmethod
    def _create(cls, owner: _Owner, identifier: str, asset: AudioAsset, *, start: int | float, trim_start: int | float, trim_end: int | float | None, mute: bool, gain: int | float, fade_in: int | float, fade_out: int | float) -> "AudioClip":
        item = object.__new__(cls); item._owner = owner; item._id = identifier; item._asset = asset
        item._start = _nonnegative(start, "start"); item._trim_start = _nonnegative(trim_start, "trim_start")
        item._trim_end = None if trim_end is None else _nonnegative(trim_end, "trim_end")
        if item._trim_end is not None and item._trim_end <= item._trim_start: raise ValueError("trim_end must be greater than trim_start")
        item._mute = _boolean(mute, "mute"); item._gain = _nonnegative(gain, "gain")
        item._fade_in = _nonnegative(fade_in, "fade_in"); item._fade_out = _nonnegative(fade_out, "fade_out")
        return item
    @property
    def id(self) -> str: return self._id
    @property
    def asset(self) -> AudioAsset: return self._asset
    @property
    def start(self) -> float: return self._start
    @property
    def trim_start(self) -> float: return self._trim_start
    @property
    def trim_end(self) -> float | None: return self._trim_end
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
    @property
    def fade_out(self) -> float: return self._fade_out
    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {"id": self.id, "asset": self.asset.id, "start": self.start, "trim_start": self.trim_start, "mute": self.mute, "gain": self.gain, "fade_in": self.fade_in, "fade_out": self.fade_out}
        if self.trim_end is not None: data["trim_end"] = self.trim_end
        return data
    def __repr__(self) -> str: return f"AudioClip(id={self.id!r}, asset={self.asset.id!r})"

class AudioTrack:
    __slots__ = ("_owner", "_ids", "_id", "_mute", "_gain", "_clips")
    _owner: _Owner; _ids: _IdAllocator; _id: str; _mute: bool; _gain: float; _clips: list[AudioClip]
    def __init__(self, *args: object, **kwargs: object) -> None: raise TypeError("AudioTrack objects must be created by AudioTimeline")
    @classmethod
    def _create(cls, owner: _Owner, ids: _IdAllocator, identifier: str, *, mute: bool, gain: int | float) -> "AudioTrack":
        item = object.__new__(cls); item._owner = owner; item._ids = ids; item._id = identifier; item._mute = _boolean(mute, "mute"); item._gain = _nonnegative(gain, "gain"); item._clips = []; return item
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
    def add_clip(self, *, asset: AudioAsset, start: int | float, trim_start: int | float = 0.0, trim_end: int | float | None = None, mute: bool = False, gain: int | float = 1.0, fade_in: int | float = 0.0, fade_out: int | float = 0.0, id: str | None = None) -> AudioClip:
        if not isinstance(asset, AudioAsset): raise TypeError("asset must be AudioAsset")
        _require_owner(self._owner, asset._owner)
        staged = AudioClip._create(self._owner, "", asset, start=start, trim_start=trim_start, trim_end=trim_end, mute=mute, gain=gain, fade_in=fade_in, fade_out=fade_out)
        if id is not None: self._ids.validate("audio-clip", id)
        staged._id = self._ids.allocate("audio-clip") if id is None else self._ids.reserve("audio-clip", id)
        self._clips.append(staged); return staged
    def to_canonical(self) -> dict[str, object]: return {"id": self.id, "mute": self.mute, "gain": self.gain, "clips": [clip.to_canonical() for clip in self._clips]}
    def __repr__(self) -> str: return f"AudioTrack(id={self.id!r}, clips={len(self.clips)})"

class AudioTimeline:
    __slots__ = ("_owner", "_ids", "_tracks")
    _owner: _Owner; _ids: _IdAllocator; _tracks: list[AudioTrack]
    def __init__(self, *args: object, **kwargs: object) -> None: raise TypeError("AudioTimeline objects must be created by ProjectBuilder")
    @classmethod
    def _create(cls, owner: _Owner, ids: _IdAllocator) -> "AudioTimeline":
        item = object.__new__(cls); item._owner = owner; item._ids = ids; item._tracks = []; return item
    @property
    def tracks(self) -> tuple[AudioTrack, ...]: return tuple(self._tracks)
    def add_track(self, *, id: str | None = None, mute: bool = False, gain: int | float = 1.0) -> AudioTrack:
        # Validate all local fields before allocating/reserving an identifier.
        staged = AudioTrack._create(self._owner, self._ids, "", mute=mute, gain=gain)
        if id is not None: self._ids.validate("audio-track", id)
        staged._id = self._ids.allocate("audio-track") if id is None else self._ids.reserve("audio-track", id)
        self._tracks.append(staged); return staged
    def to_canonical(self) -> dict[str, object]: return {"tracks": [track.to_canonical() for track in self._tracks]}
    def __repr__(self) -> str: return f"AudioTimeline(tracks={len(self.tracks)})"
