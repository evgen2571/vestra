from pathlib import Path
import subprocess
import sys

import numpy as np
import pytest

import vestra


def project() -> vestra.Project:
    value = vestra.Project(size=(16, 8), fps=4, duration=2, background="#102030")
    layer = value.root.add(vestra.Rectangle(width=8, height=8, fill="#ff0000"))
    layer.opacity.keyframe(0, 0)
    layer.opacity.keyframe(1, 1)
    return value


def test_numpy_frame_is_owned_writable_and_tightly_packed():
    prepared = project().prepare(backend="cpu")
    frame = prepared.render_frame_number(4)
    array = frame.to_numpy()
    assert array.shape == (8, 16, 4)
    assert array.dtype == np.uint8
    assert array.strides == (64, 4, 1)
    assert array.flags.c_contiguous and array.flags.owndata and array.flags.writeable
    assert array.tobytes() == frame.to_bytes()
    expected = array.copy()
    array[:] = 0
    assert frame.to_bytes() == expected.tobytes()
    prepared.render_frame_number(0)
    del prepared, frame
    assert np.array_equal(expected[0, 0], [16, 32, 48, 255])
    assert not array.any()


@pytest.mark.parametrize("extension", ["png", "jpg", "webp"])
def test_high_level_still_and_frame_index(tmp_path: Path, extension: str):
    value = project()
    path = tmp_path / f"frame.{extension}"
    frame = value.render_frame(1, path, backend="cpu")
    assert frame.frame_number == 4
    assert path.stat().st_size > 0
    with pytest.raises(vestra.FrameRenderError):
        frame.save(path)
    frame.save(path, overwrite=True)
    indexed = value.render_frame(frame_number=4, backend="cpu")
    assert indexed.to_bytes() == frame.to_bytes()
    assert np.array_equal(value.get_frame(1, backend="cpu"), frame.to_numpy())
    assert np.array_equal(
        value.get_frame(frame_number=4, backend="cpu"), frame.to_numpy()
    )


def test_selected_frames_preserve_request_order_and_duplicates(tmp_path: Path):
    value = project()
    assert (
        value.render_frames(times=[1.25, 0, 1.25], directory=tmp_path, backend="cpu")
        == 3
    )
    paths = [tmp_path / f"frame_{number:06d}.png" for number in range(3)]
    assert paths[0].read_bytes() == paths[2].read_bytes()
    assert paths[0].read_bytes() != paths[1].read_bytes()
    assert (
        value.render_frames(
            indices=[5, 0, 5], directory=tmp_path / "indices", backend="cpu"
        )
        == 3
    )
    assert [path.read_bytes() for path in paths] == [
        (tmp_path / "indices" / path.name).read_bytes() for path in paths
    ]


def test_continuous_sequence_uses_project_frame_boundaries(tmp_path: Path):
    value = project()
    count = value.render_frames(
        directory=tmp_path, start=0.26, end=1.01, backend="cpu", format="webp"
    )
    assert count == 3  # frames 2, 3, 4; times in [0.26, 1.01)
    prepared = value.prepare(backend="cpu")
    for number, index in enumerate([2, 3, 4]):
        path = tmp_path / f"frame_{number:06d}.webp"
        reference = tmp_path / f"reference-{index}.webp"
        prepared.render_frame_number(index).save(reference)
        assert path.read_bytes() == reference.read_bytes()


@pytest.mark.parametrize(
    "kwargs",
    [
        {"times": [], "indices": [0]},
        {"times": [0], "start": 0.1},
        {"start": 1, "end": 1},
        {"start": -1},
        {"end": 3},
        {"end": 1e308},
        {"start": 0.01, "end": 0.02},
        {"times": [-1]},
        {"times": [float("nan")]},
        {"times": [1e308]},
        {"indices": [True]},
        {"indices": [8]},
        {"format": "gif"},
    ],
)
def test_invalid_sequences_do_not_create_output(tmp_path: Path, kwargs):
    with pytest.raises((TypeError, ValueError, vestra.FrameRenderError)):
        project().render_frames(directory=tmp_path / "frames", backend="cpu", **kwargs)
    assert not (tmp_path / "frames").exists()


def test_sequence_does_not_overwrite_an_earlier_file_before_a_collision(tmp_path: Path):
    path = tmp_path / "frame_000001.png"
    path.write_bytes(b"keep")
    with pytest.raises(FileExistsError):
        project().render_frames(times=[0, 1], directory=tmp_path, backend="cpu")
    assert not (tmp_path / "frame_000000.png").exists()
    assert path.read_bytes() == b"keep"


@pytest.mark.parametrize("kwargs", [{}, {"seconds": 0, "frame_number": 0}])
def test_frame_selector_requires_exactly_one_time_or_index(kwargs):
    with pytest.raises(ValueError):
        project().render_frame(backend="cpu", **kwargs)


def test_empty_selection_does_not_create_directory(tmp_path: Path):
    assert (
        project().render_frames(times=[], directory=tmp_path / "frames", backend="cpu")
        == 0
    )
    assert not (tmp_path / "frames").exists()


def test_fractional_fps_sequence_boundaries(tmp_path: Path):
    value = vestra.Project(size=(2, 2), fps=(30000, 1001), duration=0.1)
    count = value.render_frames(
        directory=tmp_path, start=0.033366667, end=0.066733334, backend="cpu"
    )
    assert count == 1  # only frame 2; frame 1 is before the requested start


@pytest.mark.skipif(sys.platform == "win32", reason="requires POSIX file size limits")
@pytest.mark.parametrize("extension", ["png", "jpg", "webp"])
def test_still_flush_failure_preserves_previous_destination(
    tmp_path: Path, extension: str
):
    output = tmp_path / f"frame.{extension}"
    output.write_bytes(b"previous output")
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            """
import resource
import signal
import sys
import vestra

frame = vestra.Project(size=(2, 2), fps=1, duration=1).render_frame(0, backend="cpu")
signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
resource.setrlimit(resource.RLIMIT_FSIZE, (0, resource.getrlimit(resource.RLIMIT_FSIZE)[1]))
try:
    frame.save(sys.argv[1], overwrite=True)
except vestra.FrameRenderError:
    pass
else:
    raise AssertionError("write failure unexpectedly published")
""",
            str(output),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert output.read_bytes() == b"previous output"
    assert list(tmp_path.iterdir()) == [output]
