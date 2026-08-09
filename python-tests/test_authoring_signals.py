import pytest

from video_editor import FrameRate
from video_editor.authoring import ProjectBuilder
from video_editor.authoring.values import Point


def test_master_signal_chains_are_immutable_and_serialize_in_order() -> None:
    builder = ProjectBuilder(width=16, height=16, frame_rate=FrameRate(24, 1), output_path="out.mp4")
    raw = builder.audio.master.band(40, 160)
    first = raw.gain(2).clamp(0, 1)
    second = raw.gain(3)
    assert raw.to_canonical() == {
        "source": {"type": "audio", "tap": "master", "feature": {"type": "band_energy", "min_hz": 40.0, "max_hz": 160.0}}
    }
    assert [item["type"] for item in first.to_canonical()["transforms"]] == ["gain", "clamp"]
    assert second.to_canonical()["transforms"] == [{"type": "gain", "gain": 3.0}]
    with pytest.raises(TypeError):
        raw._feature["type"] = "peak"  # type: ignore[index]
    with pytest.raises(TypeError):
        first._transforms[0]["gain"] = 99.0  # type: ignore[index]
    assert raw.to_canonical()["source"]["feature"] == {
        "type": "band_energy",
        "min_hz": 40.0,
        "max_hz": 160.0,
    }
    assert first.to_canonical()["transforms"] == [
        {"type": "gain", "gain": 2.0},
        {"type": "clamp", "min": 0.0, "max": 1.0},
    ]


def test_signals_are_immutable_values_and_can_be_reused_across_builders() -> None:
    first = ProjectBuilder(width=16, height=16, frame_rate=FrameRate(24, 1), output_path="one.mp4")
    second = ProjectBuilder(width=16, height=16, frame_rate=FrameRate(24, 1), output_path="two.mp4")
    raw = first.audio.master.rms()
    signal = raw.gain(2)
    image = second.add_image_asset("image.png")
    clip = second.add_image_clip(source=image, start=0, duration=1, layer=0)
    clip.opacity.modulate(signal)
    assert raw.to_canonical() == {"source": {"type": "audio", "tap": "master", "feature": {"type": "rms"}}}
    assert clip.to_canonical()["opacity"]["modifiers"][0]["signal"] == signal.to_canonical()


def test_scalar_and_uniform_scale_modifiers_serialize_as_canonical_targets() -> None:
    builder = ProjectBuilder(width=16, height=16, frame_rate=FrameRate(24, 1), output_path="out.mp4")
    image = builder.add_image_asset("image.png")
    clip = builder.add_image_clip(source=image, start=0, duration=1, layer=0)
    signal = builder.audio.master.rms().remap(0, 1, 1, 1.08)
    clip.opacity.modulate(signal, mode="multiply")
    clip.transform.scale.react_to(signal)
    canonical = clip.to_canonical()
    assert canonical["opacity"]["modifiers"][0]["operation"] == "multiply"
    assert canonical["transform"]["component_modifiers"] == {
        "scale_x": [{"operation": "multiply", "signal": signal.to_canonical()}],
        "scale_y": [{"operation": "multiply", "signal": signal.to_canonical()}],
    }
    assert not hasattr(clip.transform.anchor, "modulate")
    assert not hasattr(clip.transform.position_x, "base_value")
    assert not hasattr(clip.transform.position_x, "keyframe")
    assert not hasattr(clip.transform.scale_x, "base_value")
    assert not hasattr(clip.transform.scale_x, "keyframe")


def test_public_bass_scale_glow_and_high_band_reference_shapes() -> None:
    builder = ProjectBuilder(width=16, height=16, frame_rate=FrameRate(24, 1), output_path="out.mp4")
    image = builder.add_image_asset("image.png")
    clip = builder.add_image_clip(source=image, start=0, duration=1, layer=0)
    bass = (
        builder.audio.master.band(40, 160)
        .gain(1.5)
        .remap(0, 0.2, 0, 1)
        .clamp(0, 1)
        .envelope(0.02, 0.18)
        .response_curve(0.42, 0, 0.58, 1)
    )
    clip.transform.scale.react_to(bass.remap(0, 1, 1, 1.08), mode="multiply")
    clip.effects.add_glow(
        threshold=0.25, radius=8, intensity=0, colour="#ffffff"
    ).intensity.modulate(bass.remap(0, 1, 0, 5), mode="replace")
    clip.effects.add_chromatic_aberration(amount=0, angle_degrees=0).amount.modulate(builder.audio.master.band(2_000, 12_000).gain(2), mode="replace")
    canonical = clip.to_canonical()
    scales = canonical["transform"]["component_modifiers"]
    assert scales["scale_x"] == scales["scale_y"]
    assert scales["scale_x"][0]["signal"]["source"]["feature"] == {
        "type": "band_energy",
        "min_hz": 40.0,
        "max_hz": 160.0,
    }
    assert [item["type"] for item in scales["scale_x"][0]["signal"]["transforms"]] == ["gain", "remap", "clamp", "envelope", "response_curve", "remap"]
    assert canonical["effects"][0]["intensity"]["modifiers"][0]["operation"] == "replace"
    assert canonical["effects"][1]["amount"]["modifiers"][0]["signal"]["source"]["feature"] == {"type": "band_energy", "min_hz": 2000.0, "max_hz": 12000.0}


