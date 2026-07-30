"""Phase 8C-B authoring, ordering, scope, and native round-trip checks."""

from copy import deepcopy
from pathlib import Path
from typing import get_type_hints

import pytest

import video_editor
from video_editor import FrameRate
from video_editor.authoring import ActiveInterval, BlendMode, BrightnessEffect, Interpolation, Point, ProjectBuilder, Sizing, VignetteEffect
from video_editor.authoring.effects import ClipEffectCollection, PostEffectCollection


def builder() -> tuple[ProjectBuilder, object]:
    authored = ProjectBuilder(width=16, height=16, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=2)
    return authored, authored.add_solid_color_clip(colour="#808080", start=0, duration=2, layer=0)


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
    frame = video_editor.Editor().prepare(
        authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
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
    prepared = video_editor.Editor().prepare(authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU))
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
    assert get_type_hints(ClipEffectCollection.add_brightness)["return"] is BrightnessEffect
    assert get_type_hints(PostEffectCollection.add_vignette)["return"] is VignetteEffect


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
    frame = video_editor.Editor().prepare(
        authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
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
        return video_editor.Editor().prepare(
            authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
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
        return video_editor.Editor().prepare(
            authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
        ).render_frame_number(5).to_bytes()

    baseline = render(None)
    result = render(add_effect)
    assert sum(left != right for left, right in zip(result, baseline)) == changed_channels


def test_vignette_and_motion_blur_have_stable_cpu_frame_regions() -> None:
    vignette = ProjectBuilder(width=8, height=8, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=1)
    vignette_clip = vignette.add_solid_color_clip(colour="#ffffff", start=0, duration=1, layer=0)
    vignette_clip.effects.add_vignette(amount=1, radius=0.2, softness=0.5, colour="#000000")
    vignette_frame = video_editor.Editor().prepare(
        vignette.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    ).render_frame_number(0).to_bytes()
    assert vignette_frame[:4] == bytes((0, 0, 0, 255))
    assert vignette_frame[(4 * 8 + 4) * 4:(4 * 8 + 5) * 4] == bytes((255, 255, 255, 255))

    def render_motion(animated: bool) -> bytes:
        authored = ProjectBuilder(width=8, height=6, frame_rate=FrameRate(10, 1), output_path="out.mp4",
                                  duration=1, base_directory=Path.cwd())
        asset = authored.add_image_asset("tests/assets/wgpu-small-rgba.png")
        clip = authored.add_image_clip(source=asset, start=0, duration=1, layer=0,
                                       sizing=Sizing.stretch(width=4, height=3))
        if animated:
            clip.transform.position.keyframe(time=0, value=Point(0.25, 0.5))
            clip.transform.position.keyframe(time=1, value=Point(0.75, 0.5))
            clip.effects.add_motion_blur(intensity=0.8, shutter_angle=180, max_radius=8, samples=8)
        return video_editor.Editor().prepare(
            authored.build(), video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
        ).render_frame_number(5).to_bytes()

    assert render_motion(True) != render_motion(False)
