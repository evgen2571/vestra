from __future__ import annotations

import pytest

import vestra
from vestra.transitions import Animate, CustomTransition, TransitionLayer


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
        ["opacity"],
        ["position_offset"],
        ["opacity", "scale_multiplier"],
    ]
    assert all("type" not in item["definition"] for item in transitions)


def test_direction_presets_follow_native_position_conventions() -> None:
    left = vestra.PushLeft().to_canonical()["outgoing"]["position_offset"]["keyframes"][
        -1
    ]["value"]
    right = vestra.PushRight().to_canonical()["outgoing"]["position_offset"][
        "keyframes"
    ][-1]["value"]
    assert left["x"] == -1.0 and left["y"] == pytest.approx(0.0)
    assert right["x"] == 1.0 and right["y"] == pytest.approx(0.0)
    assert vestra.PushUp().to_canonical()["outgoing"]["position_offset"]["keyframes"][
        -1
    ]["value"]["y"] == pytest.approx(-1.0)
    assert vestra.PushDown().to_canonical()["outgoing"]["position_offset"]["keyframes"][
        -1
    ]["value"]["y"] == pytest.approx(1.0)


def test_definition_reuse_does_not_share_placement_identity_or_timing() -> None:
    project, first, second = _project()
    third = project.root.add(vestra.Image("third.png"), id="third")
    fourth = project.root.add(vestra.Image("fourth.png"), id="fourth")
    definition = vestra.PushLeft()
    one = project.root.transitions.add(first, second, definition, start=4, duration=0.8)
    two = project.root.transitions.add(
        third, fourth, definition, start=12, duration=1.2
    )
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
    other = vestra.Project(size=(8, 6), fps=10, duration=20).root.add(
        vestra.Image("other.png")
    )
    definition = vestra.Crossfade()
    with pytest.raises(ValueError, match="different"):
        project.root.transitions.add(first, first, definition, start=0, duration=1)
    with pytest.raises(ValueError, match="same composition"):
        project.root.transitions.add(first, other, definition, start=0, duration=1)
    assert project.root.transitions.items == ()


def test_custom_transition_supports_one_sided_and_rejects_empty_definitions() -> None:
    outgoing_only = CustomTransition(
        outgoing=TransitionLayer(opacity=Animate(1.0, 0.0)),
    )
    incoming_only = CustomTransition(
        incoming=TransitionLayer(opacity=Animate(0.0, 1.0)),
    )

    assert set(outgoing_only.to_canonical()) == {"outgoing", "incoming"}
    assert outgoing_only.to_canonical()["incoming"] == {}
    assert incoming_only.to_canonical()["outgoing"] == {}
    with pytest.raises(ValueError, match="at least one channel"):
        CustomTransition()


def test_animate_two_point_and_multikeyframe_lower_to_normalized_tracks() -> None:
    two_point = Animate(1.0, 0.0, easing=vestra.Interpolation.EASE_OUT)
    multi = Animate.keyframes(
        (0.0, 1.0),
        (0.25, 0.8, vestra.Interpolation.EASE_IN),
        (0.75, 0.9),
        (1.0, 0.0, vestra.Interpolation.EASE_OUT),
    )
    custom = CustomTransition(
        outgoing=TransitionLayer(opacity=two_point),
        incoming=TransitionLayer(opacity=multi),
    )

    canonical = custom.to_canonical()
    assert canonical["outgoing"]["opacity"]["keyframes"] == [
        {"progress": 0.0, "value": 1.0, "interpolation": "ease_out"},
        {"progress": 1.0, "value": 0.0, "interpolation": "ease_out"},
    ]
    assert [
        keyframe["progress"]
        for keyframe in canonical["incoming"]["opacity"]["keyframes"]
    ] == [0.0, 0.25, 0.75, 1.0]
    assert (
        canonical["incoming"]["opacity"]["keyframes"][1]["interpolation"] == "ease_in"
    )
    assert canonical["incoming"]["opacity"]["keyframes"][2]["interpolation"] == "linear"


