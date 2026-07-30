from pathlib import Path
import subprocess
import sys
from typing import TYPE_CHECKING
import video_editor
from video_editor import AdapterDeviceType, Editor, Frame, PrepareOptions, Project, VideoEditorError

if TYPE_CHECKING:
    project: Project = Project.from_dict({"schema_version": 1, "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": False, "duration_mode": "automatic"}, "assets": [], "visual": {"clips": []}})
    report = Editor().validate(project)
    path: Path = project.base_directory
    assert report.is_valid
    assert path
    error: VideoEditorError
    kind: str = error.kind
    diagnostics = error.diagnostics
    warnings = error.warnings
    assert kind and diagnostics == warnings
    options = PrepareOptions(backend=video_editor.BackendPreference.CPU)
    prepared = Editor().prepare(project, options)
    token = video_editor.CancellationToken()
    request = video_editor.PreparedVideoRenderRequest("out.mp4")

    def on_progress(event: video_editor.RenderEvent) -> object:
        print(event.progress)
        return None

    rendered: video_editor.RenderResult = prepared.render_video(
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
