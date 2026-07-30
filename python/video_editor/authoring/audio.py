"""The one optional global audio track supported by the project model."""

from ._internal import _Owner, _number
from .assets import AudioAsset


def _nonnegative(value: int | float, name: str) -> float:
    number = _number(value, name)
    if number < 0.0:
        raise ValueError(f"{name} must be non-negative")
    return number


class AudioTrack:
    """Mutable settings for one builder-owned global audio track."""

    __slots__ = ("_owner", "_asset", "_timeline_start", "_trim_start", "_trim_end", "_volume",
                 "_fade_in", "_fade_out", "_mute")
    _owner: _Owner
    _asset: AudioAsset
    _timeline_start: float
    _trim_start: float
    _trim_end: float | None
    _volume: float
    _fade_in: float
    _fade_out: float
    _mute: bool

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError("AudioTrack objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls, owner: _Owner, asset: AudioAsset, *, timeline_start: int | float,
                trim_start: int | float, trim_end: int | float | None, volume: int | float,
                fade_in: int | float, fade_out: int | float, mute: bool) -> "AudioTrack":
        instance = object.__new__(cls)
        instance._initialize(owner, asset, timeline_start=timeline_start, trim_start=trim_start,
                             trim_end=trim_end, volume=volume, fade_in=fade_in,
                             fade_out=fade_out, mute=mute)
        return instance

    def _initialize(self, owner: _Owner, asset: AudioAsset, *, timeline_start: int | float,
                    trim_start: int | float, trim_end: int | float | None, volume: int | float,
                    fade_in: int | float, fade_out: int | float, mute: bool) -> None:
        self._owner = owner
        self._asset = asset
        self.timeline_start = timeline_start
        self.trim_start = trim_start
        self.trim_end = trim_end
        self.volume = volume
        self.fade_in = fade_in
        self.fade_out = fade_out
        self.mute = mute

    def _replace_from(self, other: "AudioTrack") -> None:
        """Install already-validated state without changing this node's identity."""
        self._asset = other._asset
        self._timeline_start = other._timeline_start
        self._trim_start = other._trim_start
        self._trim_end = other._trim_end
        self._volume = other._volume
        self._fade_in = other._fade_in
        self._fade_out = other._fade_out
        self._mute = other._mute

    @property
    def asset(self) -> AudioAsset:
        return self._asset

    @property
    def timeline_start(self) -> float:
        return self._timeline_start

    @timeline_start.setter
    def timeline_start(self, value: int | float) -> None:
        self._timeline_start = _nonnegative(value, "timeline_start")

    @property
    def trim_start(self) -> float:
        return self._trim_start

    @trim_start.setter
    def trim_start(self, value: int | float) -> None:
        start = _nonnegative(value, "trim_start")
        current_end = getattr(self, "_trim_end", None)
        if current_end is not None and current_end <= start:
            raise ValueError("trim_end must be greater than trim_start")
        self._trim_start = start

    @property
    def trim_end(self) -> float | None:
        return self._trim_end

    @trim_end.setter
    def trim_end(self, value: int | float | None) -> None:
        if value is None:
            self._trim_end = None
            return
        end = _nonnegative(value, "trim_end")
        if end <= self.trim_start:
            raise ValueError("trim_end must be greater than trim_start")
        self._trim_end = end

    @property
    def volume(self) -> float:
        return self._volume

    @volume.setter
    def volume(self, value: int | float) -> None:
        number = _nonnegative(value, "volume")
        if number > 1.0:
            raise ValueError("volume must be between 0 and 1")
        self._volume = number

    @property
    def fade_in(self) -> float:
        return self._fade_in

    @fade_in.setter
    def fade_in(self, value: int | float) -> None:
        self._fade_in = _nonnegative(value, "fade_in")

    @property
    def fade_out(self) -> float:
        return self._fade_out

    @fade_out.setter
    def fade_out(self, value: int | float) -> None:
        self._fade_out = _nonnegative(value, "fade_out")

    @property
    def mute(self) -> bool:
        return self._mute

    @mute.setter
    def mute(self, value: bool) -> None:
        if not isinstance(value, bool):
            raise TypeError("mute must be a boolean")
        self._mute = value

    def to_canonical(self) -> dict[str, object]:
        data: dict[str, object] = {
            "asset": self.asset.id, "timeline_start": self.timeline_start,
            "trim_start": self.trim_start, "volume": self.volume,
            "fade_in": self.fade_in, "fade_out": self.fade_out, "mute": self.mute,
        }
        if self.trim_end is not None:
            data["trim_end"] = self.trim_end
        return data

    def __repr__(self) -> str:
        return f"AudioTrack(asset={self.asset.id!r}, mute={self.mute!r})"
