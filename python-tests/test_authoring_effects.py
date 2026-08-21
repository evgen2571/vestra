"""Phase 8C-B authoring, ordering, scope, and native round-trip checks."""

from copy import deepcopy
import json
from pathlib import Path
import inspect
import subprocess
from typing import get_type_hints

import pytest

import vestra
from vestra import FrameRate
from vestra.authoring import (
    ActiveInterval, BloomEffect, BlendMode, BrightnessEffect, CameraShakeEffect, ChromaticAberrationEffect,
    ColorAdjustEffect, ContrastEffect, DirectionalBlurEffect, GaussianBlurEffect, GlowEffect,
    CubicBezier, Interpolation, MotionBlurEffect, Point, ProjectBuilder, SaturationEffect, SharpenEffect,
    Sizing, TintEffect, VignetteEffect, ZoomBlurDirection, ZoomBlurEffect, available_effects, effect_definition,
)
from vestra.authoring.effects import ClipEffectCollection, PostEffectCollection
from vestra.authoring.tracks import ScalarTrack


def builder() -> tuple[ProjectBuilder, object]:
    authored = ProjectBuilder(width=16, height=16, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=2)
    return authored, authored.add_solid_color_clip(colour="#808080", start=0, duration=2, layer=0)


def _valid_generic_parameters(definition: object) -> dict[str, object]:
    parameters = definition["parameters"]  # type: ignore[index]
    values: dict[str, object] = {}
    for parameter in parameters:
        kind = parameter["kind"]
        if not parameter["required"] and parameter["default"] is not None:
            continue
        if kind in {"scalar_property", "plain_track", "number"}:
            if parameter["name"] == "black_point":
                values[str(parameter["name"])] = 0.25
                continue
            if parameter["name"] == "white_point":
                values[str(parameter["name"])] = 0.75
                continue
            minimum, maximum = parameter["minimum"], parameter["maximum"]
            values[str(parameter["name"])] = (
                (float(minimum) + float(maximum)) / 2
                if minimum is not None and maximum is not None
                else (float(minimum) + 1.0 if minimum is not None else 0.5)
            )
        elif kind == "colour":
            values[str(parameter["name"])] = "#ffffff"
        elif kind == "integer":
            values[str(parameter["name"])] = int(parameter["integer_minimum"])
        elif kind == "point2d":
            values[str(parameter["name"])] = Point(0.5, 0.5)
        elif kind == "boolean":
            values[str(parameter["name"])] = True
        elif kind == "enum":
            values[str(parameter["name"])] = str(parameter["enum_values"][0])
        elif kind == "active_interval":
            values[str(parameter["name"])] = ActiveInterval()
    return values


def test_generic_authoring_smoke_covers_every_registered_visual_effect() -> None:
    authored = ProjectBuilder(width=16, height=16, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=2)
    asset = authored.add_image_asset("fixture.png")
    clip = authored.add_image_clip(source=asset, start=0, duration=2, layer=0)
    for definition in available_effects():
        clip.effects.add_effect(str(definition["id"]), **_valid_generic_parameters(definition))
    assert len(clip.effects.items) == len(available_effects())
    assert authored.validate().is_valid


def test_every_effect_factory_serializes_in_declaration_order_and_validates() -> None:
    authored, clip = builder()
    effects = clip.effects
    effects.add_brightness(amount=0.1)
    effects.add_contrast(amount=1.1)
    effects.add_saturation(amount=0.8)
    effects.add_tint(colour="#112233", amount=0.5)
    effects.add_gaussian_blur(radius=2)
    effects.add_directional_blur(radius=2, angle_degrees=45)
    effects.add_zoom_blur(radius=2, samples=8, anchor=Point(0.5, 0.5))
    effects.add_glow(threshold=0.3, radius=2, intensity=0.4, colour="#ffffff")
    effects.add_chromatic_aberration(amount=1, angle_degrees=45)
    effects.add_vignette(amount=0.2, radius=0.8, softness=0.3, colour="#000000")
    effects.add_sharpen(amount=0.3, radius=1)
    effects.add_color_adjust(exposure=0, gamma=1, black_point=0, white_point=1)
    effects.add_camera_shake(active_interval=ActiveInterval(0.1, 0.5), position_amount=0.01,
                             rotation_degrees=1, scale_amount=0.01, frequency=14, seed=7,
                             attack=0.03, decay=0.2)
    effects.add_motion_blur(intensity=0.2, shutter_angle=180, max_radius=8, samples=8)
    expected = ["brightness", "contrast", "saturation", "tint", "gaussian_blur", "directional_blur",
                "zoom_blur", "glow", "chromatic_aberration", "vignette", "sharpen", "color_adjust",
                "camera_shake", "motion_blur"]
    assert clip.effects is effects
    assert [effect.kind for effect in effects.items] == expected
    assert [effect["type"] for effect in authored.to_dict()["visual"]["clips"][0]["effects"]] == expected
    assert authored.validate().is_valid
    frame = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    ).render_frame_number(5)
    assert frame.to_bytes() != bytes(16 * 16 * 4)


