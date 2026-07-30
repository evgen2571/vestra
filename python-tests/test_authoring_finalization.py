"""Phase 8B ownership, transaction, and lifecycle contracts."""

from copy import deepcopy
from pathlib import Path
import subprocess

import pytest

import video_editor
from video_editor import FrameRate
from video_editor.authoring import (
    AudioAsset, AudioTrack, AuthoringError, Crop, CropTrack, ImageAsset, ImageClip, Point,
    PointTrack, ProjectBuilder, ScalarTrack, Sizing, SolidColorClip, Transform,
)


def builder() -> ProjectBuilder:
    return ProjectBuilder(
        width=160, height=90, frame_rate=FrameRate(10, 1), output_path="out.mp4",
    )


def test_owned_nodes_are_factory_only_and_have_deterministic_representations() -> None:
    for factory in (ImageAsset, AudioAsset, ImageClip, SolidColorClip, AudioTrack, Transform,
                    ScalarTrack, PointTrack, CropTrack):
        with pytest.raises(TypeError, match="ProjectBuilder"):
            factory()

    authored = builder()
    image = authored.add_image_asset("cover.png")
    audio = authored.add_audio_asset("music.wav")
    clip = authored.add_image_clip(source=image, start=0, duration=1, layer=0)
    solid = authored.add_solid_color_clip(colour="#112233", start=0, duration=1, layer=1)
    track = authored.set_audio(asset=audio, timeline_start=0, trim_start=0)
    for value in (image, audio, clip, solid, track, clip.transform, clip.opacity):
        representation = repr(value)
        assert "_Owner" not in representation
        assert "object at 0x" not in representation
        assert "_owner=" not in representation


def test_track_and_transform_identities_are_read_only_but_values_remain_mutable() -> None:
    authored = builder()
    image = authored.add_image_asset("cover.png")
    clip = authored.add_image_clip(source=image, start=0, duration=1, layer=0)
    snapshot = deepcopy(authored.to_dict())
    with pytest.raises(AttributeError):
        clip.opacity = "invalid"  # type: ignore[misc, assignment]
    with pytest.raises(AttributeError):
        clip.transform = object()  # type: ignore[misc, assignment]
    with pytest.raises(AttributeError):
        clip.transform.position = object()  # type: ignore[misc, assignment]
    with pytest.raises(AttributeError):
        clip.transform.anchor = Point(0, 0)  # type: ignore[misc, assignment]
    with pytest.raises(AttributeError):
        clip.transform.scale = Point(1, 1)  # type: ignore[misc, assignment]
    with pytest.raises(AttributeError):
        clip.transform.rotation_degrees = 0  # type: ignore[misc, assignment]
    clip.opacity.base_value = 0.5
    clip.transform.scale.base_value = Point(1.2, 1.2)
    with pytest.raises(ValueError, match="opacity"):
        clip.opacity.base_value = 2
    with pytest.raises(ValueError, match="anchor"):
        clip.transform.anchor.base_value = Point(-1, 0)
    with pytest.raises(ValueError, match="scale"):
        clip.transform.scale.base_value = Point(0, 1)
    assert snapshot["visual"] != authored.to_dict()["visual"]


def test_failed_asset_and_clip_operations_leave_ids_and_data_unchanged() -> None:
    authored = builder()
    before = authored.to_dict()
    with pytest.raises(ValueError):
        authored.add_image_asset("", id="cover")
    assert authored.to_dict() == before
    assert authored.add_image_asset("cover.png", id="cover").id == "cover"
    with pytest.raises(ValueError):
        authored.add_audio_asset("")
    assert authored.add_audio_asset("sound.wav").id == "audio-000001"

    image = authored.add_image_asset("image.png")
    before = authored.to_dict()
    with pytest.raises(ValueError):
        authored.add_image_clip(source=image, start=0, duration=0, layer=0, id="intro")
    assert authored.to_dict() == before
    assert authored.add_image_clip(source=image, start=0, duration=1, layer=0, id="intro").id == "intro"
    with pytest.raises(ValueError):
        authored.add_solid_color_clip(colour="not-a-colour", start=0, duration=1, layer=0)
    assert authored.add_solid_color_clip(colour="#000000", start=0, duration=1, layer=0).id == "clip-000001"


