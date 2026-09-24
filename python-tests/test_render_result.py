from pathlib import Path
import subprocess

import pytest

import vestra


FIXTURE = Path("tests/fixtures/wgpu-small-rgba.json")


def cpu_request(output: Path, *, preview: bool = False) -> vestra.RenderRequest:
    return vestra.RenderRequest(
        output, backend=vestra.BackendPreference.CPU, preview=preview
    )


def audio_project() -> vestra.ProjectSnapshot:
    return vestra.ProjectSnapshot.from_dict(
        {
            "schema_version": 1,
            "output": {
                "path": "unused.mp4", "width": 2, "height": 2, "frame_rate": "1/1",
                "background": "#102030", "quality": "preview", "audio": True,
                "duration_mode": "explicit", "duration": 1,
            },
            "assets": [{"id": "tone", "type": "audio", "source": "assets/tone.wav"}],
            "visual": {"clips": []},
            "audio": {"tracks": [{
                "id": "tone", "gain": 1, "mute": False, "clips": [{
                    "id": "tone-clip", "asset": "tone", "start": 0,
                    "trim_start": 0, "trim_end": 1, "gain": 1,
                    "fade_in": 0, "fade_out": 0, "mute": False,
                }],
            }]},
        },
        base_directory=Path("examples"),
    )


def stream_types(path: Path) -> set[str]:
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "stream=codec_type", "-of", "csv=p=0", str(path)],
        check=True, capture_output=True, text=True,
    )
    return set(probe.stdout.split())


def assert_full_result(result: vestra.RenderResult, output: Path, *, scope: vestra.RenderTimingScope, preview: bool) -> None:
    assert isinstance(result.editor_version, str) and result.editor_version
    assert isinstance(result.project_path, Path)
    assert result.output_path == output
    assert result.width == 174
    assert result.height == 130
    assert result.frame_rate == "1/1"
    assert result.duration_seconds == 1.0
    assert result.total_frames == 1
    assert result.visual_clip_count == 1
    assert result.audio_present is False
    assert result.preview is preview
    assert result.elapsed_ms >= 0
    assert result.elapsed_seconds >= 0
    assert result.timing_scope is scope
    assert isinstance(result.timings, vestra.RenderTimings)
    assert isinstance(result.performance, vestra.RenderPerformance)
    assert result.requested_backend is vestra.BackendPreference.CPU
    assert result.selected_backend == "cpu"
    assert result.encoder_backend == "ffmpeg"
    assert result.fallback is None
    assert result.adapter is None
    assert result.warnings == ()
    with pytest.raises(AttributeError):
        result.output_path = output  # type: ignore[misc]
    with pytest.raises(AttributeError):
        result.timings.operation_total_ms = 1  # type: ignore[misc]


def test_prepared_and_one_shot_results_are_complete_immutable_snapshots(tmp_path: Path) -> None:
    project = vestra.ProjectSnapshot.load(FIXTURE)
    editor = vestra.Editor()
    prepared = editor.prepare(project, vestra.PrepareOptions(backend=vestra.BackendPreference.CPU))
    prepared_output = tmp_path / "prepared.mp4"
    prepared_result = prepared.render_video(vestra.PreparedVideoRenderRequest(prepared_output))
    assert_full_result(
        prepared_result, prepared_output,
        scope=vestra.RenderTimingScope.PREPARED_OPERATION, preview=False,
    )

    one_shot_output = tmp_path / "one-shot.mp4"
    one_shot_result = editor.render(project, cpu_request(one_shot_output))
    assert_full_result(
        one_shot_result, one_shot_output,
        scope=vestra.RenderTimingScope.ONE_SHOT, preview=False,
    )
    del editor, project, prepared
    prepared_output.unlink()
    assert prepared_result.output_path == prepared_output
    assert prepared_result.performance.rendered_frame_count == 1
    assert prepared_result.timings.operation_total_ms >= 0


def test_preview_and_audio_results_match_readable_streams(tmp_path: Path) -> None:
    preview_request = cpu_request(tmp_path / "preview.mp4", preview=True)
    assert preview_request.preview is True
    preview_result = vestra.Editor().render(vestra.ProjectSnapshot.load(FIXTURE), preview_request)
    assert preview_result.preview is True
    assert stream_types(preview_result.output_path) == {"video"}

    audio_output = tmp_path / "audio.mp4"
    audio_result = vestra.Editor().render(audio_project(), cpu_request(audio_output))
    assert audio_result.audio_present is True
    assert stream_types(audio_output) == {"audio", "video"}