def test_effect_ids_are_scoped_and_failed_additions_are_transactional() -> None:
    authored, first = builder()
    second = authored.add_solid_color_clip(colour="#000000", start=0, duration=2, layer=1)
    assert first.effects.add_brightness(amount=0, id="same").id == "same"
    assert second.effects.add_brightness(amount=0, id="same").id == "same"
    assert authored.post_effects.add_brightness(amount=0, id="same").id == "same"
    before = deepcopy(authored.to_dict())
    with pytest.raises(ValueError):
        first.effects.add_gaussian_blur(radius=float("nan"), id="blur")
    assert authored.to_dict() == before
    assert first.effects.add_gaussian_blur(radius=2, id="blur").id == "blur"
    with pytest.raises(ValueError):
        first.effects.add_brightness(amount=0, id="blur")


def test_effect_tracks_post_effects_and_blend_modes_render() -> None:
    authored, clip = builder()
    clip.blend_mode = BlendMode.SCREEN
    effect = clip.effects.add_brightness(amount=0)
    effect.amount.keyframe(time=0, value=0)
    effect.amount.keyframe(time=1, value=0.2, interpolation=Interpolation.EASE_OUT)
    post = authored.post_effects
    post.add_vignette(amount=0.1, radius=0.8, softness=0.3, colour="#000000")
    post.add_contrast(amount=1)
    assert post is authored.post_effects
    assert [effect.kind for effect in post.items] == ["vignette", "contrast"]
    data = authored.to_dict()
    assert data["visual"]["clips"][0]["blend_mode"] == "screen"
    assert [effect["type"] for effect in data["visual"]["post_effects"]] == ["vignette", "contrast"]
    prepared = vestra.Editor().prepare(authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU))
    assert prepared.render_frame_number(0).to_bytes() != prepared.render_frame_number(10).to_bytes()


def test_effect_animation_has_snapshot_isolation() -> None:
    authored, clip = builder()
    effect = clip.effects.add_brightness(amount=0)
    native_before = authored.build()
    effect.amount.keyframe(time=1, value=0.2)
    assert native_before.to_dict()["visual"]["clips"][0]["effects"][0]["amount"] == {
        "base_value": 0.0, "keyframes": [],
    }
    assert authored.to_dict()["visual"]["clips"][0]["effects"][0]["amount"]["keyframes"][0]["time"] == 1.0


def test_effect_tracks_use_their_source_backed_time_domains() -> None:
    clip_project = ProjectBuilder(
        width=4,
        height=4,
        frame_rate=FrameRate(2, 1),
        output_path="out.mp4",
        duration=4,
        background="#000000",
    )
    clip = clip_project.add_solid_color_clip(
        colour="#ffffff", start=2, duration=2, layer=0,
    )
    brightness = clip.effects.add_brightness(amount=0)
    brightness.amount.keyframe(time=0.5, value=-1)
    clip_prepared = vestra.Editor().prepare(
        clip_project.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    assert clip_prepared.render_frame_number(4).to_bytes()[0] == 255
    assert clip_prepared.render_frame_number(5).to_bytes()[0] == 0

    post_project = ProjectBuilder(
        width=4,
        height=4,
        frame_rate=FrameRate(2, 1),
        output_path="out.mp4",
        duration=2,
        background="#000000",
    )
    post_project.add_solid_color_clip(colour="#ffffff", start=0, duration=2, layer=0)
    post_brightness = post_project.post_effects.add_brightness(amount=0)
    post_brightness.amount.keyframe(time=0.5, value=-1)
    post_prepared = vestra.Editor().prepare(
        post_project.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    assert post_prepared.render_frame_number(0).to_bytes()[0] == 255
    assert post_prepared.render_frame_number(1).to_bytes()[0] == 0


def test_camera_shake_track_time_starts_at_its_active_interval() -> None:
    authored = ProjectBuilder(
        width=8,
        height=6,
        frame_rate=FrameRate(10, 1),
        output_path="out.mp4",
        duration=2,
        base_directory=Path.cwd(),
    )
    asset = authored.add_image_asset("tests/assets/wgpu-small-rgba.png")
    clip = authored.add_image_clip(
        source=asset,
        start=0,
        duration=2,
        layer=0,
        sizing=Sizing.stretch(width=4, height=3),
    )
    shake = clip.effects.add_camera_shake(
        active_interval=ActiveInterval(0.5, 1),
        position_amount=0,
        rotation_degrees=0,
        scale_amount=0,
        frequency=8,
        seed=7,
        attack=0,
        decay=1,
    )
    shake.position_amount.keyframe(time=0, value=0)
    shake.position_amount.keyframe(time=0.5, value=0.25)
    prepared = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    before = prepared.render_frame_number(4).to_bytes()
    at_active_start = prepared.render_frame_number(5).to_bytes()
    after_local_keyframe = prepared.render_frame_number(10).to_bytes()
    assert at_active_start == before
    assert after_local_keyframe != at_active_start


def test_zero_camera_shake_matches_no_shake() -> None:
    authored, clip = builder()
    baseline = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    ).render_frame_number(5).to_bytes()
    clip.effects.add_camera_shake(
        position_amount=0, rotation_degrees=0, scale_amount=0, frequency=8, seed=7, attack=0, decay=1,
    )
    shaken = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    ).render_frame_number(5).to_bytes()
    assert shaken == baseline


