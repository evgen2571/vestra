from pathlib import Path

import pytest
from video_editor import (
    BackendPreference, CancellationToken, CancelledError, Editor, FrameRate, PrepareOptions,
    PreparedVideoRenderRequest, Project, RenderRequest,
)

from video_editor.authoring import (
    AudioFadeCurve,
    AudioClip,
    AudioGainInterpolation,
    AudioGainKeyframe,
    AuthoringError,
    ProjectBuilder,
)


def builder(*, output_audio: bool = False) -> ProjectBuilder:
    return ProjectBuilder(width=16, height=16, frame_rate=FrameRate(30, 1), output_path="out.mp4", duration=2, output_audio=output_audio, base_directory=Path.cwd())


def clips() -> tuple[ProjectBuilder, AudioClip, AudioClip]:
    project = builder()
    asset = project.add_audio_asset("examples/assets/tone.wav")
    track = project.audio.add_track()
    return project, track.add_clip(asset=asset, start=0, trim_end=1), track.add_clip(asset=asset, start=0.5, trim_end=1.5)


def test_gain_automation_serializes_immutable_snapshot() -> None:
    _, clip, _ = clips()
    points = [AudioGainKeyframe(0, 0.25, AudioGainInterpolation.HOLD), AudioGainKeyframe(0.12345, 1)]
    clip.set_gain_automation(points)
    points.clear()
    assert [point.gain for point in clip.gain_automation] == [0.25, 1.0]
    assert clip.to_canonical()["gain_automation"] == {"keyframes": [{"time": 0.0, "gain": 0.25, "interpolation": "hold"}, {"time": 0.12345, "gain": 1.0, "interpolation": "linear"}]}


def test_gain_automation_failure_is_transactional() -> None:
    _, clip, _ = clips()
    clip.set_gain_automation([AudioGainKeyframe(0, 1)])
    with pytest.raises(AuthoringError):
        clip.set_gain_automation([AudioGainKeyframe(0.1, 1)])
    assert clip.gain_automation == (AudioGainKeyframe(0, 1),)


def test_equal_power_crossfade_uses_existing_overlap_without_moving_or_trimming() -> None:
    project, outgoing, incoming = clips()
    before = (outgoing.start, outgoing.trim_start, outgoing.trim_end, incoming.start, incoming.trim_start, incoming.trim_end)
    project.audio.crossfade(outgoing, incoming)
    assert outgoing.fade_out == incoming.fade_in == 0.5
    assert outgoing.fade_out_curve is AudioFadeCurve.EQUAL_POWER
    assert before == (outgoing.start, outgoing.trim_start, outgoing.trim_end, incoming.start, incoming.trim_start, incoming.trim_end)


def test_crossfade_failure_is_transactional() -> None:
    project, outgoing, incoming = clips()
    incoming._fade_in = 0.2  # type: ignore[attr-defined]
    with pytest.raises(AuthoringError):
        project.audio.crossfade(outgoing, incoming, curve=AudioFadeCurve.EQUAL_POWER)
    assert outgoing.fade_out == 0
    assert incoming.fade_in == 0.2


def test_crossfade_does_not_overwrite_matching_duration_linear_fade() -> None:
    project = builder()
    asset = project.add_audio_asset("examples/assets/tone.wav")
    outgoing = project.audio.add_track().add_clip(asset=asset, start=0, trim_end=1, fade_out=0.5)
    incoming = project.audio.add_track().add_clip(asset=asset, start=0.5, trim_end=1.5)
    with pytest.raises(AuthoringError, match="conflicting"):
        project.audio.crossfade(outgoing, incoming, curve=AudioFadeCurve.EQUAL_POWER)
    assert outgoing.fade_out_curve is AudioFadeCurve.LINEAR
    assert incoming.fade_in == 0


def test_crossfade_rejects_foreign_and_unresolved_clips_without_mutation() -> None:
    project, outgoing, incoming = clips()
    other, foreign, _ = clips()
    with pytest.raises(AuthoringError):
        project.audio.crossfade(outgoing, foreign)
    assert outgoing.fade_out == incoming.fade_in == 0
    asset = other.add_audio_asset("examples/assets/tone.wav")
    unresolved = other.audio.add_track().add_clip(asset=asset, start=0)
    later = other.audio.add_track().add_clip(asset=asset, start=0.5, trim_end=1.0)
    with pytest.raises(AuthoringError, match="explicit trim_end"):
        other.audio.crossfade(unresolved, later)
    assert unresolved.fade_out == later.fade_in == 0


