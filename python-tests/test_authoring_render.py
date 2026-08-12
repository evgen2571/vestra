from pathlib import Path
import subprocess

import pytest

import video_editor
from video_editor import FrameRate
from video_editor.authoring import ProjectBuilder


def test_background_only_builder_renders_on_cpu(tmp_path: Path) -> None:
    project = ProjectBuilder(
        width=160,
        height=90,
        frame_rate=FrameRate(10, 1),
        output_path="canonical-output.mp4",
        duration=0.2,
        background="#ff0000",
        base_directory=tmp_path,
    ).build()
    output = tmp_path / "result.mp4"
    result = video_editor.Editor().render(
        project,
        video_editor.RenderRequest(
            output, backend=video_editor.BackendPreference.CPU, overwrite=True,
        ),
    )
    assert result.output_path == output
    assert result.width == 160
    assert result.height == 90
    assert result.duration_seconds == 0.2
    assert result.total_frames == 2
    assert output.is_file() and output.stat().st_size > 0
    assert not list(tmp_path.glob("*.tmp"))
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
         "stream=width,height,nb_frames,duration", "-of", "default=noprint_wrappers=1", str(output)],
        check=True, capture_output=True, text=True,
    )
    assert "width=160" in probe.stdout
    assert "height=90" in probe.stdout
    assert "nb_frames=2" in probe.stdout
    assert "duration=0.200000" in probe.stdout


@pytest.mark.parametrize("preset", ["classic", "dense", "neon"])
def test_public_spectrum2d_preset_authoring_renders_on_cpu(tmp_path: Path, preset: str) -> None:
    project_builder = ProjectBuilder(
        width=64, height=64, frame_rate=FrameRate(10, 1), output_path="spectrum.mp4",
        duration=0.2, base_directory=Path(__file__).resolve().parents[1],
    )
    audio = project_builder.add_audio_asset("examples/assets/tone.wav")
    track = project_builder.audio.add_track(id="music")
    track.add_clip(asset=audio, start=0, trim_end=0.2)
    clip = project_builder.add_spectrum2d_clip(start=0, duration=0.2, layer=1, preset=preset)  # type: ignore[arg-type]
    if preset == "neon":
        assert [effect.kind for effect in clip.effects.items] == ["glow", "bloom"]
    project = project_builder.build()
    output = tmp_path / "spectrum.mp4"
    result = video_editor.Editor().render(
        project, video_editor.RenderRequest(
            output, backend=video_editor.BackendPreference.CPU, overwrite=True,
        ),
    )
    assert result.output_path == output
    assert result.width == 64 and result.height == 64 and result.total_frames == 2
    assert output.is_file() and output.stat().st_size > 0