def test_effect_collection_and_identity_state_are_read_only() -> None:
    authored, clip = builder()
    effect = clip.effects.add_brightness(amount=0)
    with pytest.raises(AttributeError):
        clip.effects = object()  # type: ignore[misc]
    with pytest.raises(AttributeError):
        authored.post_effects = object()  # type: ignore[misc]
    with pytest.raises(AttributeError):
        effect.id = "changed"  # type: ignore[misc]
    with pytest.raises(AttributeError):
        effect.kind = "contrast"  # type: ignore[misc]
    with pytest.raises(AttributeError):
        clip.effects.items.append(effect)  # type: ignore[attr-defined]
    with pytest.raises(TypeError):
        clip.blend_mode = "screen"  # type: ignore[assignment]
    assert not hasattr(authored.post_effects, "add_camera_shake")
    assert not hasattr(authored.post_effects, "add_motion_blur")


def test_public_effect_factory_annotations_resolve_at_runtime() -> None:
    collections = (ClipEffectCollection, PostEffectCollection)
    for collection in collections:
        for cls in collection.__mro__:
            for name, method in vars(cls).items():
                if name.startswith("add_"):
                    hints = get_type_hints(method)
                    assert "return" in hints
                    assert all("_Owner" not in str(hint) for hint in hints.values())
    assert get_type_hints(ClipEffectCollection.add_brightness)["return"] is BrightnessEffect
    assert get_type_hints(PostEffectCollection.add_vignette)["return"] is VignetteEffect


def test_effect_public_properties_match_the_canonical_model() -> None:
    authored, clip = builder()
    sharpen: SharpenEffect = clip.effects.add_sharpen(amount=0.3, radius=1)
    vignette: VignetteEffect = clip.effects.add_vignette(
        amount=0.2, radius=0.8, softness=0.3, colour="#000000",
    )
    chromatic: ChromaticAberrationEffect = clip.effects.add_chromatic_aberration(
        amount=0.02, angle_degrees=45,
    )
    assert sharpen.amount is not sharpen.radius
    assert not hasattr(sharpen, "angle_degrees")
    assert not hasattr(vignette, "threshold")
    assert not hasattr(vignette, "intensity")
    assert not hasattr(chromatic, "radius")
    sharpen.amount.base_value = 0.5
    sharpen.radius.base_value = 2
    chromatic.amount.base_value = 0.1
    chromatic.angle_degrees.base_value = 90
    data = authored.to_dict()["visual"]["clips"][0]["effects"]
    assert data[0] == {"id": sharpen.id, "type": "sharpen", "amount": {"base_value": 0.5}, "radius": {"base_value": 2.0}}
    assert data[2] == {"id": chromatic.id, "type": "chromatic_aberration", "amount": {"base_value": 0.1}, "angle_degrees": {"base_value": 90.0}}


