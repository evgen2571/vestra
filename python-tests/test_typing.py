from pathlib import Path
import subprocess
import sys
from typing import TYPE_CHECKING
import video_editor
from video_editor import AdapterDeviceType, Editor, Frame, PrepareOptions, Project, VideoEditorError
from video_editor.authoring import AudioAsset, ImageAsset, ImageClip, Point, ProjectBuilder, Sizing, SolidColorClip

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
    builder = ProjectBuilder(
        width=160, height=90, frame_rate=video_editor.FrameRate(30, 1),
        output_path="out.mp4", duration=1.0,
    )
    authored: dict[str, object] = builder.to_dict()
    authored_project: Project = builder.build()
    authored_report: video_editor.ValidationReport = builder.validate()
    point = Point(1.0, 1.0)
    sizing_original = Sizing.original()
    sizing_scale = Sizing.scale(1.25)
    sizing_stretch = Sizing.stretch(width=1280, height=720)
    image: ImageAsset = builder.add_image_asset("cover.png")
    audio: AudioAsset = builder.add_audio_asset("music.wav")
    image_clip: ImageClip = builder.add_image_clip(source=image, start=0.0, duration=1.0, layer=0)
    solid_clip: SolidColorClip = builder.add_solid_color_clip(colour="#112233", start=0.0, duration=1.0, layer=1)
    builder.set_audio(asset=audio, timeline_start=0.0, trim_start=0.0)
    assert authored and authored_project and authored_report and point and sizing_original and sizing_scale and sizing_stretch and image_clip and solid_clip


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


def test_negative_authoring_immutability_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_immutable_assignment.py"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    assert "read-only" in result.stdout


def test_negative_authoring_track_replacement_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_track_replacement.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("read-only") == 4


def test_negative_authoring_sizing_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_invalid_sizing.py"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    # Python's type system treats bool as an int subtype. Runtime validation
    # rejects those two calls; mypy still catches the string dimension.
    assert result.stdout.count("Argument") == 1


def test_negative_authoring_asset_kind_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "python-tests/typing_failures/authoring_wrong_asset_kind.py"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert result.stdout.count("Argument") == 2
