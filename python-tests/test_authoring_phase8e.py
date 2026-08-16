"""Release-candidate conformance checks for the complete Phase 8 authoring API."""

from __future__ import annotations

from copy import deepcopy
from collections.abc import Callable
import inspect
import typing

import pytest

from vestra import Crossfade, FrameRate, ProjectSnapshot
from vestra import authoring as api
from vestra.authoring import (
    AudioAsset, AudioClip, AudioTimeline, AudioTrack, Crop, ImageAsset, ImageClip, Point, PresetCollection, ProjectBuilder,
    ScalarTrack, Sizing, SolidColorClip, Timeline, Transform,
)


PUBLIC_NAMES = {
    "ActiveInterval", "AudioAsset", "AudioClip", "AudioFadeCurve", "AudioGainInterpolation", "AudioGainKeyframe", "AudioTimeline", "AudioTrack", "AuthoringError", "BassBoostAudioEffect", "BloomEffect", "BlendMode", "BrightnessEffect",
    "CameraShakeEffect", "ChromaticAberrationEffect", "ClipEffectCollection", "Color", "ColorAdjustEffect",
    "ContrastEffect", "Crop", "CropKeyframe", "CropTrack", "CubicBezier", "DirectionalBlurEffect",
    "DurationMode", "Effect", "Flash", "FlashCollection", "TransitionDefinition", "TransitionPlacement",
    "GaussianBlurEffect", "GenericEffect", "GlowEffect", "ImageAsset", "ImageClip", "Interpolation", "JsonValue", "MotionBlurEffect",
    "AudioEffect", "AudioEffectCollection", "ParametricEqAudioEffect", "PlaybackSpeedAudioEffect", "available_audio_effects", "audio_effect_definition",
    "Point", "PointKeyframe", "PointTrack", "PostEffectCollection", "Preset", "PresetCollection", "ProjectBuilder",
    "Quality", "SaturationEffect", "ScalarKeyframe", "ScalarSignal", "ScalarTrack", "SharpenEffect", "ShapeClip", "Sizing", "SolidColorClip", "VideoAsset", "VideoClip",
    "Timeline", "TintEffect", "Transform", "TransitionCollection", "VignetteEffect", "ZoomBlurDirection", "Spectrum2DClip", "GroupClip",
    "ZoomBlurEffect", "available_effects", "effect_definition",
    "Spectrum2DPreset",
    "Spectrum2DGradient", "Spectrum2DLinearLayout", "Spectrum2DRadialLayout",
    "ParticleSystemClip", "ParticleSystem", "PointEmitter", "RectangleEmitter", "CircleEmitter",
    "ParticleBurst", "ScalarRange", "ParticlePrimitive", "ParticleBlendMode", "ParticleLifetimeStyle",
    "ScalarLifetimeStop", "ColourLifetimeStop", "ParticleAudioReactive", "ambient_stars", "snow",
    "embers", "sparks", "radial_burst",
}


def _builder() -> tuple[ProjectBuilder, AudioAsset, ImageClip, ImageClip]:
    builder = ProjectBuilder(
        width=32, height=24, frame_rate=FrameRate(20, 1), output_path="out.mp4",
        duration=2.5, base_directory=".", name="Phase 8", metadata={"release": 8},
    )
    image = builder.add_image_asset("tests/assets/wgpu-small-rgba.png")
    audio = builder.add_audio_asset("examples/assets/tone.wav")
    outgoing = builder.add_image_clip(source=image, start=0, duration=2, layer=0, sizing=Sizing.cover())
    incoming = builder.add_image_clip(source=image, start=1.5, duration=1, layer=1, sizing=Sizing.cover())
    return builder, audio, outgoing, incoming


def _complete() -> ProjectBuilder:
    builder, audio, outgoing, incoming = _builder()
    builder.add_solid_color_clip(colour="#102030", start=0, duration=2.5, layer=-1)
    outgoing.transform.position.keyframe(time=0.5, value=Point(0.45, 0.5))
    outgoing.opacity.keyframe(time=0.5, value=0.8)
    outgoing.set_crop(Crop(0, 0, 1, 1)).keyframe(time=0.5, value=Crop(0.1, 0, 0.9, 1))
    outgoing.effects.add_brightness(amount=0.1).amount.keyframe(time=0.5, value=0.2)
    outgoing.presets.apply_impact(seed=7, duration=0.4)
    builder.timeline.shift_clip(incoming, delta=0.25)
    builder.transitions.add_transition(outgoing=outgoing, incoming=incoming, definition=Crossfade().to_canonical(), start=0.25, duration=0.25)
    builder.flashes.add(start=1.8, duration=0.1, colour="#ffffff", opacity=0.4, layer=3)
    builder.post_effects.add_contrast(amount=1)
    builder.audio.add_track(id="music").add_clip(asset=audio, start=0, trim_end=0.2)
    return builder


def test_explicit_public_surface_excludes_private_implementation_names() -> None:
    assert set(api.__all__) == PUBLIC_NAMES
    assert all(not name.startswith("_") for name in api.__all__)
    assert not hasattr(api, "_Owner") and not hasattr(api, "_IdAllocator")


