"""Canonical preset and explicit timeline helper coverage."""

from __future__ import annotations

import inspect
import typing

import pytest

from vestra import FrameRate
from vestra.authoring import AuthoringError, PresetCollection, ProjectBuilder, Sizing, Timeline


def builder() -> tuple[ProjectBuilder, object]:
    authored = ProjectBuilder(width=8, height=6, frame_rate=FrameRate(10, 1), output_path="out.mp4")
    asset = authored.add_image_asset("fixtures/wgpu-small-rgba.png")
    return authored, authored.add_image_clip(source=asset, start=1, duration=2, layer=0, sizing=Sizing.cover())


@pytest.mark.parametrize("name,kwargs,expected", [
    ("apply_slow_drift", {}, {"type": "slow_drift", "intensity": 1.0}),
    ("apply_zoom_punch", {"intensity": 0.5, "start": 0.1, "duration": 0.2}, {"type": "zoom_punch", "intensity": 0.5, "start": 0.1, "duration": 0.2}),
    ("apply_impact", {"seed": 7}, {"type": "impact", "intensity": 1.0, "seed": 7}),
    ("apply_heavy_impact", {"seed": 8}, {"type": "heavy_impact", "intensity": 1.0, "seed": 8}),
    ("apply_focus_reveal", {}, {"type": "focus_reveal", "intensity": 1.0}),
])
def test_every_canonical_preset_serializes_without_python_expansion(name: str, kwargs: dict[str, object], expected: dict[str, object]) -> None:
    authored, clip = builder()
    preset = getattr(clip.presets, name)(**kwargs)
    assert preset.to_canonical() == expected
    assert authored.to_dict()["visual"]["clips"][0]["preset"] == expected
    assert authored.validate().is_valid


def test_preset_validation_and_repetition_are_transactional() -> None:
    authored, clip = builder()
    before = authored.to_dict()
    with pytest.raises(TypeError):
        clip.presets.apply_impact(seed=True)
    assert authored.to_dict() == before
    clip.presets.apply_slow_drift()
    with pytest.raises(AuthoringError):
        clip.presets.apply_focus_reveal()
    assert clip.presets.current is not None and clip.presets.current.kind == "slow_drift"


def test_timeline_shifts_only_explicit_clips_and_preserves_local_tracks() -> None:
    authored, first = builder()
    second = authored.add_image_clip(source=first.source, start=4, duration=1, layer=0)
    first.opacity.keyframe(time=0.5, value=0.5)
    authored.timeline.shift_clips([first, first, second], delta=2)
    assert (first.start, second.start) == (3.0, 6.0)
    assert first.opacity.keyframes[0].time == 0.5
    before = authored.to_dict()
    with pytest.raises(ValueError):
        authored.timeline.shift_clips([first, second], delta=-10)
    assert authored.to_dict() == before


def test_crossfade_helper_uses_existing_overlap_without_moving_clips() -> None:
    authored, first = builder()
    second = authored.add_image_clip(source=first.source, start=2, duration=2, layer=0)
    transition = authored.timeline.add_crossfade_between(first, second, duration=1)
    assert transition.start == 2 and (first.start, second.start) == (1, 2)
    with pytest.raises(ValueError):
        authored.timeline.add_crossfade_between(first, second, duration=2)


def test_readme_shift_then_crossfade_workflow_is_valid() -> None:
    authored = ProjectBuilder(width=8, height=6, frame_rate=FrameRate(10, 1), output_path="out.mp4")
    asset = authored.add_image_asset("fixtures/wgpu-small-rgba.png")
    outgoing = authored.add_image_clip(source=asset, start=0, duration=1, layer=0, sizing=Sizing.cover())
    incoming = authored.add_image_clip(source=asset, start=0.5, duration=1, layer=1, sizing=Sizing.cover())

    authored.timeline.shift_clip(incoming, delta=0.25)
    transition = authored.timeline.add_crossfade_between(outgoing, incoming, duration=0.25)

    assert transition.start == 0.75
    assert authored.validate().is_valid


def test_public_preset_and_timeline_annotations_and_signatures_hide_owner_machinery() -> None:
    methods = (
        PresetCollection.apply_slow_drift,
        PresetCollection.apply_zoom_punch,
        PresetCollection.apply_impact,
        PresetCollection.apply_heavy_impact,
        PresetCollection.apply_focus_reveal,
        Timeline.shift_clip,
        Timeline.shift_clips,
        Timeline.add_crossfade_between,
    )
    for method in methods:
        hints = typing.get_type_hints(method)
        assert hints
        signature = str(inspect.signature(method))
        assert "_Owner" not in signature and "_IdAllocator" not in signature
    with pytest.raises(TypeError, match="owned"):
        PresetCollection()
    with pytest.raises(TypeError, match="owned"):
        Timeline()
