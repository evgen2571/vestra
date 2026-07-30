import os
from pathlib import Path

import pytest

import video_editor


FIXTURE = Path("tests/fixtures/wgpu-small-rgba.json")
UNAVAILABLE = {"WGPU-ADAPTER-NOT-FOUND", "WGPU-NO-COMPATIBLE-ADAPTER"}


def wgpu_prepared() -> video_editor.PreparedProject:
    project = video_editor.Project.load(FIXTURE)
    try:
        return video_editor.Editor().prepare(
            project,
            video_editor.PrepareOptions(backend=video_editor.BackendPreference.WGPU),
        )
    except video_editor.PreparationError as error:
        if os.environ.get("VIDEO_EDITOR_REQUIRE_WGPU") == "1" or any(
            diagnostic.code not in UNAVAILABLE for diagnostic in error.diagnostics
        ):
            raise
        pytest.skip("no compatible WGPU adapter")


def test_prepared_wgpu_video_render_returns_adapter_owned_result(tmp_path: Path) -> None:
    prepared = wgpu_prepared()
    output = tmp_path / "prepared-wgpu.mp4"
    events: list[video_editor.RenderEvent] = []
    result = prepared.render_video(
        video_editor.PreparedVideoRenderRequest(output), progress=events.append
    )
    assert output.exists() and output.stat().st_size > 0
    assert result.selected_backend == "wgpu"
    assert result.adapter is not None
    assert result.performance.rendered_frame_count == result.total_frames
    assert [event.kind for event in events] == ["started"]


def test_one_shot_wgpu_video_render_preserves_backend_metadata(tmp_path: Path) -> None:
    # Preparation is deliberately attempted first to apply the shared strict/skip policy.
    wgpu_prepared()
    output = tmp_path / "one-shot-wgpu.mp4"
    result = video_editor.Editor().render(
        video_editor.Project.load(FIXTURE),
        video_editor.RenderRequest(output, backend=video_editor.BackendPreference.WGPU),
    )
    assert output.exists() and output.stat().st_size > 0
    assert result.timing_scope is video_editor.RenderTimingScope.ONE_SHOT
    assert result.selected_backend == "wgpu"
    assert result.adapter is not None