def test_constructor_policy_and_runtime_annotations_are_public_only() -> None:
    for factory in (ImageAsset, ImageClip, SolidColorClip, AudioTimeline, AudioTrack, AudioClip, Transform, ScalarTrack, PresetCollection, Timeline):
        with pytest.raises(TypeError):
            factory()
    for value in (api.Point(0, 0), api.Crop(0, 0, 1, 1), api.Color("#112233"), api.Sizing.cover(), api.CubicBezier(0, 0, 1, 1), api.Preset("zoom_punch")):
        assert value
    builder, audio, outgoing, incoming = _builder()
    callables = [
        ProjectBuilder.add_image_asset, ProjectBuilder.add_audio_asset, ProjectBuilder.add_image_clip,
        ProjectBuilder.add_solid_color_clip, builder.audio.add_track, AudioTrack.add_clip, outgoing.effects.add_brightness,
        builder.post_effects.add_contrast, builder.transitions.add_transition, builder.flashes.add,
        outgoing.presets.apply_impact, builder.timeline.shift_clip, builder.timeline.shift_clips,
        outgoing.opacity.keyframe,
    ]
    for callable_ in callables:
        assert typing.get_type_hints(callable_)
        signature = str(inspect.signature(typing.cast(Callable[..., object], callable_)))
        assert all(private not in signature for private in ("_Owner", "_IdAllocator", "_scope", "_create"))
    assert audio and incoming


@pytest.mark.parametrize("kind", ["static", "animated", "effects", "transition_flash", "preset", "audio", "complete"])
def test_authoring_round_trips_through_native_canonical_project(kind: str) -> None:
    builder, audio, outgoing, incoming = _builder()
    if kind in {"animated", "complete"}:
        outgoing.opacity.keyframe(time=0.5, value=0.5)
    if kind in {"effects", "complete"}:
        outgoing.effects.add_brightness(amount=0.1)
        builder.post_effects.add_contrast(amount=1)
    if kind in {"transition_flash", "complete"}:
        builder.timeline.shift_clip(incoming, delta=0.25)
        builder.transitions.add_transition(outgoing=outgoing, incoming=incoming, definition=Crossfade().to_canonical(), start=0.25, duration=0.25)
        builder.flashes.add(start=1.8, duration=0.1, colour="#ffffff", layer=3)
    if kind in {"preset", "complete"}:
        outgoing.presets.apply_focus_reveal(duration=0.4)
    if kind in {"audio", "complete"}:
        builder.audio.add_track(id="music").add_clip(asset=audio, start=0, trim_end=0.2)
    native = builder.build()
    round_trip = ProjectSnapshot.from_dict(native.to_dict(), base_directory=native.base_directory)
    assert native.to_dict() == round_trip.to_dict()


def test_snapshots_and_dependent_structures_are_not_hiddenly_repaired() -> None:
    builder = _complete()
    old_dict = deepcopy(builder.to_dict())
    native = builder.build()
    old_native = native.to_dict()
    transition = builder.transitions.items[0]
    outgoing, incoming = transition.outgoing, transition.incoming
    old_transitions = builder.transitions.items
    outgoing.duration = 1
    builder.timeline.shift_clip(incoming, delta=0.1)
    outgoing.presets.clear()
    assert old_dict != old_native  # Native serde expands schema defaults.
    assert native.to_dict() == old_native
    assert builder.transitions.items == old_transitions
    diagnostics = builder.validate().diagnostics
    assert any(d.code == "MVP-TRANSITION-FIT" for d in diagnostics)


def test_transactions_ids_and_independent_builder_determinism() -> None:
    builder, audio, outgoing, incoming = _builder()
    audio_track = builder.audio.add_track()
    before = builder.to_dict()
    with pytest.raises(ValueError):
        builder.add_image_asset("", id="never")
    with pytest.raises(ValueError):
        builder.timeline.shift_clips([outgoing, incoming], delta=-2)
    with pytest.raises(ValueError):
        builder.transitions.add_transition(outgoing=outgoing, incoming=incoming, definition=Crossfade().to_canonical(), start=0, duration=0)
    with pytest.raises(TypeError):
        outgoing.presets.apply_impact(seed=True)
    with pytest.raises(ValueError):
        audio_track.add_clip(asset=audio, start=0, trim_start=1, trim_end=1)
    with pytest.raises(TypeError):
        outgoing.effects.add_brightness(amount=True)
    with pytest.raises(TypeError):
        outgoing.effects.add_zoom_blur(radius=1, samples=True, anchor=Point(0.5, 0.5))
    with pytest.raises(ValueError):
        builder.flashes.add(start=0, duration=0, colour="#ffffff")
    with pytest.raises(api.AuthoringError):
        builder.transitions.add_transition(outgoing=outgoing, incoming=outgoing, definition=Crossfade().to_canonical(), start=0, duration=0.1)
    assert builder.to_dict() == before
    assert builder.add_image_asset("other.png").id == "image-000002"
    local = outgoing.effects.add_brightness(amount=0, id="shared")
    global_ = builder.post_effects.add_contrast(amount=1, id="shared")
    assert local.id == global_.id == "shared"
    left, right = _complete(), _complete()
    assert left.to_dict() == right.to_dict()
    assert left.build().to_dict() == right.build().to_dict()
    assert audio
