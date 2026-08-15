from __future__ import annotations

import pytest

import vestra
from vestra.authoring import AuthoringError, ProjectBuilder, TransitionDefinition


def builder() -> tuple[ProjectBuilder, object, object]:
    project = ProjectBuilder(width=32, height=18, frame_rate=vestra.FrameRate(30, 1), output_path="out.mp4", duration=3)
    first_asset = project.add_image_asset("first.png")
    second_asset = project.add_image_asset("second.png")
    return project, project.add_image_clip(source=first_asset, start=0, duration=3, layer=0), project.add_image_clip(source=second_asset, start=0, duration=3, layer=1)


def test_generic_definition_serializes_as_a_placement() -> None:
    project, outgoing, incoming = builder()
    definition = TransitionDefinition(vestra.Crossfade().to_canonical())
    placement = project.transitions.add_transition(outgoing=outgoing, incoming=incoming, start=0, duration=1, definition=definition)
    assert placement.definition is definition
    assert placement.to_canonical()["definition"] == definition.to_canonical()
    assert "interpolation" not in placement.to_canonical()["definition"]


def test_reusable_generic_definition_gets_independent_placements() -> None:
    project, outgoing, incoming = builder()
    third_asset = project.add_image_asset("third.png")
    third = project.add_image_clip(source=third_asset, start=0, duration=3, layer=2)
    definition = TransitionDefinition(vestra.PushLeft().to_canonical())
    first = project.transitions.add_transition(outgoing=outgoing, incoming=incoming, start=0, duration=1, definition=definition)
    second = project.transitions.add_transition(outgoing=incoming, incoming=third, start=1, duration=0.5, definition=definition)
    assert first.id != second.id
    assert (first.start, first.duration) == (0.0, 1.0)
    assert (second.start, second.duration) == (1.0, 0.5)


def test_transition_creation_is_atomic_on_invalid_values() -> None:
    project, outgoing, incoming = builder()
    definition = TransitionDefinition(vestra.Crossfade().to_canonical())
    with pytest.raises(ValueError, match="duration"):
        project.transitions.add_transition(outgoing=outgoing, incoming=incoming, start=0, duration=0, definition=definition)
    assert project.transitions.items == ()
    assert project.transitions.add_transition(outgoing=outgoing, incoming=incoming, start=0, duration=1, definition=definition).id == "transition-000001"


def test_transition_ownership_is_atomic() -> None:
    project, outgoing, incoming = builder()
    definition = TransitionDefinition(vestra.Crossfade().to_canonical())
    with pytest.raises(AuthoringError):
        project.transitions.add_transition(outgoing=outgoing, incoming=outgoing, start=0, duration=1, definition=definition)
    assert project.transitions.items == ()
