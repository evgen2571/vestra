from __future__ import annotations

from collections.abc import Hashable

import pytest

import vestra
import vestra.transitions as transition_module
from vestra.effects import GaussianBlur
from vestra.sources import Color
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


@pytest.mark.parametrize(
    ("convenience", "angle"),
    [
        (vestra.PushLeft, 180.0),
        (vestra.PushRight, 0.0),
        (vestra.PushUp, -90.0),
        (vestra.PushDown, 90.0),
    ],
)
def test_direction_presets_match_directional_push(
    convenience: type[vestra.DirectionalPush], angle: float
) -> None:
    assert (
        convenience(distance=1.25).to_canonical()
        == vestra.DirectionalPush(angle_degrees=angle, distance=1.25).to_canonical()
    )


def test_effect_presets_are_value_like_and_inspectable() -> None:
    first = vestra.BlurCrossfade(radius=4)
    second = vestra.BlurCrossfade(radius=4)
    assert first == second
    assert "BlurCrossfade" in repr(first)
    assert "radius=4.0" in repr(first)
    assert "keyframes" not in repr(first)


def test_zoom_direction_preset_names_describe_their_endpoints() -> None:
    zoom_in = vestra.ZoomIn(amount=0.8).to_canonical()
    zoom_out = vestra.ZoomOut(amount=1.2).to_canonical()
    assert zoom_in["incoming"]["scale_multiplier"]["keyframes"][0]["value"] == {
        "x": 0.8,
        "y": 0.8,
    }
    assert zoom_in["outgoing"]["scale_multiplier"]["keyframes"][-1]["value"] == {
        "x": 1.0,
        "y": 1.0,
    }
    assert zoom_out["outgoing"]["scale_multiplier"]["keyframes"][-1]["value"] == {
        "x": 1.2,
        "y": 1.2,
    }
    assert zoom_out["incoming"]["scale_multiplier"]["keyframes"][0]["value"] == {
        "x": 1.0,
        "y": 1.0,
    }


def test_convenience_preset_reprs_use_public_constructor_semantics() -> None:
    assert repr(vestra.PushLeft(distance=1.25)) == (
        "PushLeft(distance=1.25, easing=Interpolation.EASE_IN_OUT)"
    )
    assert repr(vestra.ZoomIn(amount=0.8)) == (
        "ZoomIn(amount=0.8, easing=Interpolation.EASE_IN_OUT)"
    )
    assert "outgoing_zoom" not in repr(vestra.ZoomIn())
    assert "incoming_start_zoom" not in repr(vestra.ZoomIn())


@pytest.mark.parametrize(
    "definition",
    [
        vestra.Crossfade(),
        vestra.PushLeft(distance=1.0),
        vestra.ZoomIn(amount=0.9),
        vestra.BlurCrossfade(radius=4.0),
    ],
)
def test_transition_definitions_are_explicitly_unhashable(
    definition: vestra.TransitionDefinition,
) -> None:
    assert type(definition).__hash__ is None
    assert not isinstance(definition, Hashable)
    with pytest.raises(TypeError):
        hash(definition)


def test_transition_definitions_preserve_value_equality() -> None:
    assert vestra.Crossfade() == vestra.Crossfade()
    assert vestra.PushLeft(distance=1.0) == vestra.PushLeft(distance=1.0)
    assert vestra.PushLeft(distance=1.0) != vestra.PushLeft(distance=0.5)


def test_custom_transition_is_explicitly_unhashable_and_value_equal() -> None:
    def make_custom() -> CustomTransition:
        return CustomTransition(
            outgoing=TransitionLayer(opacity=Animate(1.0, 0.0)),
            incoming=TransitionLayer(opacity=Animate(0.0, 1.0)),
        )

    custom = make_custom()
    assert type(custom).__hash__ is None
    assert not isinstance(custom, Hashable)
    with pytest.raises(TypeError):
        hash(custom)
    assert custom == make_custom()


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