def test_all_public_signal_factories_and_modifier_targets_are_available() -> None:
    builder = ProjectBuilder(width=16, height=16, frame_rate=FrameRate(24, 1), output_path="out.mp4")
    image = builder.add_image_asset("image.png")
    clip = builder.add_image_clip(source=image, start=0, duration=1, layer=0)
    signal = (
        builder.audio.master.peak()
        .gain(-1)
        .remap(0, 1, 2, -2)
        .clamp(-1, 1)
        .envelope(0.02, 0.18)
        .response_curve(0.42, -1, 0.58, 1)
    )
    assert builder.audio.master.rms().to_canonical()["source"]["feature"] == {"type": "rms"}
    assert signal.to_canonical()["source"]["feature"] == {"type": "peak"}
    clip.transform.position_x.modulate(signal)
    clip.transform.position_y.modulate(signal)
    clip.transform.scale_x.modulate(signal)
    clip.transform.scale_y.modulate(signal)
    clip.transform.rotation_degrees.modulate(signal)
    effects = clip.effects
    targets = [
        effects.add_brightness(amount=0).amount,
        effects.add_contrast(amount=1).amount,
        effects.add_saturation(amount=1).amount,
        effects.add_tint(colour="#ffffff", amount=0).amount,
        effects.add_gaussian_blur(radius=0).radius,
        effects.add_directional_blur(radius=0, angle_degrees=0).radius,
        effects.add_directional_blur(radius=0, angle_degrees=0).angle_degrees,
        effects.add_zoom_blur(radius=0, samples=2, anchor=Point(0.5, 0.5)).radius,
        effects.add_glow(threshold=0, radius=0, intensity=0, colour="#ffffff").threshold,
        effects.add_glow(threshold=0, radius=0, intensity=0, colour="#ffffff").radius,
        effects.add_glow(threshold=0, radius=0, intensity=0, colour="#ffffff").intensity,
        effects.add_chromatic_aberration(amount=0, angle_degrees=0).amount,
        effects.add_chromatic_aberration(amount=0, angle_degrees=0).angle_degrees,
        effects.add_vignette(amount=0, radius=1, softness=0, colour="#000000").amount,
        effects.add_vignette(amount=0, radius=1, softness=0, colour="#000000").radius,
        effects.add_sharpen(amount=0, radius=1).amount,
        effects.add_sharpen(amount=0, radius=1).radius,
        effects.add_color_adjust(exposure=0, gamma=1, black_point=0, white_point=1).exposure,
        effects.add_color_adjust(exposure=0, gamma=1, black_point=0, white_point=1).gamma,
        effects.add_camera_shake(position_amount=0, rotation_degrees=0, scale_amount=0, frequency=1, seed=1, attack=0, decay=1).position_amount,
        effects.add_camera_shake(position_amount=0, rotation_degrees=0, scale_amount=0, frequency=1, seed=1, attack=0, decay=1).rotation_degrees,
        effects.add_camera_shake(position_amount=0, rotation_degrees=0, scale_amount=0, frequency=1, seed=1, attack=0, decay=1).scale_amount,
        effects.add_camera_shake(position_amount=0, rotation_degrees=0, scale_amount=0, frequency=1, seed=1, attack=0, decay=1).frequency,
        effects.add_motion_blur(intensity=0, shutter_angle=0, max_radius=0, samples=2).intensity,
        effects.add_motion_blur(intensity=0, shutter_angle=0, max_radius=0, samples=2).shutter_angle,
        effects.add_motion_blur(intensity=0, shutter_angle=0, max_radius=0, samples=2).max_radius,
    ]
    for target in targets:
        target.modulate(signal)
        assert target.to_canonical()["modifiers"][0]["signal"] == signal.to_canonical()
    vignette = effects.add_vignette(amount=0, radius=1, softness=0, colour="#000000")
    adjust = effects.add_color_adjust(exposure=0, gamma=1, black_point=0, white_point=1)
    assert not hasattr(vignette.softness, "modulate")
    assert not hasattr(adjust.black_point, "modulate")
    assert not hasattr(adjust.white_point, "modulate")
