from __future__ import annotations

import os

import pytest

from vestra import (
    Circle,
    Crossfade,
    Ellipse,
    Line,
    Polygon,
    Project,
    Rectangle,
    RoundedRectangle,
)
from vestra.effects import Brightness


def test_shape_sources_lower_to_one_canonical_shape_model() -> None:
    project = Project(size=(64, 64), fps=1, duration=1)
    sources = [
        Rectangle(width=20, height=10, fill="#ff0000"),
        RoundedRectangle(width=20, height=10, corner_radius=2, fill="#00ff00"),
        Ellipse(width=20, height=10, fill="#0000ff"),
        Circle(radius=5, fill="#ffffff"),
        Line(start=(-4, 0), end=(4, 0), stroke="#ffffff", stroke_width=2),
        Polygon(points=[(-4, 4), (0, -4), (4, 4)], fill="#ffffff"),
    ]
    for index, source in enumerate(sources):
        project.root.add(source, id=f"shape-{index}")

    clips = project.snapshot().to_dict()["visual"]["clips"]
    assert all(clip["source"]["type"] == "shape" for clip in clips)
    assert clips[1]["source"]["geometry"]["corner_radius"] == 2.0
    assert clips[3]["source"]["geometry"] == {
        "type": "ellipse", "width": 10.0, "height": 10.0
    }


def test_rectangle_shape_renders_through_generic_cpu_raster_path() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    project.root.add(Rectangle(width=10, height=6, fill="#ff0000"))
    pixels = project.render_frame(0, backend="cpu").to_bytes()
    assert pixels[(16 * 32 + 16) * 4 : (16 * 32 + 17) * 4] == bytes((255, 0, 0, 255))


def test_shapes_use_layer_presentation_effects_groups_and_random_access() -> None:
    project = Project(size=(32, 32), fps=1, duration=2)
    group = project.root.group(duration=2)
    background = group.add(Rectangle(width=32, height=32, fill="#0000ff"), duration=2)
    foreground = group.add(Rectangle(width=12, height=12, fill="#ff0000"), duration=2)
    foreground.opacity = 0.5
    foreground.transform.position = (0.75, 0.75)
    foreground.effects.add(Brightness(0.1))

    first = project.render_frame(0, backend="cpu").to_bytes()
    second = project.render_frame(1, backend="cpu").to_bytes()
    again = project.render_frame(0, backend="cpu").to_bytes()

    assert first == again
    assert first != bytes(32 * 32 * 4)
    assert background.source.to_canonical()["type"] == "shape"
    assert foreground.source.to_canonical()["type"] == "shape"
    assert second == first


def test_shapes_can_be_transition_endpoints() -> None:
    project = Project(size=(16, 16), fps=10, duration=4)
    outgoing = project.root.add(Rectangle(width=16, height=16, fill="#ff0000"), duration=4)
    incoming = project.root.add(Circle(radius=8, fill="#0000ff"), duration=4)
    project.root.transitions.add(outgoing, incoming, Crossfade(), start=1, duration=1)

    center = (8 * 16 + 8) * 4
    assert project.render_frame(0.5, backend="cpu").to_bytes()[center : center + 4] == bytes((255, 0, 0, 255))
    midpoint = project.render_frame(1.5, backend="cpu").to_bytes()[center : center + 4]
    assert midpoint not in {bytes((255, 0, 0, 255)), bytes((0, 0, 255, 255))}
    assert project.render_frame(2.5, backend="cpu").to_bytes()[center : center + 4] == bytes((0, 0, 255, 255))


def test_shape_reaches_wgpu_when_adapter_is_available() -> None:
    project = Project(size=(16, 16), fps=1, duration=1)
    project.root.add(Circle(radius=4, fill="#ff0000"))
    try:
        frame = project.render_frame(0, backend="wgpu")
    except Exception as error:
        if os.environ.get("VESTRA_REQUIRE_WGPU") == "1":
            raise
        if "adapter" in str(error).lower() or "wgpu" in str(error).lower():
            pytest.skip("no compatible WGPU adapter")
        raise
    assert frame.to_bytes()[(8 * 16 + 8) * 4 : (8 * 16 + 9) * 4] == bytes((255, 0, 0, 255))


@pytest.mark.parametrize(
    "factory",
    [
        lambda: Rectangle(width=0, height=2, fill="#fff000"),
        lambda: Ellipse(width=2, height=-1, fill="#fff000"),
        lambda: Circle(radius=0, fill="#fff000"),
        lambda: Line(start=(0, 0), end=(0, 0), stroke="#fff000", stroke_width=1),
        lambda: Polygon(points=[(0, 0), (1, 1)], fill="#fff000"),
    ],
)
def test_shape_sources_reject_invalid_geometry(factory: object) -> None:
    with pytest.raises((TypeError, ValueError)):
        factory()  # type: ignore[operator]
