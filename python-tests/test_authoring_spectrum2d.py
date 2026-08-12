import json
from pathlib import Path

import pytest
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


def _native_spectrum_defaults() -> dict[str, object]:
    native = video_editor.Project.from_dict({
        "schema_version": 2,
        "output": {"path": "out.mp4", "width": 64, "height": 64, "frame_rate": "10/1",
                    "background": "#000000", "quality": "balanced", "audio": False,
                    "duration_mode": "explicit", "duration": 0.2},
        "assets": [],
        "visual": {"clips": [{
            "id": "spectrum", "source": {"type": "spectrum2d"}, "start": 0,
            "duration": 0.2, "layer": 1, "opacity": {"base_value": 1},
        }]},
    })
    return native.to_dict()["visual"]["clips"][0]["source"]  # type: ignore[index,return-value]


def _spectrum_schema() -> dict[str, object]:
    for branch in SCHEMA["$defs"]["source"]["oneOf"]:  # type: ignore[index]
        if branch.get("$ref") == "#/$defs/spectrum2d":  # type: ignore[union-attr]
            return SCHEMA["$defs"]["spectrum2d"]  # type: ignore[return-value,index]
    raise AssertionError("Spectrum2D schema branch is not registered")


def test_spectrum2d_defaults_conform_across_native_python_and_schema() -> None:
    project = builder()
    clip = project.add_spectrum2d_clip(start=0, duration=0.2, layer=1, id="spectrum")
    assert isinstance(clip, Spectrum2DClip)
    python_source = clip.to_canonical()["source"]
    rust_source = _native_spectrum_defaults()
    assert python_source == rust_source
    schema_properties = _spectrum_schema()["properties"]  # type: ignore[index]
    schema_defaults = {
        field: property_schema["default"]
        for field, property_schema in schema_properties.items()  # type: ignore[union-attr]
        if "default" in property_schema
    }
    assert schema_defaults == {field: rust_source[field] for field in schema_defaults}
    assert not list(VALIDATOR.iter_errors(project.to_dict()))


@pytest.mark.parametrize("field", ["min_hz", "max_hz"])
@pytest.mark.parametrize("value", [0, -1, float("nan"), float("inf"), -float("inf")])
def test_spectrum2d_frequency_bounds_reject_non_positive_and_non_finite_values(
    field: str, value: float,
) -> None:
    with pytest.raises((TypeError, ValueError)):
        builder().add_spectrum2d_clip(start=0, duration=0.2, layer=1, **{field: value})


@pytest.mark.parametrize("field", ["min_hz", "max_hz"])
def test_spectrum2d_frequency_bounds_accept_positive_values(field: str) -> None:
    clip = builder().add_spectrum2d_clip(start=0, duration=0.2, layer=1, **{field: 1.0})
    assert getattr(clip, field) == 1.0


def test_spectrum2d_max_hz_accepts_nyquist_and_rejects_above_it() -> None:
    valid_project = builder()
    valid_project.add_spectrum2d_clip(start=0, duration=0.2, layer=1, max_hz=24_000)
    valid = valid_project.to_dict()
    assert not list(VALIDATOR.iter_errors(valid))
    invalid = json.loads(json.dumps(valid))
    invalid["visual"]["clips"][0]["source"]["max_hz"] = 24_000.001
    assert list(VALIDATOR.iter_errors(invalid))
    with pytest.raises((TypeError, ValueError)):
        builder().add_spectrum2d_clip(start=0, duration=0.2, layer=1, max_hz=24_000.001)


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


_PRESET_SOURCES: dict[str, dict[str, object]] = {
    "classic": {
        "band_count": 24, "min_hz": 40.0, "max_hz": 16_000.0, "sensitivity": 8.0,
        "attack_seconds": 0.020, "release_seconds": 0.150, "x": 0.10, "y": 0.70,
        "width": 0.80, "height": 0.25, "bar_gap_ratio": 0.20, "colour": "#ffffff",
    },
    "dense": {
        "band_count": 48, "min_hz": 40.0, "max_hz": 18_000.0, "sensitivity": 9.0,
        "attack_seconds": 0.012, "release_seconds": 0.110, "x": 0.08, "y": 0.68,
        "width": 0.84, "height": 0.27, "bar_gap_ratio": 0.10, "colour": "#ffffff",
    },
    "neon": {
        "band_count": 32, "min_hz": 40.0, "max_hz": 16_000.0, "sensitivity": 10.0,
        "attack_seconds": 0.015, "release_seconds": 0.180, "x": 0.10, "y": 0.68,
        "width": 0.80, "height": 0.27, "bar_gap_ratio": 0.14, "colour": "#ffffff",
    },
}


