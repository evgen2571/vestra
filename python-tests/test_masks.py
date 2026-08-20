from __future__ import annotations

import os

import pytest

from vestra import Circle, Crossfade, Ellipse, MaskOperation, Polygon, Project, Rectangle


def test_layer_masks_are_owned_and_lowered() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))

    outer = layer.masks.add(Ellipse(width=20, height=20, fill="#ffffff"), id="outer")
    outer.operation = MaskOperation.REPLACE
    outer.strength = 0.75
    outer.invert = True
    outer.invert = False
    layer.masks.add(Circle(radius=4, fill="#ffffff"), operation=MaskOperation.SUBTRACT, id="hole")

    assert [mask.id for mask in layer.masks.items] == ["outer", "hole"]
    masks = project.snapshot().to_dict()["visual"]["clips"][0]["masks"]
    assert masks[0]["operation"] == "replace"
    assert masks[0]["strength"] == 0.75
    assert masks[1]["operation"] == "subtract"

    with pytest.raises(ValueError):
        layer.masks.add(Circle(radius=2, fill="#ffffff"), id="")
    with pytest.raises(ValueError):
        layer.masks.add(Circle(radius=2, fill="#ffffff"), id="outer")

    layer.masks.remove("hole")
    assert [mask.id for mask in layer.masks.items] == ["outer"]
    layer.masks.clear()
    assert layer.masks.items == ()


@pytest.mark.parametrize("backend", ["cpu", "wgpu"])
def test_geometric_masks_clip_layer_coverage(backend: str) -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    layer.masks.add(Ellipse(width=20, height=20, fill="#ffffff"), id="outer")
    layer.masks.add(Circle(radius=4, fill="#ffffff"), operation=MaskOperation.SUBTRACT, id="hole")

    try:
        pixels = project.render_frame(0, backend=backend).to_bytes()
    except Exception as error:
        if backend == "wgpu" and os.environ.get("VESTRA_REQUIRE_WGPU") != "1":
            if "adapter" in str(error).lower() or "wgpu" in str(error).lower():
                pytest.skip("no compatible WGPU adapter")
        raise

    center = (16 * 32 + 16) * 4
    edge = (10 * 32 + 16) * 4
    outside = (0 * 32 + 0) * 4
    assert pixels[center : center + 3] == bytes((0, 0, 0))
    assert pixels[edge : edge + 3] == bytes((255, 0, 0))
    assert pixels[outside : outside + 3] == bytes((0, 0, 0))


def test_masks_survive_groups_and_transition_endpoints() -> None:
    project = Project(size=(32, 32), fps=10, duration=2)
    group = project.root.group(duration=2)
    grouped = group.add(Rectangle(width=32, height=32, fill="#00ff00"), duration=2)
    grouped.masks.add(Ellipse(width=20, height=20, fill="#ffffff"), id="group-mask")

    outgoing = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"), duration=2)
    incoming = project.root.add(Rectangle(width=32, height=32, fill="#0000ff"), duration=2)
    incoming.masks.add(Ellipse(width=20, height=20, fill="#ffffff"), id="transition-mask")
    project.root.transitions.add(outgoing, incoming, Crossfade(), start=0.5, duration=0.5)

    assert project.render_frame(0, backend="cpu").to_bytes()
    assert project.render_frame(0.75, backend="cpu").to_bytes()


@pytest.mark.parametrize("backend", ["cpu", "wgpu"])
def test_mask_local_transform_moves_coverage_with_the_layer(backend: str) -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    mask = layer.masks.add(Ellipse(width=10, height=10, fill="#ffffff"), id="moved")
    mask.transform.position = (0.75, 0.5)

    try:
        pixels = project.render_frame(0, backend=backend).to_bytes()
    except Exception as error:
        if backend == "wgpu" and os.environ.get("VESTRA_REQUIRE_WGPU") != "1":
            if "adapter" in str(error).lower() or "wgpu" in str(error).lower():
                pytest.skip("no compatible WGPU adapter")
        raise

    left = (16 * 32 + 16) * 4
    moved = (16 * 32 + 24) * 4
    assert pixels[left : left + 3] == bytes((0, 0, 0))
    assert pixels[moved : moved + 3] == bytes((255, 0, 0))


def test_mask_invert_strength_and_polygon_coverage() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    inverted = layer.masks.add(Ellipse(width=16, height=16, fill="#ffffff"), id="invert")
    inverted.invert = True
    pixels = project.render_frame(0, backend="cpu").to_bytes()
    center = (16 * 32 + 16) * 4
    corner = 0
    assert pixels[center : center + 3] == bytes((0, 0, 0))
    assert pixels[corner : corner + 3] == bytes((255, 0, 0))

    layer.masks.clear()
    partial = layer.masks.add(Ellipse(width=16, height=16, fill="#ffffff"), id="partial")
    partial.strength = 0.5
    pixels = project.render_frame(0, backend="cpu").to_bytes()
    assert 120 <= pixels[corner] <= 135

    layer.masks.clear()
    layer.masks.add(
        Polygon(points=[(-8, -8), (8, -8), (0, 8)], fill="#ffffff"),
        id="polygon",
    )
    pixels = project.render_frame(0, backend="cpu").to_bytes()
    assert pixels[center : center + 3] == bytes((255, 0, 0))
    assert pixels[corner : corner + 3] == bytes((0, 0, 0))
