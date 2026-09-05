"""CPU rendering and validation coverage for Python-authored canonical presets."""

from __future__ import annotations

from pathlib import Path
import subprocess

import pytest

import vestra
from vestra import Crossfade, FrameRate
from vestra.authoring import BlendMode, Interpolation, Point, ProjectBuilder, Sizing


def _preset_project(kind: str) -> tuple[ProjectBuilder, object]:
    builder = ProjectBuilder(
        width=32, height=24, frame_rate=FrameRate(20, 1), output_path="preset.mp4",
        duration=3, base_directory=Path.cwd(),
    )
    asset = builder.add_image_asset("tests/assets/wgpu-small-rgba.png")
    clip = builder.add_image_clip(
        source=asset, start=1, duration=1.5, layer=0, sizing=Sizing.stretch(width=16, height=12),
    )
    arguments: dict[str, object] = {"duration": 0.8}
    if kind in {"impact", "heavy_impact"}:
        arguments["seed"] = 7
    getattr(clip.presets, f"apply_{kind}")(**arguments)
    assert builder.validate().is_valid
    return builder, clip


def _frames(builder: ProjectBuilder) -> vestra.PreparedProject:
    return vestra.Editor().prepare(
        builder.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )


def _pixel(frame: bytes, x: int, y: int) -> tuple[int, int, int, int]:
    offset = (y * 32 + x) * 4
    return tuple(frame[offset:offset + 4])  # type: ignore[return-value]


def test_slow_drift_moves_and_scales_in_clip_local_time() -> None:
    builder, _ = _preset_project("slow_drift")
    prepared = _frames(builder)
    before = prepared.render_frame_number(19).to_bytes()
    start = prepared.render_frame_number(20).to_bytes()
    midpoint = prepared.render_frame_number(28).to_bytes()
    end = prepared.render_frame_number(36).to_bytes()
    assert _pixel(before, 12, 8) == (0, 0, 0, 255)
    # The source-defined drift starts at the clip's project time, not at t=0,
    # and its position-plus-scale contribution changes an interior region.
    assert _pixel(start, 12, 8) == (4, 1, 1, 255)
    assert _pixel(midpoint, 12, 8) == (4, 1, 1, 255)
    assert _pixel(end, 12, 8) == (2, 0, 0, 255)


def test_zoom_punch_has_a_peak_then_returns_to_the_source_extent() -> None:
    builder, _ = _preset_project("zoom_punch")
    prepared = _frames(builder)
    start = prepared.render_frame_number(20).to_bytes()
    peak = prepared.render_frame_number(24).to_bytes()
    returned = prepared.render_frame_number(28).to_bytes()
    after = prepared.render_frame_number(36).to_bytes()
    assert _pixel(start, 12, 8) == (2, 0, 0, 255)
    assert _pixel(peak, 12, 8) == (234, 29, 32, 255)
    assert _pixel(returned, 12, 8) == _pixel(after, 12, 8) == (2, 0, 0, 255)


def test_impact_uses_deterministic_generated_chromatic_response() -> None:
    builder, _ = _preset_project("impact")
    prepared = _frames(builder)
    start = prepared.render_frame_number(20).to_bytes()
    pulse = prepared.render_frame_number(22).to_bytes()
    settled = prepared.render_frame_number(36).to_bytes()
    assert _pixel(start, 12, 8) == (2, 0, 0, 255)
    # Seeded native impact generates a chromatic/tint pulse at the active interval.
    assert _pixel(pulse, 12, 8) == (64, 102, 64, 255)
    assert _pixel(settled, 12, 8) == (2, 0, 0, 255)


def test_heavy_impact_is_source_backed_and_stronger_than_impact() -> None:
    ordinary, _ = _preset_project("impact")
    heavy, _ = _preset_project("heavy_impact")
    ordinary_frame = _frames(ordinary).render_frame_number(22).to_bytes()
    heavy_frame = _frames(heavy).render_frame_number(22).to_bytes()
    assert _pixel(heavy_frame, 12, 8) == (74, 51, 76, 255)
    assert heavy_frame != ordinary_frame


def test_focus_reveal_progresses_from_blur_to_focused_source() -> None:
    builder, _ = _preset_project("focus_reveal")
    prepared = _frames(builder)
    start = prepared.render_frame_number(20).to_bytes()
    midpoint = prepared.render_frame_number(24).to_bytes()
    focused = prepared.render_frame_number(28).to_bytes()
    assert _pixel(start, 14, 10) == _pixel(midpoint, 14, 10) == (118, 83, 91, 255)
    assert _pixel(focused, 14, 10) == (254, 255, 255, 255)


