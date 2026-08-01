from pathlib import Path

import pytest

from video_editor import BackendPreference, Editor, FrameRate, Project, ProjectError, RenderError, RenderRequest
from video_editor.authoring import AuthoringError, ProjectBuilder


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
    assert data["schema_version"] == 2
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
        Project.from_dict(data)


def test_multi_clip_audio_fails_before_output_publication(tmp_path: Path) -> None:
    project_builder = builder(output_audio=True)
    project_builder.base_directory = Path.cwd()
    project_builder.add_solid_color_clip(colour="#000000", start=0, duration=1, layer=0)
    asset = project_builder.add_audio_asset("examples/assets/tone.wav")
    track = project_builder.audio.add_track()
    track.add_clip(asset=asset, start=0, trim_end=0.2)
    track.add_clip(asset=asset, start=0.1, trim_end=0.2)
    output = tmp_path / "unsupported.mp4"
    with pytest.raises(RenderError, match="more than one audible"):
        Editor().render(project_builder.build(), RenderRequest(output, backend=BackendPreference.CPU))
    assert not output.exists()
    data = builder().to_dict()
    data["audio"] = {"asset": "tone", "timeline_start": 0, "trim_start": 0, "volume": 1}
    with pytest.raises(ProjectError):
        Project.from_dict(data)
