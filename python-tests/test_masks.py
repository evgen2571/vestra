from __future__ import annotations

import os
import struct
import zlib
from pathlib import Path
from typing import get_type_hints

import pytest

from vestra import (
    AudioGainKeyframe,
    Circle,
    Crossfade,
    Ellipse,
    Image,
    ImageMaskMode,
    Line,
    MaskOperation,
    Polygon,
    Project,
    Rectangle,
    Shape,
)
from vestra.effects import GaussianBlur
from vestra.authoring.values import Crop, Sizing

ROOT = Path(__file__).resolve().parents[1]
_UNAVAILABLE_WGPU_CODES = {"WGPU-ADAPTER-NOT-FOUND", "WGPU-NO-COMPATIBLE-ADAPTER"}


def _write_rgba_png(path: Path, width: int, height: int, pixels: list[tuple[int, int, int, int]]) -> None:
    rows = b"".join(
        b"\x00" + bytes(channel for pixel in pixels[row * width : (row + 1) * width] for channel in pixel)
        for row in range(height)
    )

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(rows))
        + chunk(b"IEND", b"")
    )


def _render(project: Project, backend: str) -> bytes:
    try:
        return project.render_frame(0, backend=backend).to_bytes()
    except Exception as error:
        if (
            backend == "wgpu"
            and os.environ.get("VESTRA_REQUIRE_WGPU") != "1"
            and _wgpu_environment_unavailable(error)
        ):
            pytest.skip("no compatible WGPU adapter")
        raise


def _wgpu_environment_unavailable(error: Exception) -> bool:
    diagnostics = getattr(error, "diagnostics", ())
    if diagnostics:
        return all(getattr(diagnostic, "code", "") in _UNAVAILABLE_WGPU_CODES for diagnostic in diagnostics)
    message = str(error).lower()
    return "no compatible adapter" in message or "adapter request returned no compatible adapter" in message


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


def test_image_masks_reuse_normal_image_source_and_lower_modes() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    alpha = layer.masks.add(Image("alpha.png"), mode=ImageMaskMode.ALPHA, id="alpha")
    luma = layer.masks.add(Image("luma.png"), mode="luma", id="luma")
    masks = project.snapshot().to_dict()["visual"]["clips"][0]["masks"]
    assert masks[0]["input"] == {"type": "image", "asset": "image-000001", "mode": "alpha"}
    assert masks[1]["input"] == {"type": "image", "asset": "image-000002", "mode": "luma"}
    assert alpha.input.path == "alpha.png"
    assert luma.input.path == "luma.png"
    with pytest.raises(TypeError):
        layer.masks.add(Rectangle(width=4, height=4, fill="#ffffff"), mode=ImageMaskMode.ALPHA)
    with pytest.raises(ValueError):
        layer.masks.add(Image("bad.png"), mode="threshold")


def test_mask_input_annotation_includes_shape_and_image() -> None:
    annotation = get_type_hints(type(layer_mask_probe()).input.fget)["return"]
    assert annotation == Shape | Image


def layer_mask_probe() -> object:
    project = Project(size=(2, 2), fps=1, duration=1)
    return project.root.add(Rectangle(width=2, height=2, fill="#ff0000")).masks.add(
        Circle(radius=1, fill="#ffffff")
    )


@pytest.mark.parametrize(
    ("image", "message"),
    [
        (Image("mask.png", sizing=Sizing.fit()), "sizing"),
        (Image("mask.png", crop=Crop(0.25, 0, 0.5, 1)), "crop"),
    ],
)
def test_image_mask_rejects_unsupported_sizing_and_crop(image: Image, message: str) -> None:
    project = Project(size=(4, 4), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=4, height=4, fill="#ff0000"))
    with pytest.raises(ValueError, match=message):
        layer.masks.add(image)


@pytest.mark.parametrize(
    ("mode", "pixels", "expected"),
    [
        (ImageMaskMode.ALPHA, [(255, 0, 0, 0), (255, 0, 0, 64), (255, 0, 0, 128), (255, 0, 0, 255)], [0, 64, 128, 255]),
        (ImageMaskMode.LUMA, [(0, 0, 0, 255), (255, 255, 255, 255), (128, 128, 128, 255), (255, 0, 0, 255)], [0, 255, 128, 54]),
    ],
)
def test_image_masks_produce_rendered_pixel_coverage(tmp_path: Path, mode: ImageMaskMode, pixels: list[tuple[int, int, int, int]], expected: list[int]) -> None:
    image_path = tmp_path / "mask.png"
    _write_rgba_png(image_path, 2, 2, pixels)
    project = Project(size=(2, 2), fps=1, duration=1, base_directory=tmp_path)
    layer = project.root.add(Rectangle(width=2, height=2, fill="#ff0000"))
    layer.masks.add(Image(image_path), mode=mode, operation=MaskOperation.REPLACE)

    rendered = project.render_frame(0, backend="cpu").to_bytes()
    assert [rendered[index * 4] for index in range(4)] == expected
    assert all(rendered[index * 4 + 1 : index * 4 + 3] == bytes((0, 0)) for index in range(4))