def test_every_effect_has_the_exact_public_property_contract() -> None:
    authored, clip = builder()
    effects = (
        clip.effects.add_brightness(amount=0),
        clip.effects.add_contrast(amount=1),
        clip.effects.add_saturation(amount=1),
        clip.effects.add_tint(colour="#000000", amount=0),
        clip.effects.add_gaussian_blur(radius=0),
        clip.effects.add_directional_blur(radius=0, angle_degrees=0),
        clip.effects.add_zoom_blur(radius=0, samples=2, anchor=Point(0.5, 0.5)),
        clip.effects.add_glow(threshold=0, radius=0, intensity=0, colour="#ffffff"),
        clip.effects.add_chromatic_aberration(amount=0, angle_degrees=0),
        clip.effects.add_vignette(amount=0, radius=1, softness=0, colour="#000000"),
        clip.effects.add_sharpen(amount=0, radius=1),
        clip.effects.add_color_adjust(exposure=0, gamma=1, black_point=0, white_point=1),
        clip.effects.add_camera_shake(
            position_amount=0,
            rotation_degrees=0,
            scale_amount=0,
            frequency=1,
            seed=0,
            attack=0,
            decay=1,
        ),
        clip.effects.add_motion_blur(intensity=0, shutter_angle=0, max_radius=0, samples=2),
    )
    expected = (
        (BrightnessEffect, {"id", "kind", "amount"}),
        (ContrastEffect, {"id", "kind", "amount"}),
        (SaturationEffect, {"id", "kind", "amount"}),
        (TintEffect, {"id", "kind", "colour", "amount"}),
        (GaussianBlurEffect, {"id", "kind", "radius"}),
        (DirectionalBlurEffect, {"id", "kind", "radius", "angle_degrees"}),
        (ZoomBlurEffect, {"id", "kind", "radius", "samples", "anchor", "direction"}),
        (GlowEffect, {"id", "kind", "threshold", "radius", "intensity", "colour"}),
        (ChromaticAberrationEffect, {"id", "kind", "amount", "angle_degrees"}),
        (VignetteEffect, {"id", "kind", "amount", "radius", "softness", "colour"}),
        (SharpenEffect, {"id", "kind", "amount", "radius"}),
        (ColorAdjustEffect, {"id", "kind", "exposure", "gamma", "black_point", "white_point"}),
        (CameraShakeEffect, {"id", "kind", "active_interval", "position_amount", "rotation_degrees", "scale_amount", "frequency", "seed", "attack", "decay"}),
        (MotionBlurEffect, {"id", "kind", "intensity", "shutter_angle", "max_radius", "samples"}),
    )
    for effect, (effect_type, properties) in zip(effects, expected, strict=True):
        public_properties = {
            name
            for cls in type(effect).__mro__
            for name, descriptor in vars(cls).items()
            if isinstance(descriptor, property)
        }
        assert type(effect) is effect_type
        assert public_properties == properties
        for property_name in properties - {"id", "kind", "active_interval", "seed", "attack", "decay", "samples", "anchor", "direction", "colour"}:
            track = getattr(effect, property_name)
            assert isinstance(track, ScalarTrack)
            assert getattr(effect, property_name) is track
            track.base_value = 0.123
            with pytest.raises(AttributeError):
                setattr(effect, property_name, track)
    canonical = authored.to_dict()["visual"]["clips"][0]["effects"]
    for effect_data, (_, properties) in zip(canonical, expected, strict=True):
        for property_name in properties:
            value = effect_data.get(property_name)
            if isinstance(value, dict) and "base_value" in value:
                assert value["base_value"] == 0.123


def test_effect_collections_hide_owner_construction_and_integer_ranges_are_transactional() -> None:
    assert str(inspect.signature(ClipEffectCollection)) == "() -> 'None'"
    assert str(inspect.signature(PostEffectCollection)) == "() -> 'None'"
    with pytest.raises(TypeError, match="obtained"):
        ClipEffectCollection()
    with pytest.raises(TypeError, match="obtained"):
        PostEffectCollection()
    authored, clip = builder()
    before = authored.to_dict()
    for value in (1, 0, -1, 33, 300, True, 2.5, "8"):
        with pytest.raises((TypeError, ValueError)):
            clip.effects.add_zoom_blur(radius=1, samples=value, anchor=Point(0.5, 0.5))  # type: ignore[arg-type]
        assert authored.to_dict() == before
    for value in (1, 0, -1, 33, 300, True, 2.5, "8"):
        with pytest.raises((TypeError, ValueError)):
            clip.effects.add_motion_blur(intensity=1, shutter_angle=180, max_radius=4, samples=value)  # type: ignore[arg-type]
        assert authored.to_dict() == before
    for value in (-1, True, 1.5, 2**64):
        with pytest.raises((TypeError, ValueError)):
            clip.effects.add_camera_shake(position_amount=0, rotation_degrees=0, scale_amount=0,
                                           frequency=1, seed=value, attack=0, decay=0.1)  # type: ignore[arg-type]
        assert authored.to_dict() == before
    assert clip.effects.add_zoom_blur(radius=1, samples=2, anchor=Point(0.5, 0.5)).id == "effect-000001"


