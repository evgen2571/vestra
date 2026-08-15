from pathlib import Path

import pytest

from vestra import (
    BackendPreference,
    CancellationToken,
    CancelledError,
    Editor,
    FrameRate,
    PrepareOptions,
    PreparedVideoRenderRequest,
    ProjectSnapshot,
    ProjectError,
    RenderRequest,
)
from vestra.authoring import (
    AuthoringError,
    ProjectBuilder,
    audio_effect_definition,
    available_audio_effects,
)


def builder(*, output_audio: bool = False) -> ProjectBuilder:
    return ProjectBuilder(width=16, height=16, frame_rate=FrameRate(30, 1), output_path="out.mp4", duration=1, output_audio=output_audio, base_directory=Path.cwd())


def test_audio_timeline_is_ordered_and_round_trips() -> None:
    project_builder = builder()
    a = project_builder.add_audio_asset("examples/assets/tone.wav")
    music = project_builder.audio.add_track(id="music", gain=1.5)
    ambience = project_builder.audio.add_track(id="ambience", mute=True)
    first = music.add_clip(asset=a, start=0, trim_end=0.5)
    second = music.add_clip(asset=a, start=0.25, trim_end=0.5)
    assert [track.id for track in project_builder.audio.tracks] == ["music", "ambience"]
    assert [clip.id for clip in music.clips] == [first.id, second.id]
    data = project_builder.to_dict()
    assert data["schema_version"] == 3
    assert data["output"]["audio"] is False  # type: ignore[index]
    assert data["audio"]["tracks"][0]["clips"][1]["start"] == 0.25  # type: ignore[index]
    project = project_builder.build()
    assert project.to_dict() == data
    report = Editor().validate(project)
    assert report.is_valid
    inspection = Editor().inspect(project)
    assert inspection.audio is not None
    assert inspection.audio.tracks[0].id == "music"
    assert [clip.id for clip in inspection.audio.tracks[0].clips] == [first.id, second.id]


def test_audio_inspection_preserves_declaration_order_when_ids_are_unsorted() -> None:
    project_builder = builder()
    asset = project_builder.add_audio_asset("examples/assets/tone.wav")
    zulu = project_builder.audio.add_track(id="zulu")
    alpha = project_builder.audio.add_track(id="alpha")
    zulu.add_clip(asset=asset, start=0, trim_end=0.5, id="z-clip")
    zulu.add_clip(asset=asset, start=0.1, trim_end=0.5, id="a-clip")
    alpha.add_clip(asset=asset, start=0, trim_end=0.5, id="b-clip")
    inspection = Editor().inspect(project_builder.build())
    assert inspection.audio is not None
    assert [track.id for track in inspection.audio.tracks] == ["zulu", "alpha"]
    assert [clip.id for clip in inspection.audio.tracks[0].clips] == ["z-clip", "a-clip"]


def test_audio_ids_and_ownership_are_global_and_transactional() -> None:
    project_builder = builder()
    asset = project_builder.add_audio_asset("tone.wav")
    first = project_builder.audio.add_track()
    with pytest.raises(ValueError):
        project_builder.audio.add_track(gain=-1)
    assert project_builder.audio.add_track().id == "audio-track-000002"
    first.add_clip(asset=asset, start=0, id="clip")
    second = project_builder.audio.tracks[1]
    with pytest.raises(AuthoringError):
        second.add_clip(asset=asset, start=0, id="clip")
    foreign = builder().add_audio_asset("other.wav")
    with pytest.raises(AuthoringError):
        second.add_clip(asset=foreign, start=0)
    assert not second.clips


def test_output_audio_is_independent_of_authored_audio() -> None:
    project_builder = builder(output_audio=False)
    asset = project_builder.add_audio_asset("tone.wav")
    track = project_builder.audio.add_track()
    track.add_clip(asset=asset, start=0)
    assert project_builder.output_audio is False
    project_builder.output_audio = True
    assert project_builder.to_dict()["output"]["audio"] is True  # type: ignore[index]


def test_schema_v1_and_old_global_audio_shape_are_rejected() -> None:
    data = builder().to_dict()
    data["schema_version"] = 1
    with pytest.raises(ProjectError):
        ProjectSnapshot.from_dict(data)


def test_audio_effects_are_typed_at_clip_track_and_master_scopes() -> None:
    project_builder = builder()
    asset = project_builder.add_audio_asset("tone.wav")
    track = project_builder.audio.add_track(id="music")
    clip = track.add_clip(asset=asset, start=0, trim_end=0.5)
    clip.effects.add_parametric_eq(frequency_hz=120, gain_db=6, q=0.8)
    track.effects.add_parametric_eq(frequency_hz=240, gain_db=-3, q=1)
    project_builder.audio.effects.add_parametric_eq(frequency_hz=480, gain_db=0, q=2)
    data = project_builder.to_dict()
    assert len(data["audio"]["effects"]) == 1  # type: ignore[index]
    assert len(data["audio"]["tracks"][0]["effects"]) == 1  # type: ignore[index]
    assert len(data["audio"]["tracks"][0]["clips"][0]["effects"]) == 1  # type: ignore[index]