def test_automation_round_trips_and_inspection_exposes_canonical_details() -> None:
    project, outgoing, incoming = clips()
    outgoing.set_gain_automation([
        AudioGainKeyframe(0, 0.5, AudioGainInterpolation.HOLD),
        AudioGainKeyframe(0.12345, 1.25),
    ])
    project.audio.crossfade(outgoing, incoming, curve=AudioFadeCurve.EQUAL_POWER)
    authored = project.to_dict()
    native = Project.from_dict(authored)
    assert native.to_dict() == authored
    inspection = Editor().inspect(native)
    clip = inspection.audio.tracks[0].clips[0]  # type: ignore[union-attr]
    assert [(point.time, point.gain, point.interpolation) for point in clip.gain_automation] == [
        (0.0, 0.5, "hold"), (0.12345, 1.25, "linear"),
    ]
    assert clip.fade_out_curve == "equal_power"


def test_public_automation_and_equal_power_crossfade_render_aac(tmp_path: Path) -> None:
    project = builder(output_audio=True)
    project.add_solid_color_clip(colour="#000000", start=0, duration=2, layer=0)
    asset = project.add_audio_asset("examples/assets/tone.wav")
    outgoing = project.audio.add_track().add_clip(asset=asset, start=0, trim_end=1.0)
    incoming = project.audio.add_track().add_clip(asset=asset, start=0.5, trim_end=1.5)
    outgoing.set_gain_automation([
        AudioGainKeyframe(0, 0, AudioGainInterpolation.LINEAR),
        AudioGainKeyframe(0.25, 1),
    ])
    project.audio.crossfade(outgoing, incoming, curve=AudioFadeCurve.EQUAL_POWER)
    result = Editor().render(
        project.build(),
        RenderRequest(tmp_path / "phase9c.mp4", backend=BackendPreference.CPU),
    )
    assert result.audio_present


def test_prepared_automation_and_crossfade_render_twice(tmp_path: Path) -> None:
    project = builder(output_audio=True)
    project.add_solid_color_clip(colour="#000000", start=0, duration=2, layer=0)
    asset = project.add_audio_asset("examples/assets/tone.wav")
    outgoing = project.audio.add_track().add_clip(asset=asset, start=0, trim_end=1.0)
    incoming = project.audio.add_track().add_clip(asset=asset, start=0.5, trim_end=1.5)
    outgoing.set_gain_automation([AudioGainKeyframe(0, 1), AudioGainKeyframe(0.12345, 0.5)])
    project.audio.crossfade(outgoing, incoming)
    prepared = Editor().prepare(project.build(), PrepareOptions(backend=BackendPreference.CPU))
    first = prepared.render_video(PreparedVideoRenderRequest(tmp_path / "first.mp4"))
    second = prepared.render_video(PreparedVideoRenderRequest(tmp_path / "second.mp4"))
    assert first.audio_present and second.audio_present


def test_cancellation_cleans_automated_crossfade_output(tmp_path: Path) -> None:
    project = builder(output_audio=True)
    project.add_solid_color_clip(colour="#000000", start=0, duration=2, layer=0)
    asset = project.add_audio_asset("examples/assets/tone.wav")
    outgoing = project.audio.add_track().add_clip(asset=asset, start=0, trim_end=1.0)
    incoming = project.audio.add_track().add_clip(asset=asset, start=0.5, trim_end=1.5)
    outgoing.set_gain_automation([AudioGainKeyframe(0, 0), AudioGainKeyframe(0.12345, 1)])
    project.audio.crossfade(outgoing, incoming)
    token = CancellationToken()
    output = tmp_path / "cancelled.mp4"

    def cancel_on_progress(event: object) -> None:
        if getattr(event, "kind") == "progress":
            token.cancel()

    with pytest.raises(CancelledError):
        Editor().render(
            project.build(), RenderRequest(output, backend=BackendPreference.CPU),
            progress=cancel_on_progress, cancellation=token,
        )
    assert not output.exists()
