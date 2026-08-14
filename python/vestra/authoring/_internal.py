"""Private ownership and identifier helpers for future authoring nodes."""

from collections import defaultdict
from math import isfinite

from .errors import AuthoringError


def _number(value: int | float, name: str) -> float:
    """Normalize a public finite real number without depending on authoring nodes."""
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    number = float(value)
    if not isfinite(number):
        raise ValueError(f"{name} must be finite")
    return number


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

    def validate(self, namespace: str, identifier: str, *, scope: object = None) -> str:
        """Check an explicit identifier without changing allocator state."""
        if not isinstance(identifier, str):
            raise TypeError("explicit IDs must be strings")
        if not identifier or identifier.isspace():
            raise AuthoringError("explicit IDs must not be empty")
        if identifier in self._reserved[(namespace, scope)]:
            raise AuthoringError(f"duplicate {namespace} ID: {identifier!r}")
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

    def release(self, namespace: str, identifier: str, *, scope: object = None) -> None:
        self._reserved[(namespace, scope)].discard(identifier)


class _Owner:
    """An identity-only token that never enters canonical project data."""


def _require_owner(expected: _Owner, actual: _Owner) -> None:
    if expected is not actual:
        raise AuthoringError("authoring handles belong to a different builder")