@pytest.mark.parametrize(
    ("mode", "expected"),
    [
        (BlendMode.NORMAL, bytes((64, 128, 192, 255))),
        (BlendMode.ADD, bytes((192, 192, 224, 255))),
        (BlendMode.SCREEN, bytes((160, 160, 200, 255))),
        (BlendMode.MULTIPLY, bytes((32, 32, 24, 255))),
        (BlendMode.OVERLAY, bytes((65, 64, 48, 255))),
    ],
)
def test_every_blend_mode_has_a_stable_cpu_pixel(mode: BlendMode, expected: bytes) -> None:
    authored = ProjectBuilder(width=2, height=2, frame_rate=FrameRate(1, 1), output_path="out.mp4", duration=1)
    authored.add_solid_color_clip(colour="#804020", start=0, duration=1, layer=0)
    top = authored.add_solid_color_clip(colour="#4080c0", start=0, duration=1, layer=1)
    top.blend_mode = mode
    frame = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    ).render_frame_number(0)
    assert frame.to_bytes()[:4] == expected


def test_clip_and_post_effect_declaration_order_changes_cpu_pixels() -> None:
    def render(*, post: bool, reverse: bool) -> bytes:
        authored = ProjectBuilder(width=2, height=2, frame_rate=FrameRate(1, 1), output_path="out.mp4", duration=1)
        if post:
            authored.add_solid_color_clip(colour="#400000", start=0, duration=1, layer=0)
            authored.add_solid_color_clip(colour="#004000", start=0, duration=1, layer=1)
            collection = authored.post_effects
        else:
            clip = authored.add_solid_color_clip(colour="#400000", start=0, duration=1, layer=0)
            collection = clip.effects
        if reverse:
            collection.add_tint(colour="#0000ff", amount=0.5)
            collection.add_brightness(amount=0.1)
        else:
            collection.add_brightness(amount=0.1)
            collection.add_tint(colour="#0000ff", amount=0.5)
        return vestra.Editor().prepare(
            authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
        ).render_frame_number(0).to_bytes()[:4]

    assert render(post=False, reverse=False) == bytes((45, 13, 140, 255))
    assert render(post=False, reverse=True) == bytes((58, 26, 153, 255))
    # The post-effect result combines red and green layers first, proving its scope is the canvas.
    assert render(post=True, reverse=False) == bytes((13, 45, 141, 255))
    assert render(post=True, reverse=True) == bytes((26, 58, 154, 255))


@pytest.mark.parametrize(
    ("add_effect", "changed_channels"),
    [
        (lambda effects: effects.add_brightness(amount=0.2), 42),
        (lambda effects: effects.add_contrast(amount=0.5), 41),
        (lambda effects: effects.add_saturation(amount=0), 38),
        (lambda effects: effects.add_tint(colour="#ff0000", amount=0.5), 39),
        (lambda effects: effects.add_gaussian_blur(radius=2), 113),
        (lambda effects: effects.add_directional_blur(radius=2, angle_degrees=45), 102),
        (lambda effects: effects.add_zoom_blur(radius=2, samples=8, anchor=Point(0.5, 0.5)), 99),
        (lambda effects: effects.add_glow(threshold=0, radius=2, intensity=1, colour="#ffffff"), 111),
        (lambda effects: effects.add_chromatic_aberration(amount=2, angle_degrees=45), 27),
        (lambda effects: effects.add_sharpen(amount=0.5, radius=2), 37),
        (lambda effects: effects.add_color_adjust(exposure=0.2, gamma=1, black_point=0, white_point=1), 33),
        (lambda effects: effects.add_camera_shake(active_interval=ActiveInterval(), position_amount=0.1,
                                                   rotation_degrees=5, scale_amount=0.1, frequency=14,
                                                   seed=7, attack=0.03, decay=0.2), 24),
    ],
)
def test_individual_effects_have_stable_cpu_frame_behavior(add_effect: object, changed_channels: int) -> None:
    def render(effect_adder: object | None) -> bytes:
        authored = ProjectBuilder(width=8, height=6, frame_rate=FrameRate(10, 1), output_path="out.mp4",
                                  duration=1, base_directory=Path.cwd())
        asset = authored.add_image_asset("tests/assets/wgpu-small-rgba.png")
        clip = authored.add_image_clip(source=asset, start=0, duration=1, layer=0,
                                       sizing=Sizing.stretch(width=8, height=6))
        if effect_adder is not None:
            effect_adder(clip.effects)  # type: ignore[operator]
        return vestra.Editor().prepare(
            authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
        ).render_frame_number(5).to_bytes()

    baseline = render(None)
    result = render(add_effect)
    assert sum(left != right for left, right in zip(result, baseline)) == changed_channels


