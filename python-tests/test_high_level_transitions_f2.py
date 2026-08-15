from __future__ import annotations

import pytest

import vestra


def _project() -> tuple[vestra.Project, vestra.Layer, vestra.Layer]:
    project = vestra.Project(size=(8, 6), fps=10, duration=20)
    first = project.root.add(vestra.Image("first.png"), id="first")
    second = project.root.add(vestra.Image("second.png"), id="second")
    return project, first, second


def test_supported_presets_lower_to_generic_channels() -> None:
    project, first, second = _project()
    for definition in (vestra.Crossfade(), vestra.PushLeft(), vestra.ZoomCrossfade()):
        project.root.transitions.add(first, second, definition, start=1, duration=1)
    transitions = project.snapshot().to_dict()["visual"]["transitions"]
    assert [sorted(item["definition"]["outgoing"]) for item in transitions] == [
        ["opacity"], ["position_offset"], ["opacity", "scale_multiplier"]
    ]
    assert all("type" not in item["definition"] for item in transitions)


def test_direction_presets_follow_native_position_conventions() -> None:
    left = vestra.PushLeft().to_canonical()["outgoing"]["position_offset"]["keyframes"][-1]["value"]
    right = vestra.PushRight().to_canonical()["outgoing"]["position_offset"]["keyframes"][-1]["value"]
    assert left["x"] == -1.0 and left["y"] == pytest.approx(0.0)
    assert right["x"] == 1.0 and right["y"] == pytest.approx(0.0)
    assert vestra.PushUp().to_canonical()["outgoing"]["position_offset"]["keyframes"][-1]["value"]["y"] == pytest.approx(-1.0)
    assert vestra.PushDown().to_canonical()["outgoing"]["position_offset"]["keyframes"][-1]["value"]["y"] == pytest.approx(1.0)


def test_definition_reuse_does_not_share_placement_identity_or_timing() -> None:
    project, first, second = _project()
    third = project.root.add(vestra.Image("third.png"), id="third")
    fourth = project.root.add(vestra.Image("fourth.png"), id="fourth")
    definition = vestra.PushLeft()
    one = project.root.transitions.add(first, second, definition, start=4, duration=0.8)
    two = project.root.transitions.add(third, fourth, definition, start=12, duration=1.2)
    assert one.definition is two.definition is definition
    assert one.id != two.id
    assert (one.start, one.duration, two.start, two.duration) == (4.0, 0.8, 12.0, 1.2)


def test_old_timing_and_unsupported_transition_presets_are_not_public() -> None:
    with pytest.raises(TypeError):
        vestra.Crossfade(start=0, duration=1)  # type: ignore[call-arg]
    assert not hasattr(vestra, "FlashCut")
    assert not hasattr(vestra, "ZoomBlurTransition")


def test_transition_ownership_and_failure_atomicity() -> None:
    project, first, second = _project()
    other = vestra.Project(size=(8, 6), fps=10, duration=20).root.add(vestra.Image("other.png"))
    definition = vestra.Crossfade()
    with pytest.raises(ValueError, match="different"):
        project.root.transitions.add(first, first, definition, start=0, duration=1)
    with pytest.raises(ValueError, match="same composition"):
        project.root.transitions.add(first, other, definition, start=0, duration=1)
    assert project.root.transitions.items == ()