def test_custom_default_easing_applies_only_to_unspecified_channels() -> None:
    custom = CustomTransition(
        default_easing=vestra.Interpolation.EASE_IN_OUT,
        outgoing=TransitionLayer(
            opacity=Animate(1.0, 0.0),
            position=Animate.keyframes((0.0, (0.0, 0.0)), (1.0, (1.0, -2.0))),
            scale=Animate(1.0, 1.1, easing=vestra.Interpolation.EASE_OUT),
            rotation=Animate(0.0, 45.0),
        ),
    )

    canonical = custom.to_canonical()
    assert (
        canonical["outgoing"]["opacity"]["keyframes"][1]["interpolation"]
        == "ease_in_out"
    )
    assert canonical["outgoing"]["position_offset"]["keyframes"][1]["value"] == {
        "x": 1.0,
        "y": -2.0,
    }
    assert (
        canonical["outgoing"]["scale_multiplier"]["keyframes"][1]["interpolation"]
        == "ease_out"
    )
    assert (
        canonical["outgoing"]["rotation_offset_degrees"]["keyframes"][1][
            "interpolation"
        ]
        == "ease_in_out"
    )
    assert "default_easing" not in canonical


@pytest.mark.parametrize(
    "keyframes",
    [
        ((0.1, 0.0), (1.0, 1.0)),
        ((0.0, 0.0), (0.9, 1.0)),
        ((0.0, 0.0), (0.5, 1.0), (0.5, 0.0)),
    ],
)
def test_animate_rejects_invalid_normalized_keyframes(
    keyframes: tuple[tuple[float, float], ...],
) -> None:
    with pytest.raises(ValueError):
        CustomTransition(
            outgoing=TransitionLayer(opacity=Animate.keyframes(*keyframes))
        )


def test_custom_transition_rejects_invalid_channel_values() -> None:
    with pytest.raises(ValueError, match="opacity"):
        CustomTransition(outgoing=TransitionLayer(opacity=Animate(-0.1, 0.0)))
    with pytest.raises(ValueError, match="scale"):
        CustomTransition(outgoing=TransitionLayer(scale=Animate(1.0, 0.0)))
    with pytest.raises(ValueError, match="rotation"):
        CustomTransition(outgoing=TransitionLayer(rotation=Animate(0.0, float("inf"))))


def test_custom_transition_reuses_one_immutable_definition_across_placements() -> None:
    project, first, second = _project()
    third = project.root.add(vestra.Image("third.png"), id="third")
    fourth = project.root.add(vestra.Image("fourth.png"), id="fourth")
    custom = CustomTransition(outgoing=TransitionLayer(opacity=Animate(1.0, 0.0)))

    one = project.root.transitions.add(first, second, custom, start=4, duration=0.8)
    two = project.root.transitions.add(third, fourth, custom, start=12, duration=1.6)

    assert one.definition is two.definition is custom
    assert (one.start, one.duration, two.start, two.duration) == (4.0, 0.8, 12.0, 1.6)
    assert not {"start", "duration", "id"}.intersection(
        custom.__dict__ if hasattr(custom, "__dict__") else ()
    )


def test_custom_transition_lowers_through_schema_v3_generic_definition() -> None:
    project, first, second = _project()
    custom = CustomTransition(
        outgoing=TransitionLayer(
            opacity=Animate(1.0, 0.0),
            position=Animate((0.0, 0.0), (1.0, -1.0)),
            scale=Animate(1.0, 1.1),
            rotation=Animate(0.0, 30.0),
        ),
        incoming=TransitionLayer(opacity=Animate(0.0, 1.0)),
    )
    project.root.transitions.add(
        first, second, custom, start=10, duration=1.25, id="cinematic"
    )

    snapshot = project.snapshot().to_dict()
    transition = snapshot["visual"]["transitions"][0]
    assert snapshot["schema_version"] == 3
    assert transition["id"] == "cinematic"
    assert transition["start"] == 10.0
    assert transition["duration"] == 1.25
    assert set(transition["definition"]["outgoing"]) == {
        "opacity",
        "position_offset",
        "scale_multiplier",
        "rotation_offset_degrees",
    }
    assert "CustomTransition" not in str(transition)
    assert "default_easing" not in transition["definition"]