def test_transition_module_exports_only_supported_public_names() -> None:
    assert set(transition_module.__all__) == {
        "InterpolationValue",
        "Animate",
        "TransitionLayer",
        "CustomTransition",
        "TransitionDefinition",
        "TransitionPlacement",
        "TransitionCollection",
        "Crossfade",
        "DirectionalPush",
        "PushLeft",
        "PushRight",
        "PushUp",
        "PushDown",
        "ZoomCrossfade",
        "ZoomIn",
        "ZoomOut",
        "BlurCrossfade",
        "ZoomBlurTransition",
        "WhipPanLeft",
        "WhipPanRight",
    }


def test_transition_layer_accepts_existing_effects_with_normalized_tracks() -> None:
    blur = GaussianBlur(0.0)
    blur.radius.keyframe(0.5, 12.0)
    custom = CustomTransition(
        outgoing=TransitionLayer(
            effects=[blur],
            opacity=Animate(1.0, 0.0),
        )
    )

    canonical = custom.to_canonical()
    assert canonical["outgoing"]["effects"][0]["type"] == "gaussian_blur"
    assert canonical["outgoing"]["effects"][0]["radius"]["keyframes"][0]["time"] == 0.5


def test_blur_crossfade_lowers_to_channels_and_existing_effect() -> None:
    canonical = vestra.BlurCrossfade().to_canonical()
    assert set(canonical["outgoing"]) == {"opacity", "effects"}
    assert canonical["incoming"]["effects"][0]["type"] == "gaussian_blur"


def test_effect_driven_presets_lower_without_native_preset_identity() -> None:
    definitions = (
        vestra.BlurCrossfade(),
        vestra.ZoomBlurTransition(),
        vestra.WhipPanLeft(),
        vestra.WhipPanRight(),
    )
    for definition in definitions:
        canonical = definition.to_canonical()
        assert "type" not in canonical
        assert any("effects" in presentation for presentation in canonical.values())


def test_effect_enabled_transition_validates_and_preserves_generic_schema() -> None:
    project, first, second = _project()
    blur = GaussianBlur(0.0)
    blur.radius.keyframe(0.5, 12.0)
    project.root.transitions.add(
        first,
        second,
        CustomTransition(outgoing=TransitionLayer(effects=[blur])),
        start=4,
        duration=2,
    )

    report = project.validate()
    assert report.is_valid
    transition = project.snapshot().to_dict()["visual"]["transitions"][0]
    assert transition["definition"]["outgoing"]["effects"][0]["type"] == "gaussian_blur"
    assert "preset" not in str(transition)


def test_nested_composition_transitions_are_owned_and_serialized_locally() -> None:
    project = vestra.Project(size=(8, 6), fps=10, duration=20)
    nested = project.root.group(start=10, duration=5, id="chapter")
    first = nested.child.add(vestra.Image("first.png"), id="first", start=0, duration=5)
    second = nested.child.add(
        vestra.Image("second.png"), id="second", start=0, duration=5
    )

    nested.child.transitions.add(first, second, vestra.Crossfade(), start=2, duration=1)

    snapshot = project.snapshot().to_dict()
    transitions = snapshot["visual"]["clips"][0]["source"]["transitions"]
    assert transitions[0]["start"] == 2.0
    assert transitions[0]["outgoing"] == "chapter/first"


def test_nested_transition_rejects_cross_scope_endpoints_atomically() -> None:
    project = vestra.Project(size=(8, 6), fps=10, duration=20)
    first_group = project.root.group(start=2, duration=5, id="first-group")
    second_group = project.root.group(start=8, duration=5, id="second-group")
    first = first_group.add(Color("#ff0000"), id="first", duration=5)
    second = first_group.add(Color("#00ff00"), id="second", duration=5)
    other = second_group.add(Color("#0000ff"), id="other", duration=5)

    first_group.child.transitions.add(
        first, second, vestra.Crossfade(), start=1, duration=1
    )
    before = first_group.child.transitions.items
    with pytest.raises(ValueError, match="same composition"):
        first_group.child.transitions.add(
            first, other, vestra.Crossfade(), start=1, duration=1
        )
    assert first_group.child.transitions.items == before