def test_vignette_and_motion_blur_have_stable_cpu_frame_regions() -> None:
    vignette = ProjectBuilder(width=8, height=8, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1)
    vignette_clip = vignette.add_solid_color_clip(colour="#ffffff", start=0, duration=1, layer=0)
    vignette_clip.effects.add_vignette(amount=1, radius=0.2, softness=0.5, colour="#000000")
    vignette_frame = vestra.Editor().prepare(
        vignette.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    ).render_frame_number(0).to_bytes()
    assert vignette_frame[:4] == bytes((0, 0, 0, 255))
    assert vignette_frame[(4 * 8 + 4) * 4:(4 * 8 + 5) * 4] == bytes((255, 255, 255, 255))

    def render_motion(with_effect: bool) -> bytes:
        authored = ProjectBuilder(width=8, height=6, frame_rate=FrameRate(10, 1), output_path="out.mp4",
                                  duration=1, base_directory=Path.cwd())
        asset = authored.add_image_asset("tests/assets/wgpu-small-rgba.png")
        clip = authored.add_image_clip(source=asset, start=0, duration=1, layer=0,
                                       sizing=Sizing.stretch(width=4, height=3))
        clip.transform.position.keyframe(time=0, value=Point(0.25, 0.5))
        clip.transform.position.keyframe(time=1, value=Point(0.75, 0.5))
        if with_effect:
            clip.effects.add_motion_blur(intensity=0.8, shutter_angle=180, max_radius=8, samples=8)
        return vestra.Editor().prepare(
            authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
        ).render_frame_number(5).to_bytes()

    # The moving project is identical in both cases. Only motion blur changes.
    assert render_motion(True) != render_motion(False)


def test_cpu_effect_video_exercises_ordered_clip_and_post_effects(tmp_path: Path) -> None:
    authored = ProjectBuilder(
        width=16, height=12, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1,
    )
    authored.add_solid_color_clip(colour="#203040", start=0, duration=1, layer=0)
    foreground = authored.add_solid_color_clip(colour="#806020", start=0, duration=1, layer=1)
    foreground.blend_mode = BlendMode.SCREEN
    brightness = foreground.effects.add_brightness(amount=0)
    brightness.amount.keyframe(time=0, value=0)
    brightness.amount.keyframe(time=1, value=0.2, interpolation=Interpolation.EASE_OUT)
    foreground.effects.add_gaussian_blur(radius=1)
    authored.post_effects.add_vignette(amount=0.1, radius=0.8, softness=0.3, colour="#000000")
    authored.post_effects.add_color_adjust(exposure=0, gamma=1, black_point=0, white_point=1)
    assert authored.validate().is_valid
    prepared = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    output = tmp_path / "effects.mp4"
    result = prepared.render_video(vestra.PreparedVideoRenderRequest(output))
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "stream=codec_type,width,height,nb_frames,duration", "-of", "json", str(output)],
        check=True, capture_output=True, text=True,
    )
    streams = json.loads(probe.stdout)["streams"]
    assert output.is_file() and output.stat().st_size > 0
    assert result.total_frames == 10 and result.audio_present is False
    assert streams == [{"codec_type": "video", "width": 16, "height": 12, "duration": "1.000000", "nb_frames": "10"}]
    assert not list(tmp_path.glob("*.tmp"))


def test_generic_effect_catalog_is_read_only_and_complete() -> None:
    definitions = available_effects()
    ids = [str(definition["id"]) for definition in definitions]
    assert len(ids) == len(set(ids))
    assert set(ids) == {
        "brightness", "contrast", "saturation", "tint", "gaussian_blur", "directional_blur",
        "motion_tile", "zoom_blur", "radial_blur", "glow", "chromatic_aberration", "vignette", "sharpen", "color_adjust",
        "camera_shake", "motion_blur", "bloom",
    }
    with pytest.raises(TypeError):
        definitions[0]["id"] = "changed"  # type: ignore[index]
    glow = effect_definition("glow")
    with pytest.raises(TypeError):
        glow["parameters"] = ()  # type: ignore[index]
    with pytest.raises(TypeError):
        glow["parameters"][0]["name"] = "changed"  # type: ignore[index]
    direction = effect_definition("zoom_blur")["parameters"][-1]
    with pytest.raises(AttributeError):
        direction["enum_values"].append("invalid")  # type: ignore[union-attr]
    assert effect_definition("zoom_blur")["parameters"][-1]["enum_values"] == (
        "inward", "outward", "centered",
    )
    _, clip = builder()
    with pytest.raises(ValueError, match="one of"):
        clip.effects.add_effect(
            "zoom_blur", radius=1, samples=2, anchor=Point(0.5, 0.5), direction="invalid",
        )
    assert effect_definition("glow")["scope"] == "clip_and_global"


