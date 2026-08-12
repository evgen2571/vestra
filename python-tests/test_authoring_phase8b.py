from pathlib import Path
import subprocess
from typing import Any, cast

import pytest

import vestra
from vestra import FrameRate
from vestra.authoring import AuthoringError, Crop, Point, ProjectBuilder, Sizing


def builder(**changes: object) -> ProjectBuilder:
    arguments: dict[str, object] = {
        "width": 160, "height": 90, "frame_rate": FrameRate(10, 1),
        "output_path": "out.mp4", "duration": None, "base_directory": ".",
    }
    arguments.update(changes)
    return ProjectBuilder(**arguments)  # type: ignore[arg-type]


def test_assets_clips_and_audio_serialize_in_registration_order() -> None:
    authored = builder(output_audio=True)
    image = authored.add_image_asset("examples/assets/red.png")
    audio = authored.add_audio_asset(Path("examples/assets/tone.wav"))
    second_image = authored.add_image_asset("examples/assets/blue.png")
    clip = authored.add_image_clip(source=image, start=0, duration=1, layer=0, sizing=Sizing.fit())
    solid = authored.add_solid_color_clip(colour="#112233", start=1, duration=1, layer=-1)
    clip.transform.position.base_value = Point(0.25, 0.75)
    clip.set_crop(Crop(0, 0, 1, 1))
    track = authored.audio.add_track(id="music")
    track.add_clip(asset=audio, start=0, fade_in=0.1)

    data = authored.to_dict()
    assets = cast(list[dict[str, Any]], data["assets"])
    clips = cast(list[dict[str, Any]], cast(dict[str, Any], data["visual"])["clips"])
    assert [asset["id"] for asset in assets] == [image.id, audio.id, second_image.id]
    assert [item["id"] for item in clips] == [clip.id, solid.id]
    first = clips[0]
    assert first["source"] == {"type": "image", "asset": image.id}
    assert first["transform"]["position"] == {"base_value": {"x": 0.25, "y": 0.75}}
    assert first["crop"] == {"base_value": {"x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0}}
    assert clips[1]["source"] == {"type": "solid_color", "colour": "#112233"}
    assert cast(dict[str, Any], data["output"])["audio"] is True
    assert cast(dict[str, Any], data["audio"])["tracks"][0]["clips"][0]["asset"] == audio.id
    assert authored.validate().is_valid


def test_asset_and_clip_ownership_and_category_rules() -> None:
    left = builder()
    right = builder()
    image = left.add_image_asset("image.png", id="same")
    audio = left.add_audio_asset("sound.wav")
    with pytest.raises(AuthoringError):
        left.add_audio_asset("other.wav", id="same")
    with pytest.raises(TypeError):
        left.add_image_clip(source=audio, start=0, duration=1, layer=0)  # type: ignore[arg-type]
    track = left.audio.add_track()
    with pytest.raises(TypeError):
        track.add_clip(asset=image, start=0)  # type: ignore[arg-type]
    with pytest.raises(AuthoringError):
        right.add_image_clip(source=image, start=0, duration=1, layer=0)
    with pytest.raises(AuthoringError):
        right.audio.add_track().add_clip(asset=audio, start=0)
    assert image != right.add_image_asset("image.png", id="same")


def test_mutation_is_validated_and_snapshots_are_isolated() -> None:
    authored = builder()
    image = authored.add_image_asset("image.png")
    clip = authored.add_image_clip(source=image, start=0, duration=1, layer=0)
    before = authored.to_dict()
    clip.opacity.base_value = 0.5
    clip.clear_crop()
    before_clips = cast(list[dict[str, Any]], cast(dict[str, Any], before["visual"])["clips"])
    assert before_clips[0]["opacity"] == {"base_value": 1.0}
    with pytest.raises(ValueError):
        clip.transform.scale.base_value = Point(0, 1)
    with pytest.raises(TypeError):
        clip.layer = True
    with pytest.raises(AttributeError):
        clip.id = "nope"  # type: ignore[misc]


def test_cpu_frame_and_video_cover_solid_image_and_audio(tmp_path: Path) -> None:
    authored = builder(base_directory=Path.cwd(), duration=None, output_audio=True)
    image = authored.add_image_asset("examples/assets/red.png")
    audio = authored.add_audio_asset("examples/assets/tone.wav")
    authored.add_solid_color_clip(colour="#0000ff", start=0, duration=0.2, layer=0)
    authored.add_image_clip(source=image, start=0, duration=0.2, layer=1, sizing=Sizing.stretch(width=160, height=90))
    authored.audio.add_track().add_clip(asset=audio, start=0, trim_end=0.2)
    project = authored.build()
    prepared = vestra.Editor().prepare(
        project, vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    frame = prepared.render_frame_number(0)
    assert frame.width == 160 and frame.height == 90 and len(frame.to_bytes()) == 160 * 90 * 4
    output = tmp_path / "authored.mp4"
    result = vestra.Editor().render(project, vestra.RenderRequest(output, backend=vestra.BackendPreference.CPU, overwrite=True))
    assert output.is_file() and output.stat().st_size > 0 and result.audio_present
    probe = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries", "stream=codec_type", "-of", "default=noprint_wrappers=1", str(output)], check=True, capture_output=True, text=True)
    assert "codec_type=audio" in probe.stdout
