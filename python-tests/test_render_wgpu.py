import os
from pathlib import Path

import pytest

import vestra


FIXTURE = Path("tests/fixtures/wgpu-small-rgba.json")
UNAVAILABLE = {"WGPU-ADAPTER-NOT-FOUND", "WGPU-NO-COMPATIBLE-ADAPTER"}


def wgpu_prepared() -> vestra.PreparedProject:
    project = vestra.Project.load(FIXTURE)
    try:
        return vestra.Editor().prepare(
            project,
            vestra.PrepareOptions(backend=vestra.BackendPreference.WGPU),
        )
    except vestra.PreparationError as error:
        if os.environ.get("VESTRA_REQUIRE_WGPU") == "1" or any(
            diagnostic.code not in UNAVAILABLE for diagnostic in error.diagnostics
        ):
            raise
        pytest.skip("no compatible WGPU adapter")


def test_prepared_wgpu_video_render_returns_adapter_owned_result(tmp_path: Path) -> None:
    prepared = wgpu_prepared()
    output = tmp_path / "prepared-wgpu.mp4"
    events: list[vestra.RenderEvent] = []
    result = prepared.render_video(
        vestra.PreparedVideoRenderRequest(output), progress=events.append
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
    result = vestra.Editor().render(
        vestra.Project.load(FIXTURE),
        vestra.RenderRequest(output, backend=vestra.BackendPreference.WGPU),
    )
    assert output.exists() and output.stat().st_size > 0
    assert result.timing_scope is vestra.RenderTimingScope.ONE_SHOT
    assert result.selected_backend == "wgpu"
    assert result.adapter is not None
