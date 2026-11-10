"""Independent frame assertions for Motion Tile's pre-transform bounds."""

import os

import pytest

from vestra import PreparationError, Project, Rectangle
from vestra.effects import MotionTile


@pytest.mark.parametrize("mirror", [False, True])
@pytest.mark.parametrize("extent,uncovered", [(100, 117504), (200, 0), (400, 0)])
def test_centered_motion_tile_coverage(
    extent: int, uncovered: int, mirror: bool
) -> None:
    project = Project(
        size=(640, 360), fps=30, duration=1, background="#ff00ff", output_audio=False
    )
    layer = project.root.add(
        Rectangle(width=640, height=360, fill="#93cbb5"),
        position=(0.5, 0.5),
        anchor=(0.5, 0.5),
        scale=0.7,
    )
    layer.effects.add(MotionTile(extent, extent, mirror_edges=mirror))
    prepared = project.prepare(backend="cpu")
    assert str(prepared.preparation_report.selected_backend) == "cpu"
    assert prepared.preparation_report.fallback is None
    rgba = prepared.render_frame_seconds(0.5).to_bytes()
    assert (
        sum(rgba[i : i + 4] == b"\xff\x00\xff\xff" for i in range(0, len(rgba), 4))
        == uncovered
    )


@pytest.mark.parametrize("backend", ["cpu", "wgpu"])
def test_animated_tile_center_moves_bounds_without_stale_frames(backend: str) -> None:
    project = Project(size=(80, 48), fps=4, duration=1, output_audio=False)
    layer = project.root.add(
        Rectangle(width=20, height=12, fill="#93cbb5"),
        position=(0.5, 0.5),
        anchor=(0.5, 0.5),
    )
    tile = layer.effects.add(MotionTile(200, 200))
    tile.tile_center.keyframe(0, (0, 0))
    tile.tile_center.keyframe(0.5, (1, 1))
    try:
        prepared = project.prepare(backend=backend)
    except PreparationError as error:
        unavailable = {"WGPU-ADAPTER-NOT-FOUND", "WGPU-NO-COMPATIBLE-ADAPTER"}
        if (
            backend != "wgpu"
            or os.environ.get("VESTRA_REQUIRE_WGPU") == "1"
            or any(
                diagnostic.code not in unavailable for diagnostic in error.diagnostics
            )
        ):
            raise
        pytest.skip("no compatible WGPU adapter")
    assert str(prepared.preparation_report.selected_backend) == backend
    assert prepared.preparation_report.fallback is None
    frames = {}
    for time, bounds in [
        (0.5, (10, 6, 50, 30)),
        (0, (30, 18, 70, 42)),
        (0.25, (20, 12, 60, 36)),
        (0.5, (10, 6, 50, 30)),
    ]:
        rgba = prepared.render_frame_seconds(time).to_bytes()
        left, top, right, bottom = bounds
        for y in range(48):
            for x in range(80):
                expected = (
                    b"\x93\xcb\xb5\xff"
                    if left <= x < right and top <= y < bottom
                    else b"\x00\x00\x00\xff"
                )
                assert rgba[(y * 80 + x) * 4 : (y * 80 + x + 1) * 4] == expected, (
                    time,
                    x,
                    y,
                )
        if time in frames:
            assert rgba == frames[time]
        frames[time] = rgba
    fresh = project.prepare(backend=backend)
    assert fresh.render_frame_seconds(0.5).to_bytes() == frames[0.5]