def test_typed_and_generic_audio_effect_authoring_have_same_canonical_shape() -> None:
    typed = builder()
    typed_asset = typed.add_audio_asset("tone.wav")
    typed_clip = typed.audio.add_track(id="music").add_clip(asset=typed_asset, start=0, trim_end=0.5)
    typed_clip.effects.add_parametric_eq(frequency_hz=120, gain_db=6, q=0.8)

    generic = builder()
    generic_asset = generic.add_audio_asset("tone.wav")
    generic_clip = generic.audio.add_track(id="music").add_clip(asset=generic_asset, start=0, trim_end=0.5)
    generic_clip.effects.add_effect("parametric_eq", frequency_hz=120, gain_db=6, q=0.8)
    assert typed.to_dict() == generic.to_dict()


def test_playback_speed_typed_and_generic_authoring_are_clip_only() -> None:
    typed = builder()
    typed_asset = typed.add_audio_asset("tone.wav")
    typed_clip = typed.audio.add_track(id="music").add_clip(asset=typed_asset, start=0, trim_end=0.5)
    typed_effect = typed_clip.effects.add_playback_speed(rate=2.0)

    generic = builder()
    generic_asset = generic.add_audio_asset("tone.wav")
    generic_clip = generic.audio.add_track(id="music").add_clip(asset=generic_asset, start=0, trim_end=0.5)
    generic_effect = generic_clip.effects.add_effect("playback_speed", rate=2.0)
    assert typed.to_dict() == generic.to_dict()
    assert typed_effect.__class__.__name__ == "PlaybackSpeedAudioEffect"
    assert generic_effect.__class__.__name__ == "AudioEffect"

    track = generic.audio.tracks[0]
    with pytest.raises(ValueError, match="not valid at this scope"):
        track.effects.add_playback_speed(rate=2.0)
    with pytest.raises(ValueError, match="not valid at this scope"):
        generic.audio.effects.add_effect("playback_speed", rate=2.0)


def test_audio_effect_metadata_is_discoverable_and_immutable() -> None:
    definitions = available_audio_effects()
    assert [definition["id"] for definition in definitions] == ["parametric_eq", "bass_boost", "playback_speed"]
    definition = audio_effect_definition("parametric_eq")
    assert set(definition["scopes"]) == {"clip", "track", "master"}
    assert definition["duration_behavior"] == "preserve"
    with pytest.raises(TypeError):
        definition["id"] = "changed"  # type: ignore[index]
    with pytest.raises(TypeError):
        definition["parameters"][0]["name"] = "changed"  # type: ignore[index]
    assert audio_effect_definition("parametric_eq")["id"] == "parametric_eq"
    speed = audio_effect_definition("playback_speed")
    assert speed["scopes"] == ("clip",)
    assert speed["duration_behavior"] == "transform"
    assert speed["parameters"][0]["minimum"] == 0.25
    assert speed["parameters"][0]["maximum"] == 4.0
    bass = audio_effect_definition("bass_boost")
    assert bass["scopes"] == ("clip", "track", "master")
    assert bass["duration_behavior"] == "preserve"
    assert [parameter["default"] for parameter in bass["parameters"]] == [6.0, 100.0]


def test_bass_boost_typed_and_generic_defaults_share_canonicalization() -> None:
    project_builder = builder()
    asset = project_builder.add_audio_asset("tone.wav")
    clip = project_builder.audio.add_track().add_clip(asset=asset, start=0)
    typed = clip.effects.add_bass_boost()
    generic = clip.effects.add_effect("bass_boost")
    typed_data = typed.to_canonical()
    generic_data = generic.to_canonical()
    typed_data.pop("id")
    generic_data.pop("id")
    assert typed_data == generic_data
    assert typed.to_canonical()["gain_db"] == 6.0
    assert typed.to_canonical()["frequency_hz"] == 100.0


def test_generic_audio_authoring_smoke_covers_every_registered_effect() -> None:
    project_builder = builder()
    asset = project_builder.add_audio_asset("tone.wav")
    clip = project_builder.audio.add_track().add_clip(asset=asset, start=0, trim_end=0.5)
    for definition in available_audio_effects():
        parameters = {}
        for parameter in definition["parameters"]:
            minimum, maximum = parameter["minimum"], parameter["maximum"]
            parameters[str(parameter["name"])] = (
                (float(minimum) + float(maximum)) / 2
                if minimum is not None and maximum is not None
                else float(minimum) + 1.0
            )
        clip.effects.add_effect(str(definition["id"]), **parameters)
    assert len(clip.effects.items) == len(available_audio_effects())


