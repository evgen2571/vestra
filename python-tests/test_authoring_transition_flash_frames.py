"""Deterministic CPU frame checks for authored coordinated timeline features."""

from __future__ import annotations

from pathlib import Path
import subprocess

import video_editor
from video_editor import FrameRate
from video_editor.authoring import BlendMode, ProjectBuilder, Sizing


def project() -> tuple[ProjectBuilder, object, object]:
    authored = ProjectBuilder(width=8, height=6, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=2, base_directory=".")
    red = authored.add_image_asset("examples/assets/red.png")
    blue = authored.add_image_asset("examples/assets/blue.png")
    return authored, authored.add_image_clip(source=red, start=0, duration=2, layer=0, sizing=Sizing.cover()), authored.add_image_clip(source=blue, start=0, duration=2, layer=0, sizing=Sizing.cover())


def frames(authored: ProjectBuilder) -> video_editor.PreparedProject:
    assert authored.validate().is_valid
    return video_editor.Editor().prepare(authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU))


def test_crossfade_and_flash_have_stable_before_midpoint_and_after_pixels() -> None:
    authored, outgoing, incoming = project()
    authored.transitions.add_crossfade(outgoing=outgoing, incoming=incoming, start=0.5, duration=1)
    prepared = frames(authored)
    assert tuple(prepared.render_frame_number(0).to_bytes()[:4]) == (250, 1, 1, 255)
    assert tuple(prepared.render_frame_number(10).to_bytes()[:4]) == (63, 1, 128, 255)
    assert tuple(prepared.render_frame_number(15).to_bytes()[:4]) == (0, 0, 254, 255)
    authored.flashes.add(start=0.8, duration=0.2, colour="#ffffff", opacity=0.5, layer=2)
    flashed = frames(authored)
    assert tuple(flashed.render_frame_number(8).to_bytes()[:4]) == (189, 128, 166, 255)
    assert tuple(flashed.render_frame_number(10).to_bytes()[:4]) == (63, 1, 128, 255)


def test_every_canonical_transition_variant_changes_the_midpoint_as_designed() -> None:
    variants = (
        (lambda a, o, i: a.transitions.add_crossfade(outgoing=o, incoming=i, start=0.5, duration=1), (63, 1, 128, 255)),
        (lambda a, o, i: a.transitions.add_zoom_crossfade(outgoing=o, incoming=i, start=0.5, duration=1, outgoing_zoom=1.2, incoming_start_zoom=0.8), (125, 1, 1, 255)),
        (lambda a, o, i: a.transitions.add_flash_cut(outgoing=o, incoming=i, start=0.5, duration=1, colour="#ffffff", intensity=0.5), (128, 128, 255, 255)),
        (lambda a, o, i: a.transitions.add_directional_push(outgoing=o, incoming=i, start=0.5, duration=1, angle_degrees=90, distance=1, blur_radius=2), (125, 1, 1, 255)),
        (lambda a, o, i: a.transitions.add_zoom_blur(outgoing=o, incoming=i, start=0.5, duration=1, outgoing_zoom=1.2, incoming_start_zoom=0.8, blur_radius=2), (114, 1, 22, 255)),
    )
    for add, midpoint_pixel in variants:
        authored, outgoing, incoming = project()
        add(authored, outgoing, incoming)
        prepared = frames(authored)
        before = prepared.render_frame_number(4).to_bytes()
        at_start = prepared.render_frame_number(5).to_bytes()
        midpoint = prepared.render_frame_number(10).to_bytes()
        at_end = prepared.render_frame_number(15).to_bytes()
        after = prepared.render_frame_number(16).to_bytes()
        assert at_start == before
        assert midpoint != before
        assert midpoint != after
        assert at_end == after
        assert tuple(midpoint[:4]) == midpoint_pixel


def test_directionality_and_overlapping_flashes_are_deterministic() -> None:
    forward, outgoing, incoming = project()
    forward.transitions.add_directional_push(outgoing=outgoing, incoming=incoming, start=0.5, duration=1, angle_degrees=90, distance=1, blur_radius=0)
    forward_frame = frames(forward).render_frame_number(10).to_bytes()

    reverse, outgoing, incoming = project()
    reverse.transitions.add_directional_push(outgoing=incoming, incoming=outgoing, start=0.5, duration=1, angle_degrees=90, distance=1, blur_radius=0)
    assert forward_frame != frames(reverse).render_frame_number(10).to_bytes()

    flashes, _, _ = project()
    flashes.flashes.add(start=0.8, duration=0.3, colour="#ffffff", opacity=0.25, layer=2)
    flashes.flashes.add(start=0.8, duration=0.3, colour="#ff0000", opacity=0.25, layer=3)
    overlap = frames(flashes).render_frame_number(8).to_bytes()
    single, _, _ = project()
    single.flashes.add(start=0.8, duration=0.3, colour="#ffffff", opacity=0.25, layer=2)
    single_pixel = frames(single).render_frame_number(8).to_bytes()
    # Draw-key order is layer then ID. The red overlay follows white, so red wins.
    assert overlap[0] > overlap[1]
    assert overlap[0] > single_pixel[0]


def test_standalone_flash_fades_are_linear_and_colour_aware() -> None:
    authored = ProjectBuilder(width=4, height=4, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1,
                              background="#808080")
    authored.flashes.add(start=0.1, duration=0.6, colour="#ff0000", opacity=0.5,
                         fade_in=0.2, fade_out=0.2, layer=1)
    prepared = frames(authored)
    before = prepared.render_frame_number(0).to_bytes()
    fade_in = prepared.render_frame_number(2).to_bytes()
    peak = prepared.render_frame_number(4).to_bytes()
    fade_out = prepared.render_frame_number(6).to_bytes()
    after = prepared.render_frame_number(8).to_bytes()
    assert tuple(before[:4]) == (128, 128, 128, 255)
    assert tuple(after[:4]) == tuple(before[:4])
    # Linear alpha compositing over 128 grey: halfway to a 0.5 red flash is ~160/96/96.
    assert 156 <= fade_in[0] <= 161 and 94 <= fade_in[1] <= 98 and fade_in[1] == fade_in[2]
    assert 190 <= peak[0] <= 193 and 62 <= peak[1] <= 66 and peak[1] == peak[2]
    assert tuple(fade_out[:4]) == tuple(fade_in[:4])
    assert peak[0] > fade_in[0] > before[0]
    assert peak[1] < fade_in[1] < before[1]


def test_cpu_video_combines_transition_flash_effect_post_effect_and_blend_mode(tmp_path: Path) -> None:
    authored, outgoing, incoming = project()
    outgoing.blend_mode = BlendMode.SCREEN
    outgoing.effects.add_brightness(amount=0.05)
    authored.post_effects.add_contrast(amount=1.0)
    authored.transitions.add_crossfade(outgoing=outgoing, incoming=incoming, start=0.5, duration=1)
    authored.flashes.add(start=0.8, duration=0.1, colour="#ffffff", opacity=0.5, layer=2)
    output = tmp_path / "transition-flash.mp4"
    result = video_editor.Editor().render(authored.build(), video_editor.RenderRequest(output, backend=video_editor.BackendPreference.CPU, overwrite=True))
    probe = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "stream=codec_type,width,height,nb_frames", "-of", "csv=p=0", str(output)], check=True, capture_output=True, text=True)
    assert result.total_frames == 20
    assert output.is_file() and output.stat().st_size > 0
    assert probe.stdout.strip() == "video,8,6,20"
    assert not list(tmp_path.glob("*.tmp"))
