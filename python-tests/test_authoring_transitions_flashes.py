"""Phase 8C-C authoring contracts for canonical transitions and flashes."""

from __future__ import annotations

import pytest
from inspect import signature
from typing import get_type_hints

from vestra import FrameRate
from vestra.authoring import (
    AuthoringError, CrossfadeTransition, DirectionalPushTransition,
    FlashCutTransition, ImageClip, ProjectBuilder, ZoomBlurTransition, ZoomCrossfadeTransition,
)


def builder() -> tuple[ProjectBuilder, ImageClip, ImageClip]:
    project = ProjectBuilder(width=32, height=18, frame_rate=FrameRate(30, 1), output_path="out.mp4", duration=3)
    first = project.add_image_asset("first.png")
    second = project.add_image_asset("second.png")
    return project, project.add_image_clip(source=first, start=0, duration=3, layer=0), project.add_image_clip(source=second, start=0, duration=3, layer=0)


def test_all_transition_variants_serialize_with_exact_canonical_kind() -> None:
    project, outgoing, incoming = builder()
    variants = (
        project.transitions.add_crossfade(outgoing=outgoing, incoming=incoming, start=0, duration=0.2),
        project.transitions.add_zoom_crossfade(outgoing=outgoing, incoming=incoming, start=0.3, duration=0.2, outgoing_zoom=1.2, incoming_start_zoom=0.8),
        project.transitions.add_flash_cut(outgoing=outgoing, incoming=incoming, start=0.6, duration=0.2, colour="#ffffff", intensity=0.5),
        project.transitions.add_directional_push(outgoing=outgoing, incoming=incoming, start=0.9, duration=0.2, angle_degrees=90, distance=1, blur_radius=2),
        project.transitions.add_zoom_blur(outgoing=outgoing, incoming=incoming, start=1.2, duration=0.2, outgoing_zoom=1.2, incoming_start_zoom=0.8, blur_radius=2),
    )
    assert tuple(type(item) for item in variants) == (CrossfadeTransition, ZoomCrossfadeTransition, FlashCutTransition, DirectionalPushTransition, ZoomBlurTransition)
    visual = project.to_dict()["visual"]
    assert isinstance(visual, dict)
    assert [item["type"] for item in visual["transitions"]] == ["crossfade", "zoom_crossfade", "flash_cut", "directional_push", "zoom_blur"]


def test_transition_ownership_identity_and_native_validation_boundary() -> None:
    project, outgoing, incoming = builder()
    assert project.transitions is project.transitions
    transition = project.transitions.add_crossfade(outgoing=outgoing, incoming=incoming, start=0, duration=1)
    assert transition.id == "transition-000001"
    with pytest.raises(AuthoringError): project.transitions.add_crossfade(outgoing=outgoing, incoming=outgoing, start=1, duration=1)
    other, _, other_incoming = builder()
    with pytest.raises(AuthoringError): project.transitions.add_crossfade(outgoing=outgoing, incoming=other_incoming, start=1, duration=1)
    incoming.visible = False
    visual = project.to_dict()["visual"]
    assert isinstance(visual, dict)
    assert visual["transitions"][0]["incoming"] == incoming.id
    assert "MVP-TRANSITION-HIDDEN" in {item.code for item in project.validate().diagnostics}
    assert other.transitions.items == ()


def test_transition_fit_conflict_and_clip_mutation_keep_authored_data() -> None:
    project, outgoing, incoming = builder()
    transition = project.transitions.add_crossfade(outgoing=outgoing, incoming=incoming, start=1, duration=1)
    native_before = project.build()
    incoming.start = 1.5
    report = project.validate()
    assert "MVP-TRANSITION-FIT" in {item.code for item in report.diagnostics}
    assert transition.to_canonical()["start"] == 1.0
    assert native_before.to_dict()["visual"]["transitions"][0]["start"] == 1.0

    incoming.start = 0
    third_asset = project.add_image_asset("third.png")
    third = project.add_image_clip(source=third_asset, start=0, duration=3, layer=0)
    project.transitions.add_crossfade(outgoing=incoming, incoming=third, start=1.5, duration=1)
    assert "MVP-TRANSITION-CONFLICT" in {item.code for item in project.validate().diagnostics}


def test_failed_transition_creation_does_not_consume_id() -> None:
    project, outgoing, incoming = builder()
    with pytest.raises(ValueError): project.transitions.add_crossfade(outgoing=outgoing, incoming=incoming, start=0, duration=0)
    assert project.transitions.add_crossfade(outgoing=outgoing, incoming=incoming, start=0, duration=1).id == "transition-000001"
    with pytest.raises(AuthoringError):
        project.transitions.add_crossfade(outgoing=outgoing, incoming=incoming, start=1, duration=1, id="transition-000001")


def test_flashes_have_stable_ids_order_and_canonical_fields() -> None:
    project, _, _ = builder()
    assert project.flashes is project.flashes
    first = project.flashes.add(start=0, duration=0.5, colour="#ffffff", opacity=0.25, fade_in=0.1, fade_out=0.2, layer=4)
    second = project.flashes.add(start=1, duration=0.5, colour="#ff0000")
    assert (first.id, second.id) == ("flash-000001", "flash-000002")
    visual = project.to_dict()["visual"]
    assert isinstance(visual, dict)
    assert [flash["id"] for flash in visual["flashes"]] == [first.id, second.id]
    with pytest.raises(AttributeError): first.id = "nope"  # type: ignore[misc]
    with pytest.raises(ValueError): project.flashes.add(start=2, duration=0, colour="#ffffff")
    assert project.flashes.add(start=2, duration=0.1, colour="#ffffff").id == "flash-000003"
    with pytest.raises(AuthoringError): project.flashes.add(start=2.5, duration=0.1, colour="#ffffff", id="flash-000001")


def test_flash_snapshot_and_failed_explicit_creation_are_transactional() -> None:
    project, _, _ = builder()
    before = project.flashes.add(start=0, duration=0.5, colour="#ffffff", id="flash-a")
    native_before = project.build()
    with pytest.raises(ValueError): project.flashes.add(start=1, duration=0, colour="#ffffff", id="flash-b")
    assert project.flashes.add(start=1, duration=0.1, colour="#ffffff", id="flash-b").id == "flash-b"
    before.opacity = 0.25
    assert native_before.to_dict()["visual"]["flashes"][0]["opacity"] == 1.0


def test_public_factory_signatures_and_hints_expose_no_owner_state() -> None:
    hints = get_type_hints(type(builder()[0].transitions).add_crossfade)
    flash_hints = get_type_hints(type(builder()[0].flashes).add)
    assert hints["return"] is CrossfadeTransition
    assert flash_hints["return"].__name__ == "Flash"
    assert "_Owner" not in str(signature(type(builder()[0].transitions).add_crossfade))
    assert "_IdAllocator" not in str(signature(type(builder()[0].flashes).add))
