"""Immutable builder-owned references to registered media assets."""

from typing import Self

from ._internal import _Owner


class _Asset:
    __slots__ = ("_id", "_source", "_owner")
    _id: str
    _source: str
    _owner: _Owner

    def __init__(self, *args: object, **kwargs: object) -> None:
        raise TypeError(f"{type(self).__name__} objects must be created by ProjectBuilder")

    @classmethod
    def _create(cls: type[Self], identifier: str, source: str, owner: _Owner) -> Self:
        instance = object.__new__(cls)
        instance._id = identifier
        instance._source = source
        instance._owner = owner
        return instance

    @property
    def id(self) -> str:
        return self._id

    @property
    def source(self) -> str:
        return self._source

    def _same_identity(self, other: object) -> bool:
        return (
            type(self) is type(other)
            and self._owner is other._owner
            and self._id == other._id
        )

    def __eq__(self, other: object) -> bool:
        return self._same_identity(other)

    def __hash__(self) -> int:
        return hash((id(self._owner), type(self), self._id))

    def __repr__(self) -> str:
        return f"{type(self).__name__}(id={self.id!r}, source={self.source!r})"


class ImageAsset(_Asset):
    """A registered image asset, created by ``ProjectBuilder``."""

    @property
    def kind(self) -> str:
        return "image"


class VideoAsset(_Asset):
    """A registered video asset, created by ``ProjectBuilder``."""

    @property
    def kind(self) -> str:
        return "video"


class AudioAsset(_Asset):
    """A registered audio asset, created by ``ProjectBuilder``."""

    @property
    def kind(self) -> str:
        return "audio"


class FontAsset(_Asset):
    """A registered file-backed font asset."""

    @property
    def kind(self) -> str:
        return "font"
