import os
from pathlib import Path

import pytest

import vestra
from vestra.sources import Color, Image


def _animated_project(tmp_path: Path) -> vestra.Project:
    project = vestra.Project(
        size=(4, 4),
        fps=4,
        duration=1,
        base_directory=tmp_path,
    )
    layer = project.root.add(Color("#ff0000"))
    layer.opacity.keyframe(0, 0)
    layer.opacity.keyframe(0.5, 1)
    return project


def test_prepare_returns_native_session_with_reports_and_random_access_frames(
    tmp_path: Path,
) -> None:
    prepared = _animated_project(tmp_path).prepare(backend="cpu")
    assert isinstance(prepared, vestra.PreparedProject)
    report = prepared.preparation_report
    assert report.requested_backend is vestra.BackendPreference.CPU
    assert report.selected_backend is vestra.BackendKind.CPU
    assert report.supports_single_frame_rendering
    assert (report.width, report.height, report.frame_count) == (4, 4, 4)

    later = prepared.render_frame_seconds(0.75)
    earlier = prepared.render_frame_number(0)
    repeated = prepared.render_frame_ns(later.timestamp_ns)
    assert later.frame_number == repeated.frame_number == 3
    assert later.to_bytes() == repeated.to_bytes()
    assert earlier.to_bytes() != later.to_bytes()
    assert bytes(later) == later.to_bytes()


def test_prepared_project_is_isolated_from_later_editor_mutations(
    tmp_path: Path,
) -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1, base_directory=tmp_path)
    layer = project.root.add(Color("#ff0000"))
    prepared = project.prepare(backend="cpu")
    original = prepared.render_frame_number(0).to_bytes()

    assert isinstance(layer.source, Color)
    layer.source.value = "#00ff00"

    assert prepared.render_frame_number(0).to_bytes() == original
    assert project.prepare(backend="cpu").render_frame_number(0).to_bytes() != original


def test_high_level_lowering_reaches_wgpu_and_matches_cpu_when_available(
    tmp_path: Path,
) -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1, base_directory=tmp_path)
    layer = project.root.add(Color("#204060"))
    layer.opacity = 0.75
    cpu = project.prepare(backend="cpu")

    try:
        wgpu = project.prepare(backend="wgpu")
    except vestra.PreparationError as error:
        unavailable = {"WGPU-ADAPTER-NOT-FOUND", "WGPU-NO-COMPATIBLE-ADAPTER"}
        if os.environ.get("VESTRA_REQUIRE_WGPU") == "1" or any(
            diagnostic.code not in unavailable for diagnostic in error.diagnostics
        ):
            raise
        pytest.skip("no compatible WGPU adapter")

    assert wgpu.preparation_report.selected_backend is vestra.BackendKind.WGPU
    assert wgpu.preparation_report.adapter is not None
    assert (
        wgpu.render_frame_number(0).to_bytes() == cpu.render_frame_number(0).to_bytes()
    )


def test_prepared_video_reuses_session_and_reports_actual_backend(
    tmp_path: Path,
) -> None:
    prepared = _animated_project(tmp_path).prepare(backend="cpu")
    events: list[vestra.RenderEvent] = []
    first_output = tmp_path / "prepared-first.mp4"
    first = prepared.render_video(
        vestra.PreparedVideoRenderRequest(first_output, overwrite=True),
        progress=events.append,
    )
    second_output = tmp_path / "prepared-second.mp4"
    second = prepared.render_video(
        vestra.PreparedVideoRenderRequest(second_output, overwrite=True),
    )
    assert first.selected_backend == second.selected_backend == "cpu"
    assert first.timing_scope is vestra.RenderTimingScope.PREPARED_OPERATION
    assert first.total_frames == second.total_frames == 4
    assert first_output.is_file() and second_output.is_file()
    assert events[0].kind == "started" and events[-1].kind == "completed"
    assert events[0].operation_id == events[-1].operation_id
    assert events[0].stage is None and events[0].fraction is None
    assert all(event.schema_version == 2 for event in events)
    assert all(
        event.kind != "progress" or event.stage == "rendering" for event in events
    )


def test_one_shot_progress_cancellation_and_result_stay_native(tmp_path: Path) -> None:
    project = _animated_project(tmp_path)
    output = tmp_path / "one-shot.mp4"
    events: list[vestra.RenderEvent] = []
    result = project.render(
        output, backend="cpu", overwrite=True, progress=events.append
    )
    assert isinstance(result, vestra.RenderResult)
    assert result.selected_backend == "cpu"
    assert result.timing_scope is vestra.RenderTimingScope.ONE_SHOT
    assert events[0].kind == "started" and events[-1].kind == "completed"
    assert all(event.operation_id == events[0].operation_id for event in events)

    cancelled = vestra.CancellationToken()
    cancelled.cancel()
    cancelled_output = tmp_path / "cancelled.mp4"
    with pytest.raises(vestra.CancelledError) as raised:
        project.render(cancelled_output, backend="cpu", cancellation=cancelled)
    assert raised.value.diagnostics
    assert not cancelled_output.exists()


def test_high_level_prepare_preserves_structured_native_diagnostics(
    tmp_path: Path,
) -> None:
    project = vestra.Project(size=(4, 4), fps=1, duration=1, base_directory=tmp_path)
    project.root.add(Image("missing.png"))
    with pytest.raises(vestra.PreparationError) as raised:
        project.prepare(backend="cpu")
    assert raised.value.diagnostics
    assert all(item.code and item.message for item in raised.value.diagnostics)