def _preset_clip(preset: str, **overrides: object) -> dict[str, object]:
    project = builder()
    project.add_spectrum2d_clip(
        start=0, duration=0.2, layer=1, id="spectrum", preset=preset, **overrides,  # type: ignore[arg-type]
    )
    assert not list(VALIDATOR.iter_errors(project.to_dict()))
    return project.to_dict()["visual"]["clips"][0]  # type: ignore[return-value,index]


def _explicit_clip(preset: str) -> dict[str, object]:
    project = builder()
    clip = project.add_spectrum2d_clip(
        start=0, duration=0.2, layer=1, id="spectrum", **_PRESET_SOURCES[preset],  # type: ignore[arg-type]
    )
    if preset == "neon":
        clip.effects.add_glow(threshold=0.35, radius=3.0, intensity=0.85, colour="#ffffff")
        clip.effects.add_bloom(threshold=0.55, radius=4.0, intensity=0.65)
    return project.to_dict()["visual"]["clips"][0]  # type: ignore[return-value,index]


@pytest.mark.parametrize("preset", ["classic", "dense", "neon"])
def test_spectrum2d_preset_expands_to_explicit_equivalent(preset: str) -> None:
    assert _preset_clip(preset) == _explicit_clip(preset)


def test_spectrum2d_preset_overrides_are_applied_after_preset_values() -> None:
    clip = _preset_clip("dense", band_count=24, height=0.20, colour="#ff00ff")
    source = clip["source"]  # type: ignore[index]
    assert source["band_count"] == 24  # type: ignore[index]
    assert source["height"] == 0.20  # type: ignore[index]
    assert source["colour"] == "#ff00ff"  # type: ignore[index]
    assert source["max_hz"] == 18_000.0  # type: ignore[index]
    assert source["bar_gap_ratio"] == 0.10  # type: ignore[index]


def test_spectrum2d_invalid_preset_fails_before_serialization() -> None:
    project = builder()
    with pytest.raises(ValueError, match="unknown Spectrum2D preset"):
        project.add_spectrum2d_clip(start=0, duration=0.2, layer=1, preset="unknown")  # type: ignore[arg-type]
    assert "preset" not in json.dumps(project.to_dict())


@pytest.mark.parametrize("preset", ["classic", "dense", "neon"])
def test_spectrum2d_preset_canonical_json_has_no_preset_identity(preset: str) -> None:
    data = _preset_clip(preset)
    assert "preset" not in json.dumps(data)
    assert "style" not in json.dumps(data)


def test_spectrum2d_preset_native_round_trip_preserves_expansion() -> None:
    authored = builder()
    authored.add_spectrum2d_clip(start=0, duration=0.2, layer=1, id="spectrum", preset="neon")
    canonical = authored.to_dict()
    reloaded = video_editor.Project.from_dict(canonical).to_dict()
    original_clip = canonical["visual"]["clips"][0]  # type: ignore[index]
    reloaded_clip = reloaded["visual"]["clips"][0]  # type: ignore[index]
    assert reloaded_clip["source"] == original_clip["source"]  # type: ignore[index]

    def without_empty_keyframes(value: object) -> object:
        if isinstance(value, dict):
            return {
                key: without_empty_keyframes(item)
                for key, item in value.items()
                if not (key == "keyframes" and item == [])
            }
        if isinstance(value, list):
            return [without_empty_keyframes(item) for item in value]
        return value

    assert without_empty_keyframes(reloaded_clip["effects"]) == without_empty_keyframes(original_clip["effects"])  # type: ignore[index]


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