def test_image_luma_rendering_covers_primary_colours_and_transparent_white(tmp_path: Path) -> None:
    pixels = [
        (0, 0, 0, 255),
        (255, 255, 255, 255),
        (128, 128, 128, 255),
        (255, 0, 0, 255),
        (0, 255, 0, 255),
        (0, 0, 255, 255),
        (255, 255, 255, 0),
        (255, 255, 255, 255),
    ]
    image_path = tmp_path / "luma.png"
    _write_rgba_png(image_path, 4, 2, pixels)
    project = Project(size=(4, 2), fps=1, duration=1, base_directory=tmp_path)
    layer = project.root.add(Rectangle(width=4, height=2, fill="#ff0000"))
    layer.masks.add(Image(image_path), mode=ImageMaskMode.LUMA, operation=MaskOperation.REPLACE)

    rendered = project.render_frame(0, backend="cpu").to_bytes()
    coverage = [rendered[index * 4] for index in range(8)]
    assert coverage[0] == 0
    assert coverage[1] == 255
    assert 126 <= coverage[2] <= 130
    assert 53 <= coverage[3] <= 55
    assert coverage[4] > coverage[3] > coverage[5]
    assert coverage[6] == 0
    assert coverage[7] == 255


def test_mixed_shape_and_image_masks_preserve_order(tmp_path: Path) -> None:
    image_path = tmp_path / "mask.png"
    _write_rgba_png(image_path, 2, 2, [(255, 255, 255, 255), (255, 255, 255, 0)] * 2)
    project = Project(size=(2, 2), fps=1, duration=1, base_directory=tmp_path)
    layer = project.root.add(Rectangle(width=2, height=2, fill="#ff0000"))
    layer.masks.add(Image(image_path), mode=ImageMaskMode.ALPHA, operation=MaskOperation.REPLACE)
    layer.masks.add(Rectangle(width=2, height=2, fill="#ffffff"), operation=MaskOperation.INTERSECT)

    rendered = project.render_frame(0, backend="cpu").to_bytes()
    assert rendered[:3] == bytes((255, 0, 0))
    assert rendered[4:7] == bytes((0, 0, 0))


def test_group_image_mask_clips_composed_result_and_keeps_child_masks_independent(tmp_path: Path) -> None:
    image_path = tmp_path / "group-mask.png"
    _write_rgba_png(image_path, 2, 2, [(255, 255, 255, 255)] * 4)
    project = Project(size=(4, 4), fps=1, duration=1, base_directory=tmp_path)
    group = project.root.group(duration=1)
    child = group.add(Rectangle(width=4, height=4, fill="#00ff00"), duration=1)
    child.masks.add(Rectangle(width=2, height=4, fill="#ffffff"))
    group.masks.add(Image(image_path), mode=ImageMaskMode.ALPHA, operation=MaskOperation.REPLACE)

    rendered = project.render_frame(0, backend="cpu").to_bytes()
    assert rendered[(2 * 4 + 1) * 4 : (2 * 4 + 1) * 4 + 3] == bytes((0, 255, 0))
    assert rendered[(0 * 4 + 0) * 4 : (0 * 4 + 0) * 4 + 3] == bytes((0, 0, 0))
    assert rendered[(2 * 4 + 3) * 4 : (2 * 4 + 3) * 4 + 3] == bytes((0, 0, 0))


def test_one_image_asset_is_registered_once_for_visible_and_mask_uses(tmp_path: Path) -> None:
    image_path = tmp_path / "shared.png"
    _write_rgba_png(image_path, 1, 1, [(255, 0, 0, 255)])
    project = Project(size=(2, 2), fps=1, duration=1, base_directory=tmp_path)
    image = Image(image_path)
    layer = project.root.add(image)
    layer.masks.add(image, mode=ImageMaskMode.ALPHA)
    layer.masks.add(image, mode=ImageMaskMode.LUMA)

    assets = project.snapshot().to_dict()["assets"]
    assert len(assets) == 1


def test_image_masks_render_on_cpu_and_wgpu() -> None:
    project = Project(size=(32, 32), fps=1, duration=1, base_directory=ROOT)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    layer.masks.add(
        Image("examples/assets/green.png"),
        mode=ImageMaskMode.ALPHA,
        feather=4,
    )
    layer.masks.add(
        Image("examples/assets/blue.png"),
        mode=ImageMaskMode.LUMA,
        operation=MaskOperation.UNION,
    )
    cpu = _render(project, "cpu")
    assert len(cpu) == 32 * 32 * 4
    gpu = _render(project, "wgpu")
    assert len(gpu) == len(cpu)


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