def test_generic_effect_matches_typed_serialization() -> None:
    typed_builder, typed_clip = builder()
    typed = typed_clip.effects.add_glow(threshold=0.8, radius=12, intensity=1.1, colour="#ffffff")
    generic_builder, generic_clip = builder()
    generic = generic_clip.effects.add_effect(
        "glow", threshold=0.8, radius=12, intensity=1.1, colour="#ffffff",
    )
    assert typed.to_canonical() == generic.to_canonical()
    animated_typed = typed_clip.effects.add_brightness(amount=0.2)
    animated_typed.amount.keyframe(time=0.5, value=0.8)
    animated_generic = generic_clip.effects.add_effect("brightness", amount=animated_typed.amount)
    assert animated_typed.to_canonical() == animated_generic.to_canonical()
    assert typed_builder.validate().is_valid and generic_builder.validate().is_valid


def test_generic_effect_copies_cubic_keyframes_and_signal_modifiers() -> None:
    typed_builder, typed_clip = builder()
    typed = typed_clip.effects.add_brightness(amount=0.2)
    typed.amount.keyframe(
        time=0.5,
        value=0.8,
        interpolation=CubicBezier(0.25, 0.1, 0.25, 1.0),
    )
    typed.amount.modulate(typed_builder.audio.master.rms(), mode="multiply")

    _, generic_clip = builder()
    generic = generic_clip.effects.add_effect("brightness", amount=typed.amount)

    assert generic.to_canonical() == typed.to_canonical()


def test_generic_camera_shake_preserves_explicit_interval_and_matches_typed() -> None:
    interval = ActiveInterval(start=2.5, duration=4.0)
    _, typed_clip = builder()
    typed = typed_clip.effects.add_camera_shake(
        active_interval=interval, position_amount=0, rotation_degrees=0, scale_amount=0,
        frequency=1, seed=1, attack=0, decay=1,
    )
    _, generic_clip = builder()
    generic = generic_clip.effects.add_effect(
        "camera_shake", active_interval=interval, position_amount=0, rotation_degrees=0,
        scale_amount=0, frequency=1, seed=1, attack=0, decay=1,
    )
    assert typed.to_canonical() == generic.to_canonical()
    assert generic.to_canonical()["start"] == 2.5
    assert generic.to_canonical()["duration"] == 4.0


def test_generic_camera_shake_uses_typed_default_interval() -> None:
    typed_builder, typed_clip = builder()
    typed = typed_clip.effects.add_camera_shake(
        position_amount=0, rotation_degrees=0, scale_amount=0, frequency=1,
        seed=1, attack=0, decay=1,
    )
    generic_builder, generic_clip = builder()
    generic = generic_clip.effects.add_effect(
        "camera_shake", position_amount=0, rotation_degrees=0, scale_amount=0,
        frequency=1, seed=1, attack=0, decay=1,
    )
    assert typed.to_canonical() == generic.to_canonical()
    assert generic.to_canonical()["start"] == 0.0
    assert "duration" not in generic.to_canonical()
    assert typed_builder.validate().is_valid and generic_builder.validate().is_valid


def test_generic_enum_metadata_and_simple_numeric_constraints() -> None:
    direction = effect_definition("zoom_blur")["parameters"][-1]
    assert direction["enum_values"] == ("inward", "outward", "centered")
    authored, clip = builder()
    effect = clip.effects.add_effect(
        "zoom_blur", radius=1, samples=2, anchor=Point(0.5, 0.5), direction=ZoomBlurDirection.CENTERED,
    )
    assert effect.to_canonical()["direction"] == "centered"
    with pytest.raises(ValueError, match="one of"):
        clip.effects.add_effect(
            "zoom_blur", radius=1, samples=2, anchor=Point(0.5, 0.5), direction="diagonal",
        )
    with pytest.raises(ValueError):
        clip.effects.add_effect(
            "camera_shake", position_amount=0, rotation_degrees=0, scale_amount=0,
            frequency=1, seed=1, attack=-0.1, decay=1,
        )
    with pytest.raises(ValueError):
        clip.effects.add_effect(
            "camera_shake", position_amount=0, rotation_degrees=0, scale_amount=0,
            frequency=1, seed=1, attack=0, decay=0,
        )
    assert authored.validate().is_valid


def test_typed_camera_shake_defers_descriptor_validation_to_project_validation() -> None:
    authored, clip = builder()
    shake = clip.effects.add_camera_shake(
        position_amount=0, rotation_degrees=0, scale_amount=0, frequency=1,
        seed=1, attack=-0.1, decay=1,
    )
    canonical = shake.to_canonical()
    assert canonical["attack"] == -0.1
    report = authored.validate()
    assert not report.is_valid


