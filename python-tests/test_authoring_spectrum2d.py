import json
import math
import os
import struct
import wave
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

import vestra
from vestra import FrameRate
from vestra.authoring import (
    BlendMode,
    ProjectBuilder,
    Spectrum2DClip,
    Spectrum2DGradient,
    Spectrum2DLinearLayout,
    Spectrum2DRadialLayout,
)


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
    native = vestra.ProjectSnapshot.from_dict({
        "schema_version": 1,
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
    assert schema_defaults.pop("style") == clip.style == "bars"
    assert schema_defaults.pop("response_power") == clip.response_power == 1.0
    assert "style" not in rust_source and "response_power" not in rust_source
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


@pytest.mark.parametrize("field, value", [
    ("layout", "radial"), ("layout", {}), ("layout", 123),
    ("gradient", "red-blue"), ("gradient", {}), ("gradient", 123),
])
def test_spectrum2d_rejects_invalid_runtime_layout_and_gradient_types(
    field: str, value: object,
) -> None:
    with pytest.raises(TypeError):
        builder().add_spectrum2d_clip(
            start=0, duration=0.2, layer=1, **{field: value},
        )


def test_spectrum2d_typed_layouts_and_gradients_validate_locally() -> None:
    assert Spectrum2DLinearLayout("center", "center_out").to_canonical() == {
        "type": "linear", "anchor": "center", "band_mapping": "center_out",
    }
    assert Spectrum2DRadialLayout(0.4, -450.0, 180.0, "both", "reverse").to_canonical()[
        "start_angle_degrees"
    ] == -450.0
    assert Spectrum2DGradient("#ff000000", "#0000ffff", "along_bar").to_canonical() == {
        "start_colour": "#ff000000", "end_colour": "#0000ffff", "direction": "along_bar",
    }
    with pytest.raises(ValueError):
        Spectrum2DRadialLayout(1.0)
    with pytest.raises(ValueError):
        Spectrum2DLinearLayout("invalid", "forward")  # type: ignore[arg-type]


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


_NEW_PRESET_SOURCES: dict[str, dict[str, object]] = {
    "mirror": {"band_count": 32, "sensitivity": 9.0, "x": 0.08, "y": 0.58, "width": 0.84, "height": 0.32, "bar_gap_ratio": 0.14, "layout": Spectrum2DLinearLayout("center")},
    "center_out": {"x": 0.08, "width": 0.84, "bar_gap_ratio": 0.12, "layout": Spectrum2DLinearLayout("bottom", "center_out")},
    "circle": {"band_count": 32, "sensitivity": 9.0, "attack_seconds": 0.015, "x": 0.22, "y": 0.22, "width": 0.56, "height": 0.56, "bar_gap_ratio": 0.18, "min_bar_height_ratio": 0.02, "layout": Spectrum2DRadialLayout(0.58)},
    "neon_circle": {"band_count": 48, "max_hz": 18_000.0, "sensitivity": 10.0, "attack_seconds": 0.012, "release_seconds": 0.160, "x": 0.20, "y": 0.20, "width": 0.60, "height": 0.60, "bar_gap_ratio": 0.12, "min_bar_height_ratio": 0.035, "layout": Spectrum2DRadialLayout(0.55), "gradient": Spectrum2DGradient("#00ffff", "#ff00ff", "across_bands")},
    "arc": {"band_count": 32, "x": 0.15, "y": 0.22, "width": 0.70, "height": 0.70, "bar_gap_ratio": 0.14, "min_bar_height_ratio": 0.02, "layout": Spectrum2DRadialLayout(0.55, 270.0, 180.0)},
}


@pytest.mark.parametrize("preset", list(_NEW_PRESET_SOURCES))
def test_new_spectrum2d_presets_expand_to_explicit_equivalents(preset: str) -> None:
    preset_clip = _preset_clip(preset)
    explicit_project = builder()
    clip = explicit_project.add_spectrum2d_clip(
        start=0, duration=0.2, layer=1, id="spectrum", **_NEW_PRESET_SOURCES[preset],  # type: ignore[arg-type]
    )
    if preset == "neon_circle":
        clip.effects.add_glow(threshold=0.35, radius=3.0, intensity=0.85, colour="#ffffff")
        clip.effects.add_bloom(threshold=0.55, radius=4.0, intensity=0.65)
    assert preset_clip == explicit_project.to_dict()["visual"]["clips"][0]
    assert "preset" not in json.dumps(preset_clip)


def test_new_preset_layout_gradient_and_minimum_height_are_whole_unit_overrides() -> None:
    clip = _preset_clip(
        "neon_circle",
        layout=Spectrum2DRadialLayout(0.40, 45.0, 180.0, "both", "reverse"),
        gradient=Spectrum2DGradient("#102030", "#d0c0b0", "along_bar"),
        min_bar_height_ratio=0.10,
    )
    source = clip["source"]  # type: ignore[index]
    assert source["layout"] == {  # type: ignore[index]
        "type": "radial", "inner_radius_ratio": 0.40,
        "start_angle_degrees": 45.0, "sweep_angle_degrees": 180.0,
        "direction": "both", "band_mapping": "reverse",
    }
    assert source["gradient"] == {  # type: ignore[index]
        "start_colour": "#102030", "end_colour": "#d0c0b0", "direction": "along_bar",
    }
    assert source["min_bar_height_ratio"] == 0.10  # type: ignore[index]
    assert source["band_count"] == 48  # type: ignore[index]
    assert source["x"] == 0.20  # type: ignore[index]
    assert [effect["type"] for effect in clip["effects"]] == ["glow", "bloom"]  # type: ignore[index]


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
    reloaded = vestra.ProjectSnapshot.from_dict(canonical).to_dict()
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
    assert "VESTRA-SPECTRUM2D-MASTER-AUDIO" in {item.code for item in report.diagnostics}


def test_spectrum2d_with_authored_audio_validates() -> None:
    project = builder(base_directory=ROOT)
    audio = project.add_audio_asset("assets/fixtures/tone.wav")
    track = project.audio.add_track(id="music")
    track.add_clip(asset=audio, start=0, trim_end=0.2)
    project.add_spectrum2d_clip(start=0, duration=0.2, layer=1)
    authored = project.to_dict()
    assert not list(VALIDATOR.iter_errors(authored))
    reloaded = vestra.ProjectSnapshot.from_dict(authored, base_directory=ROOT).to_dict()
    assert reloaded["visual"]["clips"][0]["source"] == authored["visual"]["clips"][0]["source"]  # type: ignore[index]
    assert project.validate().is_valid


def test_spectrum2d_path_and_stereo_style_lower_identically() -> None:
    path = vestra.Spectrum2DPathLayout(
        [(0, 1), (0.5, 0), (1, 1)], closed=True, direction="right", band_mapping="reverse"
    )
    wave = vestra.Spectrum2DWave(
        stroke_width=3, color="#ADD8FF", fill_opacity=0.24, glow_radius=8, bar_opacity=0.25,
        stereo=vestra.Spectrum2DStereo(left_color="#FFAAAA", right_weight=0.9),
    )
    source = vestra.Spectrum2D(
        layout=path, style="both", wave=wave, soft_knee=0.6, response_power=0.25,
    )
    expected = source.to_canonical()
    assert expected["layout"] == {
        "type": "path", "points": [{"x": 0, "y": 1}, {"x": 0.5, "y": 0}, {"x": 1, "y": 1}],
        "closed": True, "direction": "right", "band_mapping": "reverse",
    }
    assert expected["style"] == "both"
    assert expected["wave"] == {
        "stroke_width": 3, "colour": "#add8ff", "fill_opacity": 0.24, "glow_radius": 8,
        "glow_opacity": 0.45, "bar_opacity": 0.25,
        "stereo": {"left_colour": "#ffaaaa", "left_weight": 0.65, "right_weight": 0.9},
    }
    assert expected["soft_knee"] == 0.6 and expected["response_power"] == 0.25

    authored = builder().add_spectrum2d_clip(
        start=0, duration=0.2, layer=1, layout=path, style="both", wave=wave,
        soft_knee=0.6, response_power=0.25,
    )
    assert authored.to_canonical()["source"] == expected

    from vestra.lowering import LoweringContext

    project = vestra.Project(size=(64, 64), fps=10, duration=0.2)
    project.root.add(source)
    lowered = builder()
    LoweringContext(lowered).lower_composition(project.root)
    canonical = lowered.to_dict()
    assert canonical["visual"]["clips"][0]["source"] == expected
    assert not list(VALIDATOR.iter_errors(canonical))


def test_stereo_glow_preset_settings_can_be_replaced_or_cleared() -> None:
    preset = vestra.Spectrum2D(preset="stereo_glow")
    assert preset.style == "both" and preset.wave is not None and preset.wave.stereo is not None
    assert 0 < preset.wave.bar_opacity < 1 and preset.wave.glow_radius > 0
    assert preset.wave.stereo.left_weight != preset.wave.stereo.right_weight
    assert "preset" not in preset.to_canonical()

    layout = vestra.Spectrum2DEllipseLayout(0.4, 0.3, start_angle_degrees=180, sweep_angle_degrees=180)
    patched = vestra.Spectrum2D(
        preset="stereo_glow", layout=layout, style="wave", wave=None, soft_knee=None, response_power=1,
    )
    canonical = patched.to_canonical()
    assert canonical["layout"] == {
        "type": "ellipse", "radius_x": 0.4, "radius_y": 0.3,
        "start_angle_degrees": 180, "sweep_angle_degrees": 180,
        "direction": "outward", "band_mapping": "forward",
    }
    assert canonical["style"] == "wave"
    assert not {"wave", "soft_knee", "response_power"} & canonical.keys()
    assert canonical["band_count"] == 32

    advanced = _preset_clip(
        "stereo_glow", layout=layout, style="wave", wave=None, soft_knee=None, response_power=1,
    )
    assert advanced["source"] == canonical


def test_spectrum2d_new_settings_reject_invalid_geometry_and_response() -> None:
    with pytest.raises(ValueError, match="radius_y"):
        vestra.Spectrum2DEllipseLayout(0.4, 0)
    with pytest.raises(ValueError, match="sweep_angle_degrees"):
        vestra.Spectrum2DEllipseLayout(0.4, 0.3, sweep_angle_degrees=361)
    with pytest.raises(ValueError, match="differ"):
        vestra.Spectrum2DPathLayout([(0, 0), (1, 1), (0, 0)], closed=True)
    with pytest.raises(ValueError, match="differ"):
        vestra.Spectrum2DPathLayout([(0, 0), (0, 0)])
    with pytest.raises(ValueError, match="normalized"):
        vestra.Spectrum2DPathLayout([(0, -1), (1, 1)])
    with pytest.raises(ValueError, match="glow_radius"):
        vestra.Spectrum2DWave(glow_radius=float("inf"))
    with pytest.raises(ValueError, match="left_weight"):
        vestra.Spectrum2DStereo(left_weight=-1)
    with pytest.raises(ValueError, match="soft_knee"):
        vestra.Spectrum2D(soft_knee=1)
    with pytest.raises(ValueError, match="response_power"):
        vestra.Spectrum2D(response_power=0)


def _write_channel_tones(path: Path) -> None:
    payload = bytearray(96_000 * 4)
    for frame in range(96_000):
        tone = round(16_000 * math.sin(math.tau * 440 * frame / 48_000))
        struct.pack_into(
            "<hh",
            payload,
            frame * 4,
            tone if 12_000 <= frame < 36_000 else 0,
            tone if 48_000 <= frame < 72_000 else 0,
        )
    with wave.open(str(path), "wb") as output:
        output.setparams((2, 2, 48_000, 0, "NONE", "not compressed"))
        output.writeframes(payload)


def _prepare_spectrum(project: vestra.Project, backend: str) -> vestra.PreparedProject:
    try:
        return project.prepare(backend=backend)
    except vestra.PreparationError as error:
        unavailable = {"WGPU-ADAPTER-NOT-FOUND", "WGPU-NO-COMPATIBLE-ADAPTER"}
        if (
            backend != "wgpu"
            or os.environ.get("VESTRA_REQUIRE_WGPU") == "1"
            or not error.diagnostics
            or any(diagnostic.code not in unavailable for diagnostic in error.diagnostics)
        ):
            raise
        pytest.skip("no compatible WGPU adapter")


@pytest.mark.parametrize("backend", ["cpu", "wgpu"])
def test_stereo_spectrum_renders_only_the_active_channel(tmp_path: Path, backend: str) -> None:
    audio = tmp_path / "channels.wav"
    _write_channel_tones(audio)
    project = vestra.Project(size=(96, 72), fps=20, duration=2, background="#00000000", output_audio=False)
    project.audio.track("stereo").add(audio)
    project.root.add(vestra.Spectrum2D(
        preset="stereo_glow", style="wave", attack_seconds=0, release_seconds=0,
        wave=vestra.Spectrum2DWave(
            glow_radius=4,
            stereo=vestra.Spectrum2DStereo(left_color="#ff0000", right_color="#00ff00"),
        ),
    ))
    prepared = _prepare_spectrum(project, backend)
    right = prepared.render_frame_number(25).to_bytes()
    left = prepared.render_frame_number(10).to_bytes()
    silent = prepared.render_frame_number(1).to_bytes()
    assert max(left[0::4]) == 255 and max(left[1::4]) == 0
    assert max(right[1::4]) == 255 and max(right[0::4]) == 0
    assert max(silent[3::4]) == 0
    assert prepared.render_frame_number(25).to_bytes() == right


@pytest.mark.parametrize("backend", ["cpu", "wgpu"])
def test_ellipse_mask_clips_the_wave_and_halo(tmp_path: Path, backend: str) -> None:
    audio = tmp_path / "left.wav"
    _write_channel_tones(audio)
    size = (128, 96)
    rx, ry = 32, 19.2

    def frame(invert: bool | None) -> bytes:
        project = vestra.Project(size=size, fps=20, duration=2, background="#00000000", output_audio=False)
        project.audio.track("audio").add(audio)
        spectrum = project.root.add(vestra.Spectrum2D(
            layout=vestra.Spectrum2DEllipseLayout(
                rx / size[0], ry / size[1], start_angle_degrees=270,
                sweep_angle_degrees=180, direction="both",
            ),
            x=0, y=0, width=1, height=1, band_count=1,
            min_hz=300, max_hz=600, response_power=0.5, sensitivity=1.5,
            style="wave", wave=vestra.Spectrum2DWave(fill_opacity=0.6, glow_radius=6),
        ))
        if invert is not None:
            mask = spectrum.masks.add(vestra.Ellipse(width=2 * rx, height=2 * ry, fill="#ffffff"))
            mask.invert = invert
        return _prepare_spectrum(project, backend).render_frame_number(10).to_bytes()

    unmasked, inside, outside = frame(None), frame(False), frame(True)
    inside_samples = []
    outside_samples = []
    for y in range(size[1]):
        for x in range(size[0]):
            distance = math.hypot((x + 0.5 - 64) / rx, (y + 0.5 - 48) / ry)
            alpha = (y * size[0] + x) * 4 + 3
            if distance < 0.85:
                inside_samples.append(alpha)
            elif distance > 1.15:
                outside_samples.append(alpha)
    for samples, kept, clipped in [(inside_samples, inside, outside), (outside_samples, outside, inside)]:
        assert any(unmasked[i] > 0 for i in samples)
        assert all(kept[i] == unmasked[i] and clipped[i] == 0 for i in samples)