@pytest.mark.parametrize("parameters", [
    {"frequency_hz": 0, "gain_db": 0, "q": 1},
    {"frequency_hz": 120, "gain_db": 25, "q": 1},
    {"frequency_hz": 120, "gain_db": 0, "q": 0},
])
def test_audio_effect_authoring_rejects_unknown_missing_and_invalid_parameters(parameters: dict[str, float]) -> None:
    project_builder = builder()
    asset = project_builder.add_audio_asset("tone.wav")
    clip = project_builder.audio.add_track().add_clip(asset=asset, start=0)
    with pytest.raises((TypeError, ValueError)):
        clip.effects.add_effect("parametric_eq", **parameters)
    with pytest.raises(ValueError):
        clip.effects.add_effect("does_not_exist", frequency_hz=120, gain_db=0, q=1)
    with pytest.raises(TypeError):
        clip.effects.add_effect("parametric_eq", frequency_hz=120, gain_db=0, q=1, nonsense=123)
    with pytest.raises(TypeError):
        clip.effects.add_effect("parametric_eq", frequency_hz=120, gain_db=0)


def test_multi_clip_audio_renders_and_preserves_legacy_shape_rejection(tmp_path: Path) -> None:
    project_builder = builder(output_audio=True)
    project_builder.base_directory = Path.cwd()
    project_builder.add_solid_color_clip(colour="#000000", start=0, duration=1, layer=0)
    asset = project_builder.add_audio_asset("examples/assets/tone.wav")
    track = project_builder.audio.add_track()
    track.add_clip(asset=asset, start=0, trim_end=0.2)
    track.add_clip(asset=asset, start=0.1, trim_end=0.2)
    output = tmp_path / "mixed.mp4"
    result = Editor().render(project_builder.build(), RenderRequest(output, backend=BackendPreference.CPU))
    assert output.exists()
    assert result.audio_present is True
    data = builder().to_dict()
    data["audio"] = {"asset": "tone", "timeline_start": 0, "trim_start": 0, "volume": 1}
    with pytest.raises(ProjectError):
        ProjectSnapshot.from_dict(data)


def test_prepared_multi_clip_audio_renders_twice(tmp_path: Path) -> None:
    project_builder = builder(output_audio=True)
    project_builder.base_directory = Path.cwd()
    project_builder.add_solid_color_clip(colour="#000000", start=0, duration=1, layer=0)
    asset = project_builder.add_audio_asset("examples/assets/tone.wav")
    music = project_builder.audio.add_track(id="music", gain=0.75)
    music.add_clip(asset=asset, start=0, trim_end=0.4)
    music.add_clip(asset=asset, start=0.12345, trim_end=0.5, gain=0.5)
    prepared = Editor().prepare(
        project_builder.build(),
        PrepareOptions(backend=BackendPreference.CPU),
    )
    first = prepared.render_video(PreparedVideoRenderRequest(tmp_path / "first.mp4"))
    second = prepared.render_video(PreparedVideoRenderRequest(tmp_path / "second.mp4"))
    assert first.audio_present is True
    assert second.audio_present is True
    assert (tmp_path / "first.mp4").exists()
    assert (tmp_path / "second.mp4").exists()


@pytest.mark.parametrize(
    ("output_audio", "track_mute", "clip_gain"),
    [(False, False, 1.0), (True, True, 1.0), (True, False, 0.0)],
)
def test_disabled_or_inaudible_multi_track_audio_produces_video_only(
    tmp_path: Path, output_audio: bool, track_mute: bool, clip_gain: float
) -> None:
    project_builder = builder(output_audio=output_audio)
    project_builder.base_directory = Path.cwd()
    project_builder.add_solid_color_clip(colour="#000000", start=0, duration=1, layer=0)
    asset = project_builder.add_audio_asset("examples/assets/tone.wav")
    music = project_builder.audio.add_track(id="music", mute=track_mute)
    ambience = project_builder.audio.add_track(id="ambience", gain=0.0)
    music.add_clip(asset=asset, start=0, trim_end=0.2, gain=clip_gain)
    ambience.add_clip(asset=asset, start=0.12345, trim_end=0.2)
    result = Editor().render(
        project_builder.build(),
        RenderRequest(tmp_path / "video-only.mp4", backend=BackendPreference.CPU),
    )
    assert result.audio_present is False


def test_multi_input_render_cancellation_removes_temporary_output(tmp_path: Path) -> None:
    project_builder = ProjectBuilder(
        width=16,
        height=16,
        frame_rate=FrameRate(60, 1),
        output_path="out.mp4",
        duration=2,
        output_audio=True,
        base_directory=Path.cwd(),
    )
    project_builder.add_solid_color_clip(colour="#000000", start=0, duration=2, layer=0)
    asset = project_builder.add_audio_asset("examples/assets/tone.wav")
    first = project_builder.audio.add_track(id="first")
    second = project_builder.audio.add_track(id="second")
    first.add_clip(asset=asset, start=0, trim_end=0.8)
    second.add_clip(asset=asset, start=0.12345, trim_end=0.8)
    token = CancellationToken()

    def cancel_on_progress(event: object) -> None:
        if getattr(event, "kind") == "progress":
            token.cancel()

    output = tmp_path / "cancelled.mp4"
    with pytest.raises(CancelledError) as raised:
        Editor().render(
            project_builder.build(),
            RenderRequest(output, backend=BackendPreference.CPU),
            progress=cancel_on_progress,
            cancellation=token,
        )
    assert raised.value.temporary_removed is True
    assert token.is_cancelled
    assert not output.exists()
    assert list(tmp_path.iterdir()) == []
