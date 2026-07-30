from math import inf, nan

import pytest

from video_editor.authoring import ProjectBuilder
from video_editor import FrameRate, Project


def builder(*, metadata: object = None) -> ProjectBuilder:
    return ProjectBuilder(
        width=160, height=90, frame_rate=FrameRate(30, 1), output_path="out.mp4",
        duration=1.0, metadata=metadata,  # type: ignore[arg-type]
    )


def test_metadata_omits_top_level_none_and_preserves_nested_null() -> None:
    assert "metadata" not in builder().to_dict()
    data = builder(metadata={"description": None, "items": [1, None, 3]}).to_dict()
    assert data["metadata"] == {"description": None, "items": [1, None, 3]}
    assert isinstance(Project.from_dict(data), Project)


def test_metadata_is_deeply_owned_in_both_directions() -> None:
    metadata = {"items": [{"name": "first"}]}
    authored = builder(metadata=metadata)
    metadata["items"][0]["name"] = "later"
    snapshot = authored.to_dict()
    assert snapshot["metadata"] == {"items": [{"name": "first"}]}
    snapshot["metadata"]["items"][0]["name"] = "changed"  # type: ignore[index]
    assert authored.to_dict()["metadata"] == {"items": [{"name": "first"}]}


@pytest.mark.parametrize("value", [{1: "no"}, {"value": nan}, {"value": inf}, {"value": -inf}, b"no", {"x": b"no"}, {"x": object()}, {"x": {1, 2}}, ("x",)])
def test_metadata_rejects_non_json_values(value: object) -> None:
    with pytest.raises((TypeError, ValueError)):
        builder(metadata=value)
