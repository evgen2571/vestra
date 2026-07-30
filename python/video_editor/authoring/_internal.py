"""Private ownership and identifier helpers for future authoring nodes."""

from collections import defaultdict

from .errors import AuthoringError


class _IdAllocator:
    """Builder-local, deterministic identifiers in their canonical namespaces."""

    def __init__(self) -> None:
        self._next: defaultdict[tuple[str, object, str], int] = defaultdict(lambda: 1)
        self._reserved: defaultdict[tuple[str, object], set[str]] = defaultdict(set)

    def reserve(self, namespace: str, identifier: str, *, scope: object = None) -> str:
        if not isinstance(identifier, str):
            raise TypeError("explicit IDs must be strings")
        if not identifier or identifier.isspace():
            raise AuthoringError("explicit IDs must not be empty")
        key = (namespace, scope)
        if identifier in self._reserved[key]:
            raise AuthoringError(f"duplicate {namespace} ID: {identifier!r}")
        self._reserved[key].add(identifier)
        return identifier

    def allocate(self, namespace: str, prefix: str | None = None, *, scope: object = None) -> str:
        prefix = namespace if prefix is None else prefix
        reserved_key = (namespace, scope)
        counter_key = (namespace, scope, prefix)
        while True:
            identifier = f"{prefix}-{self._next[counter_key]:06d}"
            self._next[counter_key] += 1
            if identifier not in self._reserved[reserved_key]:
                self._reserved[reserved_key].add(identifier)
                return identifier


class _Owner:
    """An identity-only token that never enters canonical project data."""


def _require_owner(expected: _Owner, actual: _Owner) -> None:
    if expected is not actual:
        raise AuthoringError("authoring handles belong to a different builder")
