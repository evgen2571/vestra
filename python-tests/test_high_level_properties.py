from __future__ import annotations

import pytest

import vestra
from vestra.authoring.values import BlendMode, CubicBezier, Crop, Interpolation, Point
from vestra.lowering import Placement
from vestra.sources import Color, Image, ParticleSystem, Spectrum2D


def project(duration: float = 2.0) -> vestra.Project:
    return vestra.Project(size=(4, 4), fps=4, duration=duration)


def test_property_handles_and_tuple_assignments_are_mutable_and_atomic() -> None:
    layer = project().root.add(Color("#ffffff"))
    opacity = layer.opacity
    transform = layer.transform
    layer.opacity = 0.25
    layer.transform.position = (0.2, 0.3)
    layer.transform.anchor = (0.4, 0.6)
    layer.transform.scale = (1.5, 2.0)
    assert layer.opacity is opacity and layer.opacity.value == 0.25
    assert layer.transform is transform
    assert layer.transform.position.value == Point(0.2, 0.3)
    assert layer.transform.anchor.value == Point(0.4, 0.6)
    assert layer.transform.scale.value == Point(1.5, 2.0)
    with pytest.raises((TypeError, ValueError)):
        layer.opacity = float("nan")
    with pytest.raises((TypeError, ValueError)):
        layer.transform.scale = (1.0, 0.0)
    with pytest.raises((TypeError, ValueError)):
        layer.transform.anchor = (1.1, 0.5)
    assert layer.opacity.value == 0.25
    assert layer.transform.scale.value == Point(1.5, 2.0)
    assert layer.transform.anchor.value == Point(0.4, 0.6)


def test_opacity_keyframes_lower_to_canonical_track() -> None:
    layer = project().root.add(Color("#ffffff"))
    layer.opacity = 0.2
    layer.opacity.keyframe(0, 0.0, interpolation=Interpolation.HOLD)
    layer.opacity.keyframe(1, 1.0, interpolation=CubicBezier(0.2, 0, 0.8, 1))
    opacity = project_data(layer)["opacity"]
    assert opacity == {
        "base_value": 0.2,
        "keyframes": [
            {"time": 0.0, "value": 0.0, "interpolation": "hold"},
            {
                "time": 1.0,
                "value": 1.0,
                "interpolation": {
                    "type": "cubic_bezier",
                    "x1": 0.2,
                    "y1": 0.0,
                    "x2": 0.8,
                    "y2": 1.0,
                },
            },
        ],
    }


def project_data(layer: vestra.Layer) -> dict[str, object]:
    return layer.composition.project.snapshot().to_dict()["visual"]["clips"][0]  # type: ignore[return-value]


def test_image_transform_and_crop_are_native_without_wrapper() -> None:
    image = Image("frame.png", crop=Crop(0, 0, 1, 1))
    image.crop.keyframe(1, Crop(0.25, 0, 0.75, 1))
    layer = project().root.add(image, id="image")
    layer.transform.position = (0.2, 0.3)
    clip = project_data(layer)
    assert clip["id"] == "image"
    assert clip["source"]["type"] == "image"  # type: ignore[index]
    assert "clips" not in clip["source"]  # type: ignore[operator]
    assert clip["crop"]["keyframes"][0]["value"]["x"] == 0.25  # type: ignore[index]
    assert clip["transform"]["position"]["base_value"] == {"x": 0.2, "y": 0.3}  # type: ignore[index]


def test_composition_layer_transform_and_local_timing_lower_directly() -> None:
    p = project()
    group = p.root.group(start=0.5, duration=1, z=3, id="group")
    group.transform.position = (0.2, 0.3)
    group.add(Color("#ff0000"), start=0.25, duration=0.5, id="child")
    clip = p.snapshot().to_dict()["visual"]["clips"][0]
    assert clip["id"] == "group" and clip["layer"] == 3
    assert clip["transform"]["position"]["base_value"] == {"x": 0.2, "y": 0.3}  # type: ignore[index]
    assert clip["source"]["clips"][0]["start"] == 0.25  # type: ignore[index]


