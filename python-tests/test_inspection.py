from pathlib import Path

import pytest

import video_editor


def test_inspection_exposes_sdk_owned_snapshot() -> None:
    fixture = Path("tests/fixtures/wgpu-small-rgba.json")
    project = video_editor.Project.load(fixture)
    editor = video_editor.Editor()

    preflight = editor.preflight(project, video_editor.PreflightOptions.for_inspection())
    assert preflight.is_ready
    assert isinstance(preflight.diagnostics, tuple)

    report = editor.inspect(project)
    assert report.project == fixture
    assert report.output.width == 174
    assert report.output.height == 130
    assert report.output.frame_rate == "1/1"
    assert report.assets.images == 1
    assert report.audio is None
    assert report.warnings == ()


def test_missing_asset_is_a_preflight_report_not_an_exception(tmp_path: Path) -> None:
    project = video_editor.Project.from_dict(
        {
            "schema_version": 2,
            "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": False, "duration_mode": "automatic"},
            "assets": [{"id": "missing", "type": "image", "source": "missing.png"}],
            "visual": {"clips": []},
        },
        base_directory=tmp_path,
    )
    report = video_editor.Editor().preflight(project, video_editor.PreflightOptions.for_inspection())
    assert not report.is_ready
    assert report.errors
    assert isinstance(report.errors, tuple)


def test_inspection_failure_retains_structured_editor_error(tmp_path: Path) -> None:
    project = video_editor.Project.from_dict(
        {
            "schema_version": 2,
            "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": False, "duration_mode": "automatic"},
            "assets": [{"id": "missing", "type": "image", "source": "missing.png"}],
            "visual": {"clips": []},
        },
        base_directory=tmp_path,
    )
    with pytest.raises(video_editor.VideoEditorError) as captured:
        video_editor.Editor().inspect(project)
    error = captured.value
    assert error.kind == "project"
    assert isinstance(error.diagnostics, tuple)
    assert isinstance(error.warnings, tuple)
    diagnostic = error.diagnostics[0]
    assert diagnostic.code
    assert diagnostic.category is not None
    assert diagnostic.severity is not None
    assert diagnostic.message
    assert diagnostic.pointer is not None
    with pytest.raises(AttributeError):
        diagnostic.code = "changed"  # type: ignore[misc]