def test_all_failure_paths_preserve_builder_state_and_shared_namespaces() -> None:
    authored = builder()
    before = authored.to_dict()
    with pytest.raises(ValueError):
        authored.add_audio_asset("", id="sound")
    assert authored.to_dict() == before
    assert authored.add_audio_asset("sound.wav", id="sound").id == "sound"

    image = authored.add_image_asset("image.png")
    wrong_kind = authored.add_audio_asset("other.wav")
    other = builder().add_image_asset("other.png")
    before = authored.to_dict()
    with pytest.raises(TypeError):
        authored.add_image_clip(source=wrong_kind, start=0, duration=1, layer=0)  # type: ignore[arg-type]
    with pytest.raises(AuthoringError):
        authored.add_image_clip(source=other, start=0, duration=1, layer=0)
    with pytest.raises(ValueError):
        authored.add_solid_color_clip(colour="#000000", start=0, duration=0, layer=0, id="solid")
    assert authored.to_dict() == before
    assert authored.add_solid_color_clip(colour="#000000", start=0, duration=1, layer=0, id="solid").id == "solid"
    with pytest.raises(TypeError):
        authored.set_audio(asset=image, timeline_start=0, trim_start=0)  # type: ignore[arg-type]


def test_explicit_duration_retains_native_truncation_warning() -> None:
    authored = ProjectBuilder(
        width=160, height=90, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1,
    )
    authored.add_solid_color_clip(colour="#000000", start=0, duration=2, layer=0)
    report = video_editor.Editor().inspect(authored.build())
    assert report.output.duration == 1.0
    assert any(warning.code == "MVP-DURATION-TRUNCATED" for warning in report.warnings)


def test_crop_and_audio_nodes_are_stable_and_failed_updates_are_transactional() -> None:
    authored = builder()
    image = authored.add_image_asset("image.png")
    sound = authored.add_audio_asset("sound.wav")
    clip = authored.add_image_clip(source=image, start=0, duration=1, layer=0)
    crop = clip.crop
    assert not clip.has_crop
    clip.set_crop(Crop(0, 0, 1, 1))
    assert crop is clip.crop and clip.has_crop
    before_crop = clip.to_canonical()
    with pytest.raises(TypeError):
        clip.set_crop("bad")
    assert clip.to_canonical() == before_crop
    clip.clear_crop()
    assert crop is clip.crop and not clip.has_crop and "crop" not in clip.to_canonical()
    crop.base_value = Crop(0.25, 0, 0.75, 1)
    assert "crop" not in clip.to_canonical()

    track = authored.set_audio(asset=sound, timeline_start=0, trim_start=0)
    assert track is authored.audio and authored.has_audio
    before_audio = authored.to_dict()
    with pytest.raises(ValueError):
        authored.set_audio(asset=sound, timeline_start=0, trim_start=1, trim_end=1)
    assert authored.to_dict() == before_audio
    assert authored.set_audio(asset=sound, timeline_start=1, trim_start=0) is track
    authored.clear_audio()
    assert authored.audio is track and not authored.has_audio and "audio" not in authored.to_dict()


@pytest.mark.parametrize(
    ("opacity", "visible", "expected"),
    [(1.0, True, bytes((17, 34, 51, 255))), (0.5, True, bytes((9, 17, 26, 255))),
     (1.0, False, bytes((0, 0, 0, 255)))],
)
def test_solid_colour_cpu_pixels_cover_opacity_and_visibility(
    opacity: float, visible: bool, expected: bytes,
) -> None:
    authored = builder()
    authored.background = "#000000"
    authored.add_solid_color_clip(
        colour="#112233", start=0, duration=1, layer=0, opacity=opacity, visible=visible,
    )
    frame = video_editor.Editor().prepare(
        authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    ).render_frame_number(0)
    assert frame.to_bytes()[:4] == expected


def test_native_inspection_resolves_visual_and_audio_automatic_duration() -> None:
    visual = builder()
    visual.add_solid_color_clip(colour="#000000", start=1, duration=2.5, layer=0, visible=False)
    visual_report = video_editor.Editor().inspect(visual.build())
    assert visual_report.output.duration == 3.5
    assert visual_report.visual_clips == 1
    assert visual_report.assets.images == 0 and visual_report.assets.audio == 0

    audio = builder()
    asset = audio.add_audio_asset("examples/assets/tone.wav")
    audio.add_solid_color_clip(colour="#000000", start=0, duration=1, layer=0)
    audio.set_audio(asset=asset, timeline_start=1, trim_start=0, trim_end=2)
    audio_report = video_editor.Editor().inspect(audio.build())
    assert audio_report.output.duration == 3.0
    assert audio_report.assets.audio == 1
    assert audio_report.audio is not None
    assert audio_report.audio.asset == asset.id and audio_report.audio.start == 1.0
    assert audio_report.audio.end == 3.0

    assert audio.audio is not None
    audio.audio.mute = True
    muted_report = video_editor.Editor().inspect(audio.build())
    assert muted_report.output.duration == 1.0
    assert muted_report.audio is None


