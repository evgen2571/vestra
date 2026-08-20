from __future__ import annotations

import os

import pytest

from vestra import Circle, Crossfade, Ellipse, Line, MaskOperation, Polygon, Project, Rectangle
from vestra.effects import GaussianBlur


def _render(project: Project, backend: str) -> bytes:
    try:
        return project.render_frame(0, backend=backend).to_bytes()
    except Exception as error:
        if backend == "wgpu" and os.environ.get("VESTRA_REQUIRE_WGPU") != "1":
            if "adapter" in str(error).lower() or "wgpu" in str(error).lower():
                pytest.skip("no compatible WGPU adapter")
        raise


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
    assert masks[0]["strength"]["base_value"] == 0.75
    assert masks[0]["feather"]["base_value"] == 0.0
    assert masks[1]["operation"] == "subtract"

    with pytest.raises(ValueError):
        layer.masks.add(Circle(radius=2, fill="#ffffff"), id="")
    with pytest.raises(ValueError):
        layer.masks.add(Circle(radius=2, fill="#ffffff"), id="outer")

    layer.masks.remove("hole")
    assert [mask.id for mask in layer.masks.items] == ["outer"]
    layer.masks.clear()
    assert layer.masks.items == ()


def test_masks_expose_dynamic_scalar_properties() -> None:
    project = Project(size=(32, 32), fps=1, duration=2)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    mask = layer.masks.add(Ellipse(width=20, height=20, fill="#ffffff"), feather=8)
    mask.strength.keyframe(1, 0.25)
    mask.feather.keyframe(1, 16)
    mask.transform.position.keyframe(1, (0.7, 0.5))
    canonical = project.snapshot().to_dict()["visual"]["clips"][0]["masks"][0]
    assert canonical["strength"]["keyframes"][0]["value"] == 0.25
    assert canonical["feather"]["base_value"] == 8.0
    assert canonical["feather"]["keyframes"][0]["value"] == 16.0
def test_generated_mask_ids_skip_removed_ids() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))

    first = layer.masks.add(Circle(radius=4, fill="#ffffff"))
    second = layer.masks.add(Circle(radius=4, fill="#ffffff"))
    third = layer.masks.add(Circle(radius=4, fill="#ffffff"))
    layer.masks.remove(second)
    replacement = layer.masks.add(Circle(radius=4, fill="#ffffff"))

    assert [mask.id for mask in layer.masks.items] == [first.id, third.id, replacement.id]
    assert len({mask.id for mask in layer.masks.items}) == 3


def test_mask_ids_reject_whitespace_and_non_strings() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))

    with pytest.raises(ValueError):
        layer.masks.add(Circle(radius=4, fill="#ffffff"), id="   ")
    with pytest.raises(TypeError):
        layer.masks.add(Circle(radius=4, fill="#ffffff"), id=42)  # type: ignore[arg-type]


def test_line_is_not_a_supported_mask_input() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))

    with pytest.raises((TypeError, ValueError), match="Line|line|mask"):
        layer.masks.add(
            Line(start=(0, 0), end=(8, 8), stroke="#ffffff", stroke_width=1)
        )


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


@pytest.mark.parametrize("backend", ["cpu", "wgpu"])
def test_rendered_replace_uses_only_the_replacing_mask(backend: str) -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    first = layer.masks.add(Rectangle(width=16, height=32, fill="#ffffff"), id="first")
    first.transform.position = (0.25, 0.5)
    replacing = layer.masks.add(
        Rectangle(width=8, height=32, fill="#ffffff"),
        operation=MaskOperation.REPLACE,
        id="replacing",
    )
    replacing.transform.position = (0.75, 0.5)

    pixels = _render(project, backend)
    left = (16 * 32 + 8) * 4
    right = (16 * 32 + 24) * 4
    assert pixels[left : left + 3] == bytes((0, 0, 0))
    assert pixels[right : right + 3] == bytes((255, 0, 0))