def test_nested_custom_transition_reuses_generic_channels() -> None:
    project = vestra.Project(size=(8, 6), fps=10, duration=20)
    nested = project.root.group(start=10, duration=5, id="chapter")
    first = nested.add(Color("#ff0000"), id="first", duration=5)
    second = nested.add(Color("#0000ff"), id="second", duration=5)
    definition = CustomTransition(
        outgoing=TransitionLayer(
            opacity=Animate(1.0, 0.0),
            position=Animate((0.0, 0.0), (1.0, 0.0)),
            scale=Animate(1.0, 1.1),
            rotation=Animate(0.0, 30.0),
        )
    )
    nested.child.transitions.add(first, second, definition, start=2, duration=1)

    transition = project.snapshot().to_dict()["visual"]["clips"][0]["source"][
        "transitions"
    ][0]
    assert set(transition["definition"]["outgoing"]) == {
        "opacity",
        "position_offset",
        "scale_multiplier",
        "rotation_offset_degrees",
    }


def test_nested_transition_uses_composition_local_time_at_runtime() -> None:
    project = vestra.Project(size=(2, 2), fps=10, duration=20)
    nested = project.root.group(start=10, duration=5, id="chapter")
    outgoing = nested.add(Color("#ff0000"), id="outgoing", duration=5)
    incoming = nested.add(Color("#0000ff"), id="incoming", duration=5)
    nested.child.transitions.add(
        outgoing, incoming, vestra.Crossfade(), start=2, duration=2
    )

    assert project.render_frame(11, backend="cpu").to_bytes()[:4] == bytes(
        (255, 0, 0, 255)
    )
    midpoint = project.render_frame(13, backend="cpu").to_bytes()[:4]
    assert midpoint not in {bytes((255, 0, 0, 255)), bytes((0, 0, 255, 255))}
    assert project.render_frame(14, backend="cpu").to_bytes()[:4] == bytes(
        (0, 0, 255, 255)
    )


def test_nested_transition_offsets_accumulate_through_deep_compositions() -> None:
    project = vestra.Project(size=(2, 2), fps=10, duration=20)
    outer = project.root.group(start=5, duration=10, id="outer")
    inner = outer.group(start=3, duration=5, id="inner")
    outgoing = inner.add(Color("#ff0000"), id="outgoing", duration=5)
    incoming = inner.add(Color("#0000ff"), id="incoming", duration=5)
    inner.child.transitions.add(
        outgoing, incoming, vestra.Crossfade(), start=2, duration=2
    )

    assert project.render_frame(9, backend="cpu").to_bytes()[:4] == bytes(
        (255, 0, 0, 255)
    )
    midpoint = project.render_frame(11, backend="cpu").to_bytes()[:4]
    assert midpoint not in {bytes((255, 0, 0, 255)), bytes((0, 0, 255, 255))}


