import json
from pathlib import Path

from jsonschema import Draft202012Validator

import video_editor
from video_editor import FrameRate
from video_editor.authoring import BlendMode, ProjectBuilder, Spectrum2DClip


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = json.loads((ROOT / "schemas/project.schema.json").read_text())
VALIDATOR = Draft202012Validator(SCHEMA)


def builder(**changes: object) -> ProjectBuilder:
    arguments: dict[str, object] = {
        "width": 64, "height": 64, "frame_rate": FrameRate(10, 1),
        "output_path": "spectrum.mp4", "duration": 0.2,
    }
    arguments.update(changes)
    return ProjectBuilder(**arguments)  # type: ignore[arg-type]


def test_spectrum2d_defaults_are_typed_and_canonical() -> None:
    project = builder()
    clip = project.add_spectrum2d_clip(start=0, duration=0.2, layer=1, id="spectrum")
    assert isinstance(clip, Spectrum2DClip)
    source = clip.to_canonical()["source"]
    assert source == {
        "type": "spectrum2d", "band_count": 24, "min_hz": 40.0, "max_hz": 16000.0,
        "sensitivity": 8.0, "attack_seconds": 0.02, "release_seconds": 0.15,
        "x": 0.1, "y": 0.7, "width": 0.8, "height": 0.25,
        "bar_gap_ratio": 0.2, "colour": "#ffffff",
    }
    assert not list(VALIDATOR.iter_errors(project.to_dict()))
    native_defaults = video_editor.Project.from_dict({
        "schema_version": 2,
        "output": {"path": "out.mp4", "width": 64, "height": 64, "frame_rate": "10/1",
                    "background": "#000000", "quality": "balanced", "audio": False,
                    "duration_mode": "explicit", "duration": 0.2},
        "assets": [],
        "visual": {"clips": [clip.to_canonical()], "transitions": [], "flashes": [], "post_effects": []},
    })
    assert native_defaults.to_dict()["visual"]["clips"][0]["source"] == source  # type: ignore[index]


def test_spectrum2d_explicit_configuration_and_normal_effects() -> None:
    project = builder()
    clip = project.add_spectrum2d_clip(
        start=0, duration=0.2, layer=2, band_count=32, min_hz=60, max_hz=12000,
        sensitivity=10, attack_seconds=0.04, release_seconds=0.3,
        x=0.05, y=0.65, width=0.9, height=0.3, bar_gap_ratio=0.4, colour="#12ab34cc",
    )
    clip.opacity.base_value = 0.75
    clip.blend_mode = BlendMode.ADD
    brightness = clip.effects.add_brightness(amount=0.2)
    bloom = clip.effects.add_bloom(threshold=0.5, intensity=0.8, radius=4.0)
    data = project.to_dict()
    source = data["visual"]["clips"][0]["source"]  # type: ignore[index]
    assert source["band_count"] == 32  # type: ignore[index]
    assert source["colour"] == "#12ab34cc"  # type: ignore[index]
    assert [effect["type"] for effect in data["visual"]["clips"][0]["effects"]] == [  # type: ignore[index]
        "brightness", "bloom",
    ]
    assert brightness and bloom
    assert not list(VALIDATOR.iter_errors(data))


def test_spectrum2d_no_audio_uses_native_diagnostic() -> None:
    project = builder()
    project.add_spectrum2d_clip(start=0, duration=0.2, layer=1)
    report = project.validate()
    assert "MVP-SPECTRUM2D-MASTER-AUDIO" in {item.code for item in report.diagnostics}


def test_spectrum2d_with_authored_audio_validates() -> None:
    project = builder(base_directory=ROOT)
    audio = project.add_audio_asset("examples/assets/tone.wav")
    track = project.audio.add_track(id="music")
    track.add_clip(asset=audio, start=0, trim_end=0.2)
    project.add_spectrum2d_clip(start=0, duration=0.2, layer=1)
    authored = project.to_dict()
    assert not list(VALIDATOR.iter_errors(authored))
    reloaded = video_editor.Project.from_dict(authored, base_directory=ROOT).to_dict()
    assert reloaded["visual"]["clips"][0]["source"] == authored["visual"]["clips"][0]["source"]  # type: ignore[index]
    assert project.validate().is_valid