@pytest.mark.parametrize("backend", ["cpu", "wgpu"])
def test_rendered_union_keeps_both_separated_regions(backend: str) -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    left_mask = layer.masks.add(Rectangle(width=8, height=32, fill="#ffffff"), id="left")
    left_mask.transform.position = (0.25, 0.5)
    right_mask = layer.masks.add(
        Rectangle(width=8, height=32, fill="#ffffff"),
        operation=MaskOperation.UNION,
        id="right",
    )
    right_mask.transform.position = (0.75, 0.5)

    pixels = _render(project, backend)
    left = (16 * 32 + 8) * 4
    gap = (16 * 32 + 16) * 4
    right = (16 * 32 + 24) * 4
    assert pixels[left : left + 3] == bytes((255, 0, 0))
    assert pixels[gap : gap + 3] == bytes((0, 0, 0))
    assert pixels[right : right + 3] == bytes((255, 0, 0))


def test_rendered_effect_is_clipped_by_mask_after_blur() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=8, height=8, fill="#ff0000"))
    layer.effects.add(GaussianBlur(radius=4))
    layer.masks.add(Rectangle(width=8, height=8, fill="#ffffff"), id="clip")

    pixels = _render(project, "cpu")
    outside = (16 * 32 + 11) * 4
    center = (16 * 32 + 16) * 4
    assert pixels[outside : outside + 3] == bytes((0, 0, 0))
    assert pixels[center] > 200


def test_rendered_mask_is_applied_before_layer_opacity() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(
        Rectangle(width=16, height=16, fill="#ff0000"), opacity=0.5
    )
    layer.masks.add(Rectangle(width=8, height=8, fill="#ffffff"), id="clip")

    pixels = _render(project, "cpu")
    inside = (16 * 32 + 16) * 4
    outside = (16 * 32 + 4) * 4
    assert 120 <= pixels[inside] <= 135
    assert pixels[inside + 1 : inside + 3] == bytes((0, 0))
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


def test_group_mask_clips_the_composed_group_result() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    group = project.root.group(duration=1)
    group.add(Rectangle(width=32, height=32, fill="#00ff00"), duration=1)
    child = group.add(Rectangle(width=8, height=8, fill="#0000ff"), duration=1)
    child.masks.add(Ellipse(width=4, height=4, fill="#ffffff"), id="child-mask")
    group.masks.add(Ellipse(width=16, height=16, fill="#ffffff"), id="group-mask")

    pixels = project.render_frame(0, backend="cpu").to_bytes()
    center = (16 * 32 + 16) * 4
    outside = (0 * 32 + 0) * 4
    assert pixels[center : center + 3] == bytes((0, 0, 255))
    assert pixels[outside : outside + 3] == bytes((0, 0, 0))


def test_layer_and_mask_local_transforms_use_distinct_coordinate_spaces() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    layer.transform.position = (0.75, 0.5)
    layer.transform.scale = (1.3, 1.0)
    mask = layer.masks.add(Ellipse(width=10, height=10, fill="#ffffff"), id="local")
    mask.transform.position = (0.65, 0.5)
    mask.transform.rotation_degrees.value = 20.0

    pixels = project.render_frame(0, backend="cpu").to_bytes()
    left = (16 * 32 + 16) * 4
    moved = (16 * 32 + 24) * 4
    assert pixels[left : left + 3] == bytes((0, 0, 0))
    assert pixels[moved] > 100 and pixels[moved + 1 : moved + 3] == bytes((0, 0))


def test_transformed_mask_cpu_wgpu_parity_is_tight_when_wgpu_is_available() -> None:
    def render(backend: str) -> bytes:
        project = Project(size=(32, 32), fps=1, duration=1)
        layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
        mask = layer.masks.add(Ellipse(width=14, height=10, fill="#ffffff"), id="parity")
        mask.transform.position = (0.63, 0.47)
        mask.transform.scale = (1.3, 0.8)
        mask.transform.rotation_degrees.value = 20.0
        return project.render_frame(0, backend=backend).to_bytes()

    cpu = render("cpu")
    try:
        wgpu = render("wgpu")
    except Exception as error:
        if os.environ.get("VESTRA_REQUIRE_WGPU") != "1" and "adapter" in str(error).lower():
            pytest.skip("no compatible WGPU adapter")
        raise
    differences = [abs(left - right) for left, right in zip(cpu, wgpu)]
    assert max(differences) <= 32


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