def test_root_and_sibling_nested_scopes_reuse_one_definition_without_id_collisions() -> (
    None
):
    project = vestra.Project(size=(2, 2), fps=10, duration=20)
    root_first = project.root.add(Color("#ff0000"), id="root-first", duration=20)
    root_second = project.root.add(Color("#0000ff"), id="root-second", duration=20)
    first_group = project.root.group(start=2, duration=5, id="first-group")
    second_group = project.root.group(start=8, duration=5, id="second-group")
    first_a = first_group.add(Color("#ff0000"), id="a", duration=5)
    first_b = first_group.add(Color("#0000ff"), id="b", duration=5)
    second_a = second_group.add(Color("#ff0000"), id="a", duration=5)
    second_b = second_group.add(Color("#0000ff"), id="b", duration=5)
    definition = vestra.Crossfade()

    project.root.transitions.add(
        root_first, root_second, definition, start=1, duration=1, id="fade"
    )
    first_group.child.transitions.add(
        first_a, first_b, definition, start=1, duration=1, id="fade"
    )
    second_group.child.transitions.add(
        second_a, second_b, definition, start=1, duration=1, id="fade"
    )

    snapshot = project.snapshot().to_dict()
    assert snapshot["visual"]["transitions"][0]["id"] == "fade"
    assert [
        clip["source"]["transitions"][0]["id"]
        for clip in snapshot["visual"]["clips"][2:]
    ] == ["fade", "fade"]


def test_nested_effect_transition_uses_the_existing_effect_path() -> None:
    project = vestra.Project(size=(2, 2), fps=10, duration=20)
    nested = project.root.group(start=10, duration=5, id="chapter")
    outgoing = nested.add(Color("#ff0000"), id="outgoing", duration=5)
    incoming = nested.add(Color("#0000ff"), id="incoming", duration=5)
    nested.child.transitions.add(
        outgoing, incoming, vestra.BlurCrossfade(radius=4), start=2, duration=2
    )

    assert project.validate().is_valid
    effects = project.snapshot().to_dict()["visual"]["clips"][0]["source"][
        "transitions"
    ][0]["definition"]["outgoing"]["effects"]
    assert effects[0]["type"] == "gaussian_blur"


def test_nested_transition_fit_is_checked_against_local_child_lifetimes() -> None:
    project = vestra.Project(size=(2, 2), fps=10, duration=20)
    nested = project.root.group(start=10, duration=5, id="chapter")
    outgoing = nested.add(Color("#ff0000"), id="outgoing", duration=5)
    incoming = nested.add(Color("#0000ff"), id="incoming", start=2, duration=4)
    nested.child.transitions.add(
        outgoing, incoming, vestra.Crossfade(), start=5, duration=1
    )

    report = project.validate()
    assert not report.is_valid
    assert any(diagnostic.code == "VESTRA-TRANSITION-FIT" for diagnostic in report.errors)


def test_nested_transition_random_access_is_deterministic() -> None:
    project = vestra.Project(size=(2, 2), fps=10, duration=20)
    nested = project.root.group(start=10, duration=5, id="chapter")
    outgoing = nested.add(Color("#ff0000"), id="outgoing", duration=5)
    incoming = nested.add(Color("#0000ff"), id="incoming", duration=5)
    nested.child.transitions.add(
        outgoing, incoming, vestra.Crossfade(), start=2, duration=2
    )

    frames = [
        project.render_frame(time, backend="cpu").to_bytes() for time in (13, 9, 12, 13)
    ]
    assert frames[0] == frames[-1]


@pytest.mark.parametrize(
    "keyframes",
    [
        ((-0.1, 0.0), (1.0, 1.0)),
        ((0.0, 0.0), (1.1, 1.0)),
        ((float("nan"), 0.0), (1.0, 1.0)),
        ((0.0, 0.0), (float("inf"), 1.0)),
        ((0.0, 0.0),),
    ],
)
def test_animate_rejects_invalid_progress_values_and_keyframe_count(
    keyframes: tuple[tuple[float, float], ...],
) -> None:
    with pytest.raises((TypeError, ValueError)):
        Animate.keyframes(*keyframes)


@pytest.mark.parametrize(
    "value",
    [(float("nan"), 0.0), (float("inf"), 0.0), (0.0, float("-inf"))],
)
def test_animate_rejects_non_finite_position_values(value: tuple[float, float]) -> None:
    with pytest.raises(ValueError):
        CustomTransition(
            outgoing=TransitionLayer(
                position=Animate(value, (0.0, 0.0)),
            )
        )


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