def test_uniform_mask_scale_binding_lowers_to_both_components() -> None:
    project = Project(size=(32, 32), fps=1, duration=1)
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    mask = layer.masks.add(Ellipse(width=20, height=20, fill="#ffffff"))
    mask.transform.scale.bind(
        project.audio.signal.rms().remap(input=(0, 1), output=(0.2, 2.0)),
        operation="replace",
    )

    canonical = project.snapshot().to_dict()["visual"]["clips"][0]["masks"][0]
    scale = canonical["transform"]["scale"]
    modifiers = canonical["transform"]["component_modifiers"]
    assert "bindings" not in scale
    assert modifiers["scale_x"] == modifiers["scale_y"]
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
        if backend == "wgpu" and os.environ.get("VESTRA_REQUIRE_WGPU") != "1" and _wgpu_environment_unavailable(error):
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
        mask.feather.value = 7.75
        return project.render_frame(0, backend=backend).to_bytes()

    cpu = render("cpu")
    try:
        wgpu = render("wgpu")
    except Exception as error:
        if os.environ.get("VESTRA_REQUIRE_WGPU") != "1" and _wgpu_environment_unavailable(error):
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
        if backend == "wgpu" and os.environ.get("VESTRA_REQUIRE_WGPU") != "1" and _wgpu_environment_unavailable(error):
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


def test_dynamic_mask_properties_change_rendered_output() -> None:
    def project_with_mask() -> tuple[Project, object]:
        project = Project(size=(32, 32), fps=2, duration=1)
        layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
        return project, layer

    project, layer = project_with_mask()
    position = layer.masks.add(Rectangle(width=8, height=32, fill="#ffffff"))
    position.transform.position.keyframe(0, (0.25, 0.5))
    position.transform.position.keyframe(0.5, (0.75, 0.5))
    assert project.render_frame(0, backend="cpu").to_bytes() != project.render_frame(
        0.5, backend="cpu"
    ).to_bytes()

    project, layer = project_with_mask()
    scale = layer.masks.add(Ellipse(width=8, height=8, fill="#ffffff"))
    scale.transform.scale.keyframe(0, (0.5, 0.5))
    scale.transform.scale.keyframe(0.5, (2.0, 2.0))
    assert project.render_frame(0, backend="cpu").to_bytes() != project.render_frame(
        0.5, backend="cpu"
    ).to_bytes()

    project, layer = project_with_mask()
    rotation = layer.masks.add(Rectangle(width=20, height=4, fill="#ffffff"))
    rotation.transform.rotation_degrees.keyframe(0, 0)
    rotation.transform.rotation_degrees.keyframe(0.5, 45)
    assert project.render_frame(0, backend="cpu").to_bytes() != project.render_frame(
        0.5, backend="cpu"
    ).to_bytes()

    project, layer = project_with_mask()
    strength = layer.masks.add(Rectangle(width=8, height=8, fill="#ffffff"))
    strength.operation = MaskOperation.REPLACE
    strength.strength.keyframe(0, 0)
    strength.strength.keyframe(0.5, 1)
    assert project.render_frame(0, backend="cpu").to_bytes() != project.render_frame(
        0.5, backend="cpu"
    ).to_bytes()

    project, layer = project_with_mask()
    feather = layer.masks.add(Rectangle(width=12, height=12, fill="#ffffff"))
    feather.feather.keyframe(0, 0)
    feather.feather.keyframe(0.5, 8)
    assert project.render_frame(0, backend="cpu").to_bytes() != project.render_frame(
        0.5, backend="cpu"
    ).to_bytes()


def test_mask_uniform_scale_signal_is_evaluated_at_render_time() -> None:
    project = Project(size=(32, 32), fps=4, duration=1, base_directory=".")
    layer = project.root.add(Rectangle(width=32, height=32, fill="#ff0000"))
    mask = layer.masks.add(Ellipse(width=20, height=20, fill="#ffffff"))
    mask.transform.scale.bind(
        project.audio.signal.rms().remap(input=(0, 1), output=(0.2, 2.0)),
        operation="replace",
    )
    clip = project.audio.track("tone").add("examples/assets/tone.wav", trim_end=1)
    clip.set_gain_automation([
        AudioGainKeyframe(0, 0),
        AudioGainKeyframe(0.5, 1),
    ])
    quiet = project.render_frame(0, backend="cpu").to_bytes()
    loud = project.render_frame(0.75, backend="cpu").to_bytes()
    assert quiet != loud
