from pathlib import Path
import subprocess
import sys
from typing import TYPE_CHECKING
import vestra
from vestra import AdapterDeviceType, Editor, Frame, PrepareOptions, Project, ProjectSnapshot, Source, Transform, VideoEditorError
from vestra.authoring import (
    AudioAsset, AudioFadeCurve, AudioGainInterpolation, AudioGainKeyframe, Crop, CropKeyframe, ImageAsset, ImageClip, Interpolation, Point, PointKeyframe,
    Preset, ProjectBuilder, ScalarKeyframe, Sizing, SolidColorClip,
)
from vestra.audio import AudioClip, AudioTimeline, AudioTrack, PlaybackSpeed

if TYPE_CHECKING:
    high_level_project = Project(size=(2, 2), fps=(30, 1), duration=1)
    placed_source: Source = vestra.sources.Color("#112233")
    placed_layer = high_level_project.root.add(placed_source, z=1)
    placed_layer.opacity = 0.75
    placed_layer.transform.position = (0.25, 0.75)
    placed_layer.transform.anchor = (0.5, 0.5)
    placed_layer.transform.scale = 1.25
    placed_layer.transform.rotation = 15.0
    replacement_transform: Transform = Transform()
    placed_layer.transform = replacement_transform
    nested: vestra.CompositionLayer = high_level_project.root.group("nested")
    nested.add(vestra.sources.Color("#334455"))
    assert placed_layer.composition is high_level_project.root
    project: ProjectSnapshot = ProjectSnapshot.from_dict({"schema_version": 3, "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": False, "duration_mode": "automatic"}, "assets": [], "visual": {"clips": []}})
    report = Editor().validate(project)
    path: Path = project.base_directory
    assert report.is_valid
    assert path
    error: VideoEditorError
    kind: str = error.kind
    diagnostics = error.diagnostics
    warnings = error.warnings
    assert kind and diagnostics == warnings
    options = PrepareOptions(backend=vestra.BackendPreference.CPU)
    prepared = Editor().prepare(project, options)
    token = vestra.CancellationToken()
    request = vestra.PreparedVideoRenderRequest("out.mp4")

    def on_progress(event: vestra.RenderEvent) -> object:
        print(event.progress)
        return None

    rendered: vestra.RenderResult = prepared.render_video(
        request, progress=on_progress, cancellation=token
    )
    preparation_report = prepared.preparation_report
    frame: Frame = prepared.render_frame_number(0)
    pixels: bytes = frame.to_bytes()
    device: AdapterDeviceType | None = (
        preparation_report.adapter.device_type
        if preparation_report.adapter is not None
        else None
    )
    assert pixels or device or rendered.output_path
    builder = ProjectBuilder(
        width=160, height=90, frame_rate=vestra.FrameRate(30, 1),
        output_path="out.mp4", duration=1.0,
    )
    authored: dict[str, object] = builder.to_dict()
    authored_project: ProjectSnapshot = builder.build()
    authored_report: vestra.ValidationReport = builder.validate()
    point = Point(1.0, 1.0)
    sizing_original = Sizing.original()
    sizing_scale = Sizing.scale(1.25)
    sizing_stretch = Sizing.stretch(width=1280, height=720)
    image: ImageAsset = builder.add_image_asset("cover.png")
    audio: AudioAsset = builder.add_audio_asset("music.wav")
    image_clip: ImageClip = builder.add_image_clip(source=image, start=0.0, duration=1.0, layer=0)
    solid_clip: SolidColorClip = builder.add_solid_color_clip(colour="#112233", start=0.0, duration=1.0, layer=1)
    track = builder.audio.add_track(id="music")
    audio_clip = track.add_clip(asset=audio, start=0.0)
    audio_clip.set_gain_automation([
        AudioGainKeyframe(time=0.0, gain=0.0, interpolation=AudioGainInterpolation.LINEAR),
        AudioGainKeyframe(time=0.25, gain=1.0, interpolation=AudioGainInterpolation.HOLD),
    ])
    incoming_audio_clip = track.add_clip(asset=audio, start=0.5, trim_end=1.0)
    builder.audio.crossfade(audio_clip, incoming_audio_clip, curve=AudioFadeCurve.EQUAL_POWER)
    scalar_keyframe: ScalarKeyframe = image_clip.opacity.keyframe(
        time=0.0, value=0.0, interpolation=Interpolation.EASE_OUT,
    )
    point_keyframe: PointKeyframe = image_clip.transform.scale.keyframe(time=1.0, value=Point(1.2, 1.2))
    crop_keyframe: CropKeyframe = image_clip.crop.keyframe(time=0.5, value=Crop(0, 0, 1, 1))
    signal = builder.audio.master.rms()
    image_clip.opacity.modulate(signal)
    image_clip.transform.position_x.modulate(signal)
    image_clip.transform.scale.react_to(signal)
    image_clip.effects.add_vignette(
        amount=0,
        radius=1,
        softness=0.5,
        colour="#000000",
    ).amount.modulate(signal)
    assert authored and authored_project and authored_report and point and sizing_original and sizing_scale and sizing_stretch and image_clip and solid_clip
    assert scalar_keyframe and point_keyframe and crop_keyframe
    preset: Preset = image_clip.presets.apply_impact(seed=7, intensity=1.0)
    builder.timeline.shift_clip(image_clip, delta=0.0)
    builder.timeline.shift_clips([image_clip], delta=0.0)
    assert preset and audio_clip and incoming_audio_clip
    audio_timeline: AudioTimeline = high_level_project.audio
    audio_track: AudioTrack = audio_timeline.track("music")
    high_level_audio_clip: AudioClip = audio_track.add("tone.wav", start=0, trim_end=1)
    high_level_audio_clip.effects.add(PlaybackSpeed(1.25))
    assert audio_track.project is high_level_project


