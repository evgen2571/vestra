"""CPU checks that authoring delegates animation evaluation to the native engine."""

from pathlib import Path
import subprocess

import pytest

import vestra
from vestra import FrameRate
from vestra.authoring import Crop, CubicBezier, Interpolation, Point, ProjectBuilder, Sizing


def animated_opacity(interpolation: Interpolation | CubicBezier, frame_number: int = 5) -> bytes:
    authored = ProjectBuilder(
        width=4, height=4, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1,
        background="#000000",
    )
    clip = authored.add_solid_color_clip(colour="#ffffff", start=0, duration=1, layer=0)
    clip.opacity.keyframe(time=0, value=0)
    clip.opacity.keyframe(time=1, value=1, interpolation=interpolation)
    prepared = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    return prepared.render_frame_number(frame_number).to_bytes()[:4]


def test_cpu_frame_evaluates_named_interpolation_and_bezier() -> None:
    linear = animated_opacity(Interpolation.LINEAR)[0]
    assert 120 <= linear <= 135
    assert animated_opacity(Interpolation.HOLD)[0] == 0
    assert animated_opacity(Interpolation.EASE_IN)[0] < linear
    assert animated_opacity(Interpolation.EASE_OUT)[0] > linear
    # Halfway through a symmetric ease is linear. Sample at 0.25 instead.
    linear_quarter = animated_opacity(Interpolation.LINEAR, 2)[0]
    ease_in_out_quarter = animated_opacity(Interpolation.EASE_IN_OUT, 2)[0]
    assert ease_in_out_quarter < linear_quarter
    bezier = animated_opacity(CubicBezier(0.42, 0, 1, 1), 2)[0]
    assert bezier < linear_quarter


def test_cpu_frame_has_complete_track_lifecycle_semantics() -> None:
    authored = ProjectBuilder(
        width=4, height=4, frame_rate=FrameRate(4, 1), output_path="out.mp4", duration=2,
        background="#000000",
    )
    clip = authored.add_solid_color_clip(colour="#ffffff", start=0, duration=2, layer=0, opacity=0.2)
    clip.opacity.keyframe(time=0.5, value=0)
    clip.opacity.keyframe(time=1, value=1, interpolation=Interpolation.LINEAR)
    prepared = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    # Initial base value, before first, at first, midway, and final respectively.
    values = [prepared.render_frame_number(frame).to_bytes()[0] for frame in range(5)]
    assert values == [51, 51, 0, 128, 255]

    single = ProjectBuilder(
        width=4, height=4, frame_rate=FrameRate(4, 1), output_path="out.mp4", duration=2,
        background="#000000",
    )
    single_clip = single.add_solid_color_clip(colour="#ffffff", start=0, duration=2, layer=0, opacity=0.2)
    single_clip.opacity.keyframe(time=0.5, value=0.8)
    single_prepared = vestra.Editor().prepare(
        single.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
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
    prepared = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
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
        return vestra.Editor().prepare(
            authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
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


def test_animated_anchor_crop_and_position_have_stable_geometric_regions() -> None:
    """Exercise axes, clip-local interpolation, and crop geometry without edge-only checks."""
    def image_clip() -> tuple[ProjectBuilder, object]:
        authored = ProjectBuilder(width=8, height=6, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1, base_directory=Path.cwd())
        asset = authored.add_image_asset("tests/assets/wgpu-small-rgba.png")
        return authored, authored.add_image_clip(source=asset, start=0, duration=1, layer=0, sizing=Sizing.stretch(width=4, height=3))

    def pixel(authored: ProjectBuilder, frame: int, x: int, y: int) -> tuple[int, int, int, int]:
        rendered = vestra.Editor().prepare(authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU)).render_frame_number(frame).to_bytes()
        offset = (y * 8 + x) * 4
        return tuple(rendered[offset:offset + 4])  # type: ignore[return-value]

    anchored, clip = image_clip()
    clip.transform.anchor.keyframe(time=0, value=Point(0.5, 0.5))
    clip.transform.anchor.keyframe(time=1, value=Point(0, 0))
    assert pixel(anchored, 0, 3, 2) == (254, 254, 254, 255)
    assert pixel(anchored, 5, 4, 3) == (254, 254, 254, 255)
    assert pixel(anchored, 5, 3, 2) == (0, 0, 0, 255)

    cropped, clip = image_clip()
    clip.set_crop(Crop(0, 0, 1, 1))
    clip.crop.keyframe(time=0, value=Crop(0, 0, 1, 1))
    clip.crop.keyframe(time=1, value=Crop(0.5, 0, 0.5, 1))
    assert pixel(cropped, 0, 3, 2) == (254, 254, 254, 255)
    assert pixel(cropped, 5, 3, 2) == (218, 229, 251, 255)
    assert pixel(cropped, 9, 4, 2) == (0, 0, 0, 255)

    moved, clip = image_clip()
    clip.transform.position.keyframe(time=0, value=Point(0.5, 0.5))
    clip.transform.position.keyframe(time=1, value=Point(0.25, 0.25))
    assert pixel(moved, 0, 3, 2) == (254, 254, 254, 255)
    assert pixel(moved, 5, 2, 2) == (253, 184, 2, 255)


def test_animated_gaussian_blur_and_brightness_have_stable_metrics() -> None:
    blurred = ProjectBuilder(width=8, height=6, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1, base_directory=Path.cwd())
    asset = blurred.add_image_asset("tests/assets/wgpu-small-rgba.png")
    clip = blurred.add_image_clip(source=asset, start=0, duration=1, layer=0, sizing=Sizing.stretch(width=4, height=3))
    effect = clip.effects.add_gaussian_blur(radius=0)
    effect.radius.keyframe(time=0, value=0)
    effect.radius.keyframe(time=1, value=3)
    prepared = vestra.Editor().prepare(blurred.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU))
    assert prepared.render_frame_number(0).to_bytes()[(2 * 8 + 3) * 4] == 254
    assert prepared.render_frame_number(5).to_bytes()[(2 * 8 + 3) * 4] == 163
    assert prepared.render_frame_number(9).to_bytes()[(2 * 8 + 3) * 4] == 66

    coloured = ProjectBuilder(width=4, height=4, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1)
    solid = coloured.add_solid_color_clip(colour="#808080", start=0, duration=1, layer=0)
    brightness = solid.effects.add_brightness(amount=0)
    brightness.amount.keyframe(time=0, value=0)
    brightness.amount.keyframe(time=1, value=0.2)
    prepared = vestra.Editor().prepare(coloured.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU))
    assert [prepared.render_frame_number(frame).to_bytes()[0] for frame in (0, 5, 9)] == [128, 154, 174]


