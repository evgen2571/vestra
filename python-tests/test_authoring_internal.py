import pytest

from video_editor.authoring._internal import _IdAllocator, _Owner, _require_owner
from video_editor.authoring.errors import AuthoringError


def test_ids_are_deterministic_local_and_fixed_width() -> None:
    first = _IdAllocator()
    second = _IdAllocator()
    assert first.allocate("clip", "clip") == "clip-000001"
    assert first.allocate("clip", "clip") == "clip-000002"
    assert first.allocate("asset", "image") == "image-000001"
    assert second.allocate("clip", "clip") == "clip-000001"


def test_explicit_reservations_reject_duplicates_and_skip_collisions() -> None:
    allocator = _IdAllocator()
    allocator.reserve("clip", "clip-000001")
    assert allocator.allocate("clip", "clip") == "clip-000002"
    with pytest.raises(AuthoringError, match="duplicate"):
        allocator.reserve("clip", "clip-000001")


def test_asset_namespace_has_independent_prefix_counters() -> None:
    allocator = _IdAllocator()
    assert allocator.allocate("asset", "image") == "image-000001"
    assert allocator.allocate("asset", "audio") == "audio-000001"
    allocator.reserve("asset", "image-000002")
    assert allocator.allocate("asset", "image") == "image-000003"
    with pytest.raises(AuthoringError, match="duplicate"):
        allocator.reserve("asset", "image-000001")


def test_effect_scopes_are_independent() -> None:
    allocator = _IdAllocator()
    allocator.reserve("effect", "effect-000001", scope="clip-a")
    allocator.reserve("effect", "effect-000001", scope="clip-b")
    assert allocator.allocate("effect", "effect", scope="clip-a") == "effect-000002"
    assert allocator.allocate("effect", "effect", scope="clip-b") == "effect-000002"


def test_owners_are_identity_local() -> None:
    owner = _Owner()
    _require_owner(owner, owner)
    with pytest.raises(AuthoringError, match="different builder"):
        _require_owner(owner, _Owner())