def test_negative_immutability_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/immutable_assignment.py"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("read-only") == 15


def test_negative_callback_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/non_callable_progress.py"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("incompatible type") == 2


def test_negative_authoring_immutability_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_immutable_assignment.py"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    assert "read-only" in result.stdout


def test_negative_authoring_track_replacement_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_track_replacement.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("read-only") == 4


def test_negative_high_level_property_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/high_level_invalid_properties.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("Incompatible types") == 4


def test_negative_high_level_audio_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/high_level_invalid_audio.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1


def test_negative_high_level_signal_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/high_level_invalid_signals.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("error:") >= 3


def test_positive_high_level_effect_fixture_typechecks() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_effects.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr


def test_negative_high_level_effect_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/high_level_invalid_effects.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("error:") >= 2


def test_positive_high_level_timeline_fixture_typechecks() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_timeline_features.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr


def test_negative_high_level_timeline_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/high_level_invalid_timeline_features.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("error:") >= 4


def test_negative_authoring_sizing_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_invalid_sizing.py"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    # Python's type system treats bool as an int subtype. Runtime validation
    # rejects those two calls; mypy still catches the string dimension.
    assert result.stdout.count("Argument") == 1


def test_negative_authoring_asset_kind_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_wrong_asset_kind.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("Argument") == 2


def test_negative_keyframe_value_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_wrong_keyframe_value.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("Argument") == 3


def test_clip_only_post_effect_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_invalid_post_effect.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert "add_camera_shake" in result.stdout


def test_invalid_effect_property_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_invalid_effect_properties.py"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("has no attribute") == 4
    assert "samples" in result.stdout


def test_invalid_signal_modulation_surface_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_invalid_signal_modulation.py"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("has no attribute") == 5
    assert "ScalarModifierTarget" in result.stdout
    assert "ScalarTrack" in result.stdout
    assert "keyframe" in result.stdout
    assert "base_value" in result.stdout
    assert "modulate" in result.stdout
    assert "react_to" in result.stdout


def test_invalid_transition_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_invalid_transition.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert "Argument" in result.stdout
    assert result.stdout.count("read-only") == 4


def test_invalid_audio_automation_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_invalid_audio_automation.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("error:") == 4
