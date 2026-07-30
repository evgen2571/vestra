"""Immutable builder-owned references to registered media assets."""

from dataclasses import dataclass

from ._internal import _Owner


@dataclass(frozen=True, slots=True, eq=False)
class _Asset:
    _id: str
    _source: str
    _owner: _Owner

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


class ImageAsset(_Asset):
    """A registered image asset, created by ``ProjectBuilder``."""

    @property
    def kind(self) -> str:
        return "image"


class AudioAsset(_Asset):
    """A registered audio asset, created by ``ProjectBuilder``."""

    @property
    def kind(self) -> str:
        return "audio"