def test_animated_tint_uses_the_native_colour_effect_track() -> None:
    authored = ProjectBuilder(width=4, height=4, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1)
    clip = authored.add_solid_color_clip(colour="#808080", start=0, duration=1, layer=0)
    tint = clip.effects.add_tint(colour="#ff0000", amount=0)
    tint.amount.keyframe(time=0, value=0)
    tint.amount.keyframe(time=1, value=1)
    prepared = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    before, middle, final = [prepared.render_frame_number(frame).to_bytes()[:4] for frame in (0, 5, 9)]
    assert tuple(before) == (128, 128, 128, 255)
    assert 188 <= middle[0] <= 193 and 62 <= middle[1] <= 66 and middle[1] == middle[2]
    assert final[0] > 240 and final[1] < 16 and final[1] == final[2]


def test_cpu_video_renders_authored_animation_without_audio(tmp_path: Path) -> None:
    authored = ProjectBuilder(
        width=160, height=90, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1,
        background="#000000", base_directory=Path.cwd(),
    )
    image = authored.add_image_asset("examples/assets/red.png")
    clip = authored.add_image_clip(source=image, start=0, duration=1, layer=0)
    clip.transform.scale.keyframe(time=0, value=clip.transform.scale.base_value)
    clip.transform.scale.keyframe(time=1, value=type(clip.transform.scale.base_value)(1.2, 1.2), interpolation=Interpolation.EASE_IN_OUT)
    clip.opacity.keyframe(time=0, value=0)
    clip.opacity.keyframe(time=0.5, value=1, interpolation=CubicBezier(0.25, 0.1, 0.25, 1))
    assert authored.validate().is_valid
    output = tmp_path / "animated.mp4"
    result = vestra.Editor().render(authored.build(), vestra.RenderRequest(
        output, backend=vestra.BackendPreference.CPU, overwrite=True,
    ))
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "stream=codec_type,nb_frames,duration", "-of", "csv=p=0", str(output)],
        check=True, capture_output=True, text=True,
    )
    assert result.total_frames == 10 and output.is_file() and output.stat().st_size > 0
    assert probe.stdout.splitlines()[0] == "video,1.000000,10"
    assert not list(tmp_path.glob("*.tmp"))