def test_typed_and_generic_camera_shake_valid_serialization_remains_equal() -> None:
    _, typed_clip = builder()
    typed = typed_clip.effects.add_camera_shake(
        position_amount=0, rotation_degrees=0, scale_amount=0, frequency=1,
        seed=1, attack=0, decay=1,
    )
    _, generic_clip = builder()
    generic = generic_clip.effects.add_effect(
        "camera_shake", position_amount=0, rotation_degrees=0, scale_amount=0,
        frequency=1, seed=1, attack=0, decay=1,
    )
    assert typed.to_canonical() == generic.to_canonical()


def test_generic_effect_rejects_invalid_calls_and_preserves_scope_rules() -> None:
    authored, clip = builder()
    with pytest.raises(ValueError, match="unknown visual effect"):
        clip.effects.add_effect("not_a_real_effect")
    with pytest.raises(TypeError, match="unknown parameter"):
        clip.effects.add_effect("glow", threshold=0.8, radius=12, intensity=1, colour="#ffffff", extra=1)
    with pytest.raises(TypeError, match="missing required parameter"):
        clip.effects.add_effect("glow", threshold=0.8, radius=12, intensity=1)
    with pytest.raises(ValueError, match="only valid on clips"):
        authored.post_effects.add_effect(
            "camera_shake", position_amount=0, rotation_degrees=0, scale_amount=0,
            frequency=1, seed=1, attack=0, decay=1,
        )


def test_generic_color_adjust_plain_track_bounds_cover_base_and_keyframes() -> None:
    authored, clip = builder()
    with pytest.raises(ValueError, match="outside its authored range"):
        clip.effects.add_effect(
            "color_adjust", exposure=0, gamma=1, black_point=1, white_point=1,
        )

    black_point = ScalarTrack._create(clip._owner, 0.999)
    black_point.keyframe(time=1, value=1)
    with pytest.raises(ValueError, match="outside its authored range"):
        clip.effects.add_effect(
            "color_adjust", exposure=0, gamma=1, black_point=black_point, white_point=1,
        )

    clip.effects.add_effect(
        "color_adjust", exposure=0, gamma=1, black_point=0.999, white_point=1,
    )


def test_bloom_typed_and_generic_authoring_share_canonicalization() -> None:
    _, typed_clip = builder()
    typed = typed_clip.effects.add_bloom(threshold=0.5, radius=2, intensity=0.75)
    _, generic_clip = builder()
    generic = generic_clip.effects.add_effect("bloom", threshold=0.5, radius=2, intensity=0.75)
    assert isinstance(typed, BloomEffect)
    typed_data = typed.to_canonical()
    generic_data = generic.to_canonical()
    typed_data.pop("id")
    generic_data.pop("id")
    assert typed_data == generic_data
    typed.threshold.keyframe(time=1, value=0.8)
    assert typed.to_canonical()["threshold"]["keyframes"]


def test_schema_effect_catalog_has_rust_catalog_ids_and_parameters() -> None:
    schema = json.loads((Path(__file__).parents[1] / "schemas/project.schema.json").read_text())
    schema_effects = {}
    for reference in schema["$defs"]["effect"]["oneOf"]:
        definition = schema["$defs"][reference["$ref"].rsplit("/", 1)[1]]
        effect_type = definition["properties"]["type"]
        ids = effect_type.get("enum", [effect_type.get("const")])
        for effect_id in ids:
            schema_effects[effect_id] = definition
    assert set(schema_effects) == {str(definition["id"]) for definition in available_effects()}
    for definition in available_effects():
        properties = schema_effects[definition["id"]]["properties"]
        for parameter in definition["parameters"]:
            name = parameter["name"]
            if parameter["kind"] == "active_interval":
                assert {"start", "duration"} <= set(properties)
            else:
                assert name in properties
                if parameter["kind"] == "enum":
                    assert tuple(properties[name]["enum"]) == parameter["enum_values"]
                if parameter["kind"] == "number":
                    key = "exclusiveMinimum" if parameter["minimum_exclusive"] else "minimum"
                    assert properties[name].get(key) == parameter["minimum"]
                if parameter["kind"] == "scalar_property" and parameter["minimum"] is not None:
                    authored = properties[name]["allOf"][1]["properties"]["base_value"]
                    key = "exclusiveMinimum" if parameter["minimum_exclusive"] else "minimum"
                    assert authored[key] == parameter["minimum"]


def test_checked_schema_is_fresh_from_rust_catalog(tmp_path: Path) -> None:
    generated = tmp_path / "project.schema.json"
    subprocess.run(
        ["cargo", "run", "-q", "-p", "vestra-cli", "--", "generate-schema", "--output", str(generated)],
        check=True,
    )
    assert generated.read_bytes() == (Path(__file__).parents[1] / "schemas/project.schema.json").read_bytes()
