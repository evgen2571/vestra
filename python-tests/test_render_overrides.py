from pathlib import Path
import subprocess

import numpy as np
import pytest

import vestra
from vestra.authoring import Quality


@pytest.mark.parametrize("resolution", [None, (32, 16), (128, 64), (64, 64)])
def test_frame_and_sequence_overrides_keep_layout_and_do_not_edit_project(
    tmp_path: Path, resolution
):
    project = vestra.Project(size=(64, 32), fps=30, duration=1, background="#000000")
    project.root.add(vestra.Rectangle(width=32, height=16, fill="#ff0000"))
    before = project.snapshot().to_dict()
    frame = project.get_frame(0, backend="cpu", resolution=resolution)
    width, height = resolution or (64, 32)
    assert frame.shape == (height, width, 4)
    assert np.array_equal(frame[height // 2, width // 2], [255, 0, 0, 255])
    assert np.array_equal(frame[height // 2, width // 8], [0, 0, 0, 255])
    assert np.array_equal(frame[height // 8, width // 2], [0, 0, 0, 255])
    coverage = np.count_nonzero(frame[:, :, 0])
    scale = min(width / 64, height / 32)
    assert coverage == round(32 * scale) * round(16 * scale)
    count = project.render_frames(
        directory=tmp_path,
        backend="cpu",
        resolution=resolution,
        fps=4,
    )
    assert count == 4
    assert len(list(tmp_path.glob("frame_*.png"))) == 4
    assert project.snapshot().to_dict() == before


@pytest.mark.parametrize(
    "resolution", [(0, 16), (-2, 16), (True, 16), (2.5, 16), (16,), [16, 16]]
)
def test_resolution_override_rejects_invalid_dimensions(resolution):
    project = vestra.Project(size=(32, 16), fps=30, duration=1)
    with pytest.raises((TypeError, ValueError)):
        project.prepare(backend="cpu", resolution=resolution)


@pytest.mark.parametrize("resolution", [(1, 2), (3, 4), (8194, 16)])
def test_resolution_override_uses_output_limits(resolution):
    project = vestra.Project(size=(32, 16), fps=30, duration=1)
    with pytest.raises(vestra.PreparationError, match="runtime"):
        project.prepare(backend="cpu", resolution=resolution)


def test_runtime_fps_and_quality_preserve_project_and_sample_times(tmp_path: Path):
    project = vestra.Project(size=(32, 16), fps=30, duration=1, quality=Quality.HIGH)
    layer = project.root.add(vestra.sources.Color("#ff0000"))
    layer.opacity.keyframe(0, 0)
    layer.opacity.keyframe(1, 1)
    before = project.snapshot().to_dict()
    prepared = project.prepare(backend="cpu", fps=(10, 1))
    assert prepared.preparation_report.frame_count == 10
    assert (
        prepared.render_frame_number(5).to_bytes()
        == project.render_frame(0.5, backend="cpu").to_bytes()
    )
    result = project.render(
        tmp_path / "override.mp4",
        backend="cpu",
        fps=10,
        quality=Quality.PREVIEW,
        start=0.21,
        end=0.79,
        show_progress=False,
    )
    assert result.total_frames == 5
    assert result.duration_seconds == pytest.approx(0.5)
    assert project.snapshot().to_dict() == before


def test_preview_reduces_fps_and_allows_explicit_overrides(tmp_path: Path):
    project = vestra.Project(size=(32, 16), fps=30, duration=1)
    before = project.snapshot().to_dict()
    result = project.render_preview(
        tmp_path / "preview.mp4", backend="cpu", show_progress=False
    )
    assert result.total_frames == 15
    result = project.render_preview(
        tmp_path / "custom.mp4",
        backend="cpu",
        fps=24,
        quality=Quality.HIGH,
        show_progress=False,
    )
    assert result.total_frames == 24
    assert project.snapshot().to_dict() == before


@pytest.mark.parametrize("fps", [0, -1, True, 2.5, (1, 0), (True, 1)])
def test_fps_override_validation(fps):
    project = vestra.Project(size=(32, 16), fps=30, duration=1)
    with pytest.raises((TypeError, ValueError)):
        project.prepare(backend="cpu", fps=fps)


def test_quality_override_validation(tmp_path: Path):
    project = vestra.Project(size=(32, 16), fps=30, duration=1)
    with pytest.raises(TypeError, match="quality"):
        project.render(tmp_path / "bad.mp4", quality="high", backend="cpu")


def first_frame(path: Path, width: int, height: int) -> np.ndarray:
    raw = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(path),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]
    )
    return np.frombuffer(raw, dtype=np.uint8).reshape(height, width, 3)


@pytest.mark.parametrize("grouped", [False, True])
def test_preview_preserves_authored_layout_at_reduced_resolution(
    tmp_path: Path, grouped: bool
):
    project = vestra.Project(
        size=(1280, 720), fps=30, duration=0.1, quality=Quality.HIGH
    )
    parent = project.root.group("group") if grouped else project.root
    parent.add(vestra.Rectangle(width=640, height=360, fill="#ff0000"))
    before = project.snapshot().to_dict()
    full = tmp_path / "full.mp4"
    preview = tmp_path / "preview.mp4"
    project.render(full, backend="cpu", show_progress=False)
    result = project.render(preview, preview=True, backend="cpu", show_progress=False)
    assert project.snapshot().to_dict() == before
    assert (result.width, result.height) == (640, 360)
    reference = first_frame(full, 1280, 720)
    decoded = first_frame(preview, 640, 360)
    for x, y in [(0.125, 0.5), (0.5, 0.125), (0.5, 0.5), (0.9, 0.9)]:
        assert np.allclose(
            decoded[int(y * 360), int(x * 640)],
            reference[int(y * 720), int(x * 1280)],
            atol=4,
        )