def test_muted_audio_serializes_but_cpu_video_has_no_audio_stream(tmp_path: Path) -> None:
    authored = builder()
    authored.base_directory = Path.cwd()
    asset = authored.add_audio_asset("examples/assets/tone.wav")
    authored.add_solid_color_clip(colour="#000000", start=0, duration=0.2, layer=0)
    track = authored.set_audio(asset=asset, timeline_start=0, trim_start=0, trim_end=0.2, mute=True)
    assert authored.to_dict()["audio"] == track.to_canonical()
    output = tmp_path / "muted.mp4"
    result = video_editor.Editor().render(
        authored.build(), video_editor.RenderRequest(
            output, backend=video_editor.BackendPreference.CPU, overwrite=True,
        ),
    )
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "stream=codec_type", "-of", "csv=p=0", str(output)],
        check=True, capture_output=True, text=True,
    )
    assert result.audio_present is False
    assert probe.stdout.split() == ["video"]


def test_image_pixels_visibility_and_native_id_tie_breaking() -> None:
    image_project = builder()
    image = image_project.add_image_asset("examples/assets/red.png")
    image_project.add_image_clip(
        source=image, start=0, duration=1, layer=0, sizing=Sizing.stretch(width=160, height=90),
    )
    image_frame = video_editor.Editor().prepare(
        image_project.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    ).render_frame_number(0)
    assert image_frame.to_bytes()[:4] == bytes((250, 1, 1, 255))
    image_project.clips[0].visible = False
    hidden_frame = video_editor.Editor().prepare(
        image_project.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    ).render_frame_number(0)
    assert hidden_frame.to_bytes()[:4] == bytes((0, 0, 0, 255))

    ordered = builder()
    first = ordered.add_solid_color_clip(colour="#ff0000", start=0, duration=1, layer=0, id="z-last")
    second = ordered.add_solid_color_clip(colour="#0000ff", start=0, duration=1, layer=0, id="a-first")
    assert [clip.id for clip in ordered.clips] == [first.id, second.id]
    tied_frame = video_editor.Editor().prepare(
        ordered.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    ).render_frame_number(0)
    assert tied_frame.to_bytes()[:4] == bytes((255, 0, 0, 255))


def test_image_sizing_crop_and_transform_change_cpu_interior_pixels() -> None:
    def render(
        sizing: Sizing | None = None, *, crop: Crop | None = None, position: Point | None = None,
        scale: Point | None = None, rotation: float | None = None, clear_crop: bool = False,
    ) -> bytes:
        authored = builder()
        authored.width = 8
        authored.height = 6
        image = authored.add_image_asset("tests/assets/wgpu-small-rgba.png")
        clip = authored.add_image_clip(
            source=image, start=0, duration=1, layer=0, sizing=sizing, crop=crop,
        )
        if position is not None:
            clip.transform.position.base_value = position
        if scale is not None:
            clip.transform.scale.base_value = scale
        if rotation is not None:
            clip.transform.rotation_degrees.base_value = rotation
        if clear_crop:
            clip.clear_crop()
        return video_editor.Editor().prepare(
            authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
        ).render_frame_number(0).to_bytes()

    def pixel(frame: bytes, x: int, y: int) -> bytes:
        offset = (y * 8 + x) * 4
        return frame[offset:offset + 4]

    original = render()
    fit = render(Sizing.fit())
    cover = render(Sizing.cover())
    scaled = render(Sizing.scale(0.04))
    stretched = render(Sizing.stretch(width=8, height=6))
    assert pixel(original, 0, 0) == bytes((0, 74, 236, 255))
    assert pixel(fit, 0, 0) == bytes((0, 0, 0, 255))
    assert pixel(cover, 2, 1) == bytes((234, 30, 32, 255))
    assert pixel(scaled, 2, 1) == bytes((7, 2, 1, 255))
    assert pixel(stretched, 4, 3) == bytes((0, 72, 236, 255))

    uncropped = render(Sizing.stretch(width=8, height=6))
    cropped = render(Sizing.stretch(width=8, height=6), crop=Crop(0.5, 0, 0.5, 1))
    assert pixel(uncropped, 2, 1) != pixel(cropped, 2, 1)
    assert render(Sizing.stretch(width=8, height=6), crop=Crop(0.5, 0, 0.5, 1), clear_crop=True) == uncropped

    moved = render(Sizing.stretch(width=4, height=3), position=Point(0.25, 0.25))
    scaled_transform = render(Sizing.stretch(width=4, height=3), scale=Point(0.5, 0.5))
    rotated = render(Sizing.stretch(width=4, height=3), rotation=45)
    assert pixel(moved, 2, 1) == bytes((0, 73, 236, 255))
    assert pixel(scaled_transform, 2, 1) != pixel(moved, 2, 1)
    assert pixel(rotated, 2, 1) == bytes((0, 0, 0, 255))