@pytest.mark.parametrize(
    ("start", "duration", "pointer"),
    [(2.1, None, "/visual/clips/0/preset/start"), (1.9, 1.0, "/visual/clips/0/preset/duration"), (0.0, 3.0, "/visual/clips/0/preset/duration")],
)
def test_preset_timing_remains_authored_for_native_interval_validation(
    start: float, duration: float | None, pointer: str,
) -> None:
    builder = ProjectBuilder(width=8, height=6, frame_rate=FrameRate(10, 1), output_path="out.mp4")
    asset = builder.add_image_asset("tests/assets/wgpu-small-rgba.png")
    clip = builder.add_image_clip(source=asset, start=0, duration=2, layer=0)
    clip.presets.apply_zoom_punch(start=start, duration=duration)
    preset = builder.to_dict()["visual"]["clips"][0]["preset"]  # type: ignore[index]
    assert preset.get("start", 0.0) == start and preset.get("duration") == duration
    diagnostic = builder.validate().diagnostics[0]
    assert diagnostic.code == "VESTRA-EFFECT-INTERVAL" and diagnostic.pointer == pointer


def test_complete_authoring_project_renders_frames_and_video_with_audio(tmp_path: Path) -> None:
    builder = ProjectBuilder(
        width=32, height=24, frame_rate=FrameRate(20, 1), output_path="complete.mp4",
        duration=2.5, base_directory=Path.cwd(), background="#101010",
    )
    image = builder.add_image_asset("tests/assets/wgpu-small-rgba.png")
    blue = builder.add_image_asset("examples/assets/blue.png")
    audio = builder.add_audio_asset("examples/assets/tone.wav")
    builder.add_solid_color_clip(colour="#102030", start=0, duration=2.5, layer=-1)
    outgoing = builder.add_image_clip(source=image, start=0, duration=2, layer=0, sizing=Sizing.stretch(width=16, height=12))
    incoming = builder.add_image_clip(source=blue, start=1.5, duration=1, layer=1, sizing=Sizing.cover())
    outgoing.transform.position.keyframe(time=0, value=Point(0.5, 0.5))
    outgoing.transform.position.keyframe(time=0.5, value=Point(0.45, 0.5), interpolation=Interpolation.EASE_IN_OUT)
    outgoing.opacity.keyframe(time=0, value=0.7)
    outgoing.set_crop(outgoing.crop.base_value)
    outgoing.crop.keyframe(time=0.5, value=outgoing.crop.base_value)
    outgoing.effects.add_brightness(amount=0.05).amount.keyframe(time=0.5, value=0.15)
    outgoing.blend_mode = BlendMode.SCREEN
    outgoing.presets.apply_impact(seed=7, duration=0.4)
    builder.timeline.shift_clip(incoming, delta=0.25)
    builder.transitions.add_transition(outgoing=outgoing, incoming=incoming, definition=Crossfade().to_canonical(), start=1.75, duration=0.25)
    builder.flashes.add(start=1.8, duration=0.1, colour="#ffffff", opacity=0.4, layer=3)
    builder.post_effects.add_contrast(amount=1.0)
    builder.output_audio = True
    builder.audio.add_track(id="music").add_clip(asset=audio, start=0, trim_end=0.2)
    assert builder.validate().is_valid
    native = builder.build()
    prepared = _frames(builder)
    assert prepared.render_frame_number(2).to_bytes() != prepared.render_frame_number(36).to_bytes()
    assert prepared.render_frame_number(36).to_bytes() != prepared.render_frame_number(40).to_bytes()
    output = tmp_path / "complete.mp4"
    result = vestra.Editor().render(
        native, vestra.RenderRequest(output, backend=vestra.BackendPreference.CPU, overwrite=True),
    )
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "stream=codec_type,width,height,nb_frames,duration", "-of", "csv=p=0", str(output)],
        check=True, capture_output=True, text=True,
    )
    assert result.total_frames == 50 and output.is_file() and output.stat().st_size > 0
    assert "video,32,24,2.500000,50" in probe.stdout and "audio" in probe.stdout
    assert not list(tmp_path.glob("*.tmp"))