@pytest.mark.parametrize("source", [Color("#ffffff"), ParticleSystem(), Spectrum2D()])
def test_unsupported_transform_uses_neutral_inner_clip(source: object) -> None:
    p = project()
    layer = p.root.add(
        source, start=0.5, duration=1, z=4, visible=False, opacity=0.4, id="source"
    )  # type: ignore[arg-type]
    data = project_data(layer)
    assert data["source"]["type"] != "group"  # type: ignore[index]
    layer.transform.position = (0.2, 0.3)
    data = project_data(layer)
    assert data["source"]["type"] == "group"  # type: ignore[index]
    inner = data["source"]["clips"][0]  # type: ignore[index]
    assert data["id"] == "source" and data["start"] == 0.5 and data["layer"] == 4
    assert data["visible"] is False and data["opacity"]["base_value"] == 0.4  # type: ignore[index]
    assert inner["id"] == "source" and inner["blend_mode"] == "normal"
    assert inner["start"] == 0 and inner["duration"] == 1 and inner["layer"] == 0
    assert inner["visible"] is True and inner["opacity"]["base_value"] == 1.0  # type: ignore[index]


def test_default_transform_never_wraps_and_presentation_fields_lower() -> None:
    p = project()
    layer = p.root.add(Color("#ffffff"), z=-2, visible=False, opacity=0.4, id="plain")
    layer.blend_mode = BlendMode.MULTIPLY
    data = project_data(layer)
    assert data["source"]["type"] == "solid_color" and data["layer"] == -2  # type: ignore[index]
    assert data["visible"] is False and data["blend_mode"] == "multiply"


def test_registered_source_handlers_receive_explicit_placement_policy() -> None:
    layer = project().root.add(
        Color("#ffffff"), start=0.75, duration=1.25, z=3, opacity=0.4
    )
    actual = Placement.from_layer(layer)
    neutral = Placement.from_layer(layer, neutral=True)
    assert actual.start == 0.75 and actual.duration == 1.25 and actual.layer == 3
    assert actual.opacity == 0.4 and actual.visible is True
    assert neutral == Placement(0.0, 1.25, 0, True, 1.0, BlendMode.NORMAL)


def test_nested_transform_ids_are_scoped() -> None:
    p = project()
    group = p.root.group(id="outer")
    child = group.group(id="inner")
    layer = child.add(Color("#ffffff"), id="same")
    layer.transform.scale = 1.25
    data = p.snapshot().to_dict()["visual"]["clips"][0]
    assert data["source"]["clips"][0]["id"] == "outer/inner"  # type: ignore[index]
    assert data["source"]["clips"][0]["source"]["clips"][0]["id"] == "outer/inner/same"  # type: ignore[index]


def test_cpu_opacity_keyframes_change_frame() -> None:
    p = project(1)
    layer = p.root.add(Color("#ff0000"))
    layer.opacity.keyframe(0, 0)
    layer.opacity.keyframe(0.5, 1)
    assert p.render_frame(0, backend="cpu").to_bytes()[:4] == bytes((0, 0, 0, 255))
    assert p.render_frame(0.75, backend="cpu").to_bytes()[:4] == bytes((255, 0, 0, 255))


def test_cpu_animated_transform_executes_through_capability_adapter() -> None:
    p = project(1)
    layer = p.root.add(Color("#ff0000"), id="animated")
    layer.transform.position.keyframe(0, (0.5, 0.5))
    layer.transform.position.keyframe(0.5, (1.0, 0.5))

    clip = project_data(layer)
    assert clip["source"]["type"] == "group"  # type: ignore[index]
    assert p.render_frame(0, backend="cpu").to_bytes()[:4] == bytes((255, 0, 0, 255))
    assert p.render_frame(0.75, backend="cpu").to_bytes()[:4] == bytes((0, 0, 0, 255))
