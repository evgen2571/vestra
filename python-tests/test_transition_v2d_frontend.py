from __future__ import annotations

import pytest

import vestra
from vestra.authoring import ProjectBuilder, TransitionDefinition


def _project() -> tuple[vestra.Project, vestra.Layer, vestra.Layer]:
    project = vestra.Project(size=(8, 6), fps=10, duration=3)
    first = project.root.add(vestra.Image("first.png"), id="first")
    second = project.root.add(vestra.Image("second.png"), id="second")
    return project, first, second


def test_definition_reuse_keeps_timing_and_ids_on_placements() -> None:
    project, first, second = _project()
    third = project.root.add(vestra.Image("third.png"), id="third")
    fourth = project.root.add(vestra.Image("fourth.png"), id="fourth")
    push = vestra.PushLeft()

    first_placement = project.root.transitions.add(
        first, second, push, start=4, duration=0.8
    )
    second_placement = project.root.transitions.add(
        third, fourth, push, start=12, duration=1.2
    )

    assert first_placement.definition is push
    assert second_placement.definition is push
    assert first_placement.id != second_placement.id
    assert (first_placement.start, first_placement.duration) == (4.0, 0.8)
    assert (second_placement.start, second_placement.duration) == (12.0, 1.2)
    assert not {"start", "duration", "id"}.intersection(
        vars(push) if hasattr(push, "__dict__") else ()
    )


def test_old_timing_in_definition_syntax_is_rejected() -> None:
    with pytest.raises(TypeError):
        vestra.Crossfade(start=4, duration=1)  # type: ignore[call-arg]


def test_directional_push_has_no_silent_blur_parameter() -> None:
    push = vestra.DirectionalPush()
    assert not hasattr(push, "blur_radius")
    with pytest.raises(TypeError):
        vestra.DirectionalPush(blur_radius=2)  # type: ignore[call-arg]


@pytest.mark.parametrize(
    "definition", [vestra.Crossfade(), vestra.PushLeft(), vestra.ZoomCrossfade()]
)
def test_builtins_lower_through_generic_canonical_definition(
    definition: object,
) -> None:
    project, first, second = _project()
    project.root.transitions.add(first, second, definition, start=0.5, duration=1)  # type: ignore[arg-type]
    item = project.snapshot().to_dict()["visual"]["transitions"][0]
    assert "type" not in item["definition"]
    assert "interpolation" not in item["definition"]


def test_generic_authoring_definition_and_placement() -> None:
    project = ProjectBuilder(
        width=8,
        height=6,
        frame_rate=vestra.FrameRate(10, 1),
        output_path="out.mp4",
        duration=2,
    )
    asset = project.add_image_asset("image.png")
    outgoing = project.add_image_clip(source=asset, start=0, duration=2, layer=0)
    incoming = project.add_image_clip(source=asset, start=0, duration=2, layer=1)
    definition = TransitionDefinition(vestra.Crossfade().to_canonical())
    placement = project.transitions.add_transition(
        outgoing=outgoing,
        incoming=incoming,
        definition=definition,
        start=0,
        duration=1,
    )
    assert placement.definition is definition
    assert project.to_dict()["visual"]["transitions"][0]["start"] == 0.0


def test_easing_is_written_on_generated_channel_keyframes() -> None:
    definition = vestra.ZoomCrossfade(easing=vestra.Interpolation.EASE_IN_OUT)
    canonical = definition.to_canonical()
    assert "easing" not in canonical
    assert "interpolation" not in canonical
    assert (
        canonical["outgoing"]["opacity"]["keyframes"][0]["interpolation"]
        == "ease_in_out"
    )
    assert (
        canonical["incoming"]["scale_multiplier"]["keyframes"][1]["interpolation"]
        == "ease_in_out"
    )


def test_zoom_direction_presets_are_generic_scale_definitions() -> None:
    for definition in (vestra.ZoomIn(), vestra.ZoomOut()):
        canonical = definition.to_canonical()
        assert set(canonical["outgoing"]) == {"opacity", "scale_multiplier"}
        assert set(canonical["incoming"]) == {"opacity", "scale_multiplier"}


def test_duplicate_explicit_transition_id_is_rejected_atomically() -> None:
    project, first, second = _project()
    third = project.root.add(vestra.Image("third.png"), id="third")
    fourth = project.root.add(vestra.Image("fourth.png"), id="fourth")
    definition = vestra.Crossfade()

    project.root.transitions.add(
        first, second, definition, start=1, duration=1, id="fade"
    )
    with pytest.raises(ValueError, match="duplicate transition ID: 'fade'"):
        project.root.transitions.add(
            third, fourth, definition, start=4, duration=1, id="fade"
        )

    assert len(project.root.transitions.items) == 1
    assert project.root.transitions.items[0].id == "fade"


def test_explicit_transition_id_does_not_consume_automatic_sequence() -> None:
    project, first, second = _project()
    third = project.root.add(vestra.Image("third.png"), id="third")
    fourth = project.root.add(vestra.Image("fourth.png"), id="fourth")
    definition = vestra.Crossfade()

    project.root.transitions.add(
        first, second, definition, start=1, duration=1, id="custom"
    )
    automatic = project.root.transitions.add(
        third, fourth, definition, start=4, duration=1
    )

    assert automatic.id == "transition-000001"


def test_automatic_transition_id_skips_explicitly_occupied_generated_name() -> None:
    project, first, second = _project()
    third = project.root.add(vestra.Image("third.png"), id="third")
    fourth = project.root.add(vestra.Image("fourth.png"), id="fourth")
    definition = vestra.Crossfade()

    project.root.transitions.add(
        first, second, definition, start=1, duration=1, id="transition-000001"
    )
    automatic = project.root.transitions.add(
        third, fourth, definition, start=4, duration=1
    )

    assert automatic.id == "transition-000002"
