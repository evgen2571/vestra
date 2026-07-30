"""CPU checks that authoring delegates animation evaluation to the native engine."""

from pathlib import Path
import subprocess

import pytest

import video_editor
from video_editor import FrameRate
from video_editor.authoring import Crop, CubicBezier, Interpolation, Point, ProjectBuilder, Sizing


def animated_opacity(interpolation: Interpolation | CubicBezier) -> bytes:
    authored = ProjectBuilder(
        width=4, height=4, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1,
        background="#000000",
    )
    clip = authored.add_solid_color_clip(colour="#ffffff", start=0, duration=1, layer=0)
    clip.opacity.keyframe(time=0, value=0)
    clip.opacity.keyframe(time=1, value=1, interpolation=interpolation)
    prepared = video_editor.Editor().prepare(
        authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    )
    return prepared.render_frame_number(5).to_bytes()[:4]


def test_cpu_frame_evaluates_named_interpolation_and_bezier() -> None:
    linear = animated_opacity(Interpolation.LINEAR)[0]
    assert 120 <= linear <= 135
    assert animated_opacity(Interpolation.HOLD)[0] == 0
    assert animated_opacity(Interpolation.EASE_IN)[0] < linear
    assert animated_opacity(Interpolation.EASE_OUT)[0] > linear
    assert animated_opacity(Interpolation.EASE_IN_OUT)[0] in range(120, 136)
    bezier = animated_opacity(CubicBezier(0.42, 0, 1, 1))[0]
    assert bezier < linear


def test_cpu_frame_has_complete_track_lifecycle_semantics() -> None:
    authored = ProjectBuilder(
        width=4, height=4, frame_rate=FrameRate(4, 1), output_path="out.mp4", duration=2,
        background="#000000",
    )
    clip = authored.add_solid_color_clip(colour="#ffffff", start=0, duration=2, layer=0, opacity=0.2)
    clip.opacity.keyframe(time=0.5, value=0)
    clip.opacity.keyframe(time=1, value=1, interpolation=Interpolation.LINEAR)
    prepared = video_editor.Editor().prepare(
        authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    )
    # Before first, at first, midway, at final, and after final respectively.
    values = [prepared.render_frame_number(frame).to_bytes()[0] for frame in range(5)]
    assert values == [51, 51, 0, 128, 255]

    single = ProjectBuilder(
        width=4, height=4, frame_rate=FrameRate(4, 1), output_path="out.mp4", duration=2,
        background="#000000",
    )
    single_clip = single.add_solid_color_clip(colour="#ffffff", start=0, duration=2, layer=0, opacity=0.2)
    single_clip.opacity.keyframe(time=0.5, value=0.8)
    single_prepared = video_editor.Editor().prepare(
        single.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    )
    assert [single_prepared.render_frame_number(frame).to_bytes()[0] for frame in (0, 2, 5)] == [51, 204, 204]


def test_cpu_frame_uses_clip_local_keyframes_after_clip_start_mutation() -> None:
    authored = ProjectBuilder(
        width=4, height=4, frame_rate=FrameRate(2, 1), output_path="out.mp4", duration=4,
        background="#000000",
    )
    clip = authored.add_solid_color_clip(colour="#ffffff", start=2, duration=2, layer=0, opacity=0)
    clip.opacity.keyframe(time=0.5, value=1)
    native_before = authored.build()
    clip.start = 3
    assert clip.opacity.keyframes[0].time == 0.5
    assert native_before.to_dict()["visual"]["clips"][0]["start"] == 2.0
    prepared = video_editor.Editor().prepare(
        authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    )
    assert prepared.render_frame_number(5).to_bytes()[0] == 0
    assert prepared.render_frame_number(7).to_bytes()[0] == 255


def test_cpu_frames_change_for_each_animated_image_track() -> None:
    def render(mutate: object | None = None) -> bytes:
        authored = ProjectBuilder(
            width=8, height=6, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1,
            base_directory=Path.cwd(),
        )
        image = authored.add_image_asset("tests/assets/wgpu-small-rgba.png")
        clip = authored.add_image_clip(
            source=image, start=0, duration=1, layer=0, sizing=Sizing.stretch(width=4, height=3),
        )
        if mutate is not None:
            mutate(clip)
        return video_editor.Editor().prepare(
            authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
        ).render_frame_number(5).to_bytes()

    baseline = render()

    def position(clip: object) -> None:
        clip.transform.position.keyframe(time=0, value=Point(0.5, 0.5))
        clip.transform.position.keyframe(time=1, value=Point(0.25, 0.25))

    def anchor(clip: object) -> None:
        clip.transform.anchor.keyframe(time=0, value=Point(0.5, 0.5))
        clip.transform.anchor.keyframe(time=1, value=Point(0, 0))

    def scale(clip: object) -> None:
        clip.transform.scale.keyframe(time=0, value=Point(1, 1))
        clip.transform.scale.keyframe(time=1, value=Point(0.5, 0.5))

    def rotation(clip: object) -> None:
        clip.transform.rotation_degrees.keyframe(time=0, value=0)
        clip.transform.rotation_degrees.keyframe(time=1, value=45)

    def crop(clip: object) -> None:
        clip.set_crop(Crop(0, 0, 1, 1))
        clip.crop.keyframe(time=0, value=Crop(0, 0, 1, 1))
        clip.crop.keyframe(time=1, value=Crop(0.5, 0, 0.5, 1))

    for mutate in (position, anchor, scale, rotation, crop):
        assert render(mutate) != baseline


def test_cpu_video_renders_authored_animation_without_audio(tmp_path: Path) -> None:
    authored = ProjectBuilder(
        width=160, height=90, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1,
        background="#000000", base_directory=Path.cwd(),
    )
    image = authored.add_image_asset("examples/assets/red.png")
    clip = authored.add_image_clip(source=image, start=0, duration=1, layer=0)
    clip.transform.scale.keyframe(time=0, value=clip.transform.scale.base_value)
    clip.transform.scale.keyframe(time=1, value=type(clip.transform.scale.base_value)(1.2, 1.2), interpolation=Interpolation.EASE_IN_OUT)
    clip.opacity.keyframe(time=0, value=0, interpolation=CubicBezier(0.25, 0.1, 0.25, 1))
    clip.opacity.keyframe(time=0.5, value=1)
    assert authored.validate().is_valid
    output = tmp_path / "animated.mp4"
    result = video_editor.Editor().render(authored.build(), video_editor.RenderRequest(
        output, backend=video_editor.BackendPreference.CPU, overwrite=True,
    ))
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "stream=codec_type,nb_frames,duration", "-of", "csv=p=0", str(output)],
        check=True, capture_output=True, text=True,
    )
    assert result.total_frames == 10 and output.is_file() and output.stat().st_size > 0
    assert probe.stdout.splitlines()[0] == "video,1.000000,10"
    assert not list(tmp_path.glob("*.tmp"))
