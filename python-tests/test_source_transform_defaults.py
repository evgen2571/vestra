from __future__ import annotations

from collections.abc import Callable
from typing import Any

import pytest

import vestra
from vestra.properties.lowering import lower_transform
from vestra.sources import (
    Circle, Color, Ellipse, Group, Image, Line, ParticleSystem, Polygon,
    Rectangle, Source, Spectrum2D, Text, Video,
)


def project() -> vestra.Project:
    return vestra.Project(size=(16, 16), fps=4, duration=2)


FACTORIES: list[Callable[..., Source]] = [
    lambda **kwargs: Rectangle(width=4, height=2, fill="#ffffff", **kwargs),
    lambda **kwargs: Ellipse(width=4, height=2, fill="#ffffff", **kwargs),
    lambda **kwargs: Circle(radius=2, fill="#ffffff", **kwargs),
    lambda **kwargs: Line(start=(0, 0), end=(2, 2), stroke="#ffffff", stroke_width=1, **kwargs),
    lambda **kwargs: Polygon(points=[(0, 0), (2, 0), (0, 2)], fill="#ffffff", **kwargs),
    lambda **kwargs: Text("V", font="font.ttf", font_size=4, **kwargs),
    lambda **kwargs: Image("image.png", **kwargs),
    lambda **kwargs: Video("video.mp4", **kwargs),
    lambda **kwargs: Group(**kwargs),
    lambda **kwargs: Color("#ffffff", **kwargs),
    lambda **kwargs: Spectrum2D(**kwargs),
    lambda **kwargs: ParticleSystem(**kwargs),
]


@pytest.mark.parametrize("factory", FACTORIES)
@pytest.mark.parametrize(
    ("argument", "value", "property_name", "expected"),
    [
        ("position", (0.25, 0.75), "position", vestra.Point(0.25, 0.75)),
        ("anchor", (0, 1), "anchor", vestra.Point(0, 1)),
        ("scale", (2, 3), "scale", vestra.Point(2, 3)),
        ("scale", 2, "scale", vestra.Point(2, 2)),
        ("rotation_degrees", -45, "rotation_degrees", -45),
    ],
)
def test_each_visual_source_inherits_individual_defaults(
    factory: Callable[..., Source], argument: str, value: Any,
    property_name: str, expected: Any,
) -> None:
    source = factory(**{argument: value})
    composition = project()
    layer = composition.root.add(source, duration=1)
    child = Group().add(source)
    assert getattr(source.transform, property_name).value == expected
    assert getattr(layer.transform, property_name).value == expected
    assert getattr(child.transform, property_name).value == expected
    assert layer.transform is not source.transform
    assert child.transform is not source.transform


@pytest.mark.parametrize("factory", FACTORIES[:-1])
def test_rotation_alias_is_consistent(factory: Callable[..., Source]) -> None:
    source = factory(rotation=15)
    assert source.transform.rotation.value == source.transform.rotation_degrees.value == 15
    with pytest.raises(TypeError, match="only one"):
        factory(rotation=15, rotation_degrees=15)


@pytest.mark.parametrize("factory", FACTORIES)
def test_existing_constructor_defaults_are_unchanged(factory: Callable[..., Source]) -> None:
    source = factory()
    assert source.transform.is_default()
    assert project().root.add(source, duration=1).transform.is_default()
    assert Group().add(source).transform.is_default()


def animated_transform() -> vestra.Transform:
    transform = vestra.Transform()
    signal = vestra.MasterAudioSignals().rms()
    transform.position = (0.3, 0.4)
    transform.anchor = (0.2, 0.6)
    transform.scale = (1.5, 2)
    transform.rotation = 30
    transform.position.keyframe(0.5, (0.6, 0.7))
    transform.anchor.keyframe(0.5, (0.4, 0.6))
    transform.scale.keyframe(0.5, (2, 3))
    transform.rotation.keyframe(0.5, 90)
    transform.scale.bind(signal)
    transform.scale.x.bind(signal)
    transform.scale.y.bind(signal, operation="replace")
    transform.rotation.bind(signal, operation="multiply")
    for component in (transform.position_x, transform.position_y, transform.scale_x, transform.scale_y):
        component.bind(signal)
    return transform


@pytest.mark.parametrize("factory", FACTORIES)
def test_preconfigured_transform_is_copied_by_constructor_and_snapshot(
    factory: Callable[..., Source],
) -> None:
    defaults = animated_transform()
    original = lower_transform(defaults)
    source = factory(transform=defaults)
    assert lower_transform(source.transform) == original
    defaults.rotation = 10
    defaults.position_x.clear_bindings()
    assert lower_transform(source.transform) == original
    copied = source.snapshot()
    source.transform.scale.clear_keyframes()
    source.transform.scale.x.clear_bindings()
    assert lower_transform(copied.transform) == original


@pytest.mark.parametrize("placement_kind", ["composition", "direct_group", "standalone_group"])
@pytest.mark.parametrize(
    ("overrides", "replaced"),
    [
        ({"position": (0.8, 0.5)}, "position"),
        ({"anchor": (0, 0)}, "anchor"),
        ({"scale": 3}, "scale"),
        ({"rotation": 15}, "rotation_degrees"),
        ({"rotation_degrees": -20}, "rotation_degrees"),
    ],
)
def test_individual_placement_override_replaces_only_that_property_state(
    placement_kind: str, overrides: dict[str, Any], replaced: str,
) -> None:
    source = Rectangle(width=4, height=2, fill="#ffffff", transform=animated_transform())
    before = lower_transform(source.transform)
    composition = project()
    container: Any = (
        composition.root if placement_kind == "composition" else
        composition.root.group() if placement_kind == "direct_group" else Group()
    )
    placement = container.add(source, **overrides)
    expected = source.transform.copy()
    if replaced == "position":
        expected.position.clear_keyframes()
        expected.position = overrides["position"]
        expected.position_x.clear_bindings()
        expected.position_y.clear_bindings()
    elif replaced == "anchor":
        expected.anchor.clear_keyframes()
        expected.anchor = overrides["anchor"]
    elif replaced == "scale":
        expected.scale.clear_keyframes()
        expected.scale = overrides["scale"]
        for target in (expected.scale, expected.scale.x, expected.scale.y, expected.scale_x, expected.scale_y):
            target.clear_bindings()
    else:
        expected.rotation.clear_keyframes()
        expected.rotation = next(iter(overrides.values()))
        expected.rotation.clear_bindings()
    assert lower_transform(placement.transform) == lower_transform(expected)
    assert lower_transform(source.transform) == before
    assert not getattr(placement.transform, replaced).keyframes


def test_placement_reuse_and_layer_transform_assignment_preserve_all_bindings() -> None:
    source = Image("image.png", transform=animated_transform())
    composition = project()
    first = composition.root.add(source)
    second = composition.root.add(source)
    expected = lower_transform(source.transform)
    assert lower_transform(first.transform) == lower_transform(second.transform) == expected
    first.transform.position_x.clear_bindings()
    first.transform.scale.x.clear_bindings()
    first.transform.rotation.clear_keyframes()
    assert lower_transform(second.transform) == expected
    assert lower_transform(source.transform) == expected

    handles = (first.transform, first.transform.position, first.transform.scale, first.transform.position_x)
    first.transform = source.transform
    assert handles == (first.transform, first.transform.position, first.transform.scale, first.transform.position_x)
    assert lower_transform(first.transform) == expected
    first.transform.scale_y.clear_bindings()
    assert lower_transform(source.transform) == expected


def test_full_transform_replacement_and_conflicts_are_explicit() -> None:
    source = Color("#ffffff", position=(0.1, 0.2), scale=3)
    replacement = animated_transform()
    composition = project()
    layer = composition.root.add(source, transform=replacement)
    assert lower_transform(layer.transform) == lower_transform(replacement)
    for container in (composition.root, Group()):
        before = tuple(container.layers if hasattr(container, "layers") else container.children)
        with pytest.raises(TypeError, match="cannot be combined"):
            container.add(source, transform=replacement, position=(0.1, 0.2))
        assert tuple(container.layers if hasattr(container, "layers") else container.children) == before
    with pytest.raises(TypeError, match="cannot be combined"):
        Color("#ffffff", transform=replacement, scale=2)


@pytest.mark.parametrize(
    "overrides",
    [
        {"position": (float("nan"), 0)}, {"position": True},
        {"anchor": (-0.1, 0.5)}, {"anchor": (0.5, 1.1)},
        {"scale": 0}, {"scale": (-1, 1)}, {"scale": True},
        {"rotation": float("inf")}, {"rotation_degrees": True},
        {"rotation": 10, "rotation_degrees": 20}, {"transform": 1},
    ],
)
def test_invalid_defaults_and_placement_overrides_leave_authoring_state_unchanged(
    overrides: dict[str, Any],
) -> None:
    with pytest.raises((TypeError, ValueError)):
        Rectangle(width=4, height=2, fill="#ffffff", **overrides)
    source = Rectangle(width=4, height=2, fill="#ffffff", transform=animated_transform())
    defaults = lower_transform(source.transform)
    composition = project()
    standalone = Group()
    for container in (composition.root, standalone):
        with pytest.raises((TypeError, ValueError)):
            container.add(source, **overrides)
    assert composition.root.layers == ()
    assert standalone.children == ()
    assert lower_transform(source.transform) == defaults
    assert composition.root.add(source, id="layer-000001").id == "layer-000001"


def test_source_transform_defaults_do_not_add_an_extra_source_transform_pass() -> None:
    source = Rectangle(width=4, height=2, fill="#ffffff", position=(0.25, 0.5), scale=2)
    composition = project()
    composition.root.add(source)
    clip = composition.snapshot().to_dict()["visual"]["clips"][0]
    assert clip["source"]["type"] == "shape"
    assert "transform" not in clip["source"]
    assert clip["transform"]["position"]["base_value"] == {"x": 0.25, "y": 0.5}
    assert clip["transform"]["scale"]["base_value"] == {"x": 2, "y": 2}
    group = Group([source], position=(0.75, 0.5))
    child_clip = group.to_canonical(inherited_duration=2)["clips"][0]
    assert "transform" not in child_clip["source"]
    assert child_clip["transform"]["position"]["base_value"] == {"x": 0.25, "y": 0.5}


def test_particle_orientation_keeps_its_existing_meaning() -> None:
    source = ParticleSystem(rotation=45, rotation_degrees=15)
    assert source.definition.rotation == 45
    assert source.transform.rotation.value == 15
    assert source.snapshot().transform.rotation.value == 15


def test_custom_sources_without_base_initialization_keep_default_presentation() -> None:
    class CustomSource(Source):
        def __init__(self) -> None:
            self.value = "custom"
    source = CustomSource()
    assert source.transform.is_default()
    assert project().root.add(source).transform.is_default()


def test_cpu_applies_constructor_scale_and_position_once_and_snapshot_is_isolated() -> None:
    composition = project()
    source = Rectangle(width=4, height=2, fill="#ffffff", position=(0.25, 0.5), scale=2)
    layer = composition.root.add(source)
    snapshot = composition.snapshot()
    pixels = composition.render_frame(0, backend="cpu").to_bytes()

    def pixel(x: int, y: int) -> bytes:
        offset = (y * 16 + x) * 4
        return pixels[offset:offset + 4]

    assert pixel(4, 8) == pixel(6, 8) == bytes((255, 255, 255, 255))
    assert pixel(9, 8) == bytes((0, 0, 0, 255))
    layer.transform.position = (0.75, 0.5)
    source.transform.scale = 3
    assert snapshot.to_dict()["visual"]["clips"][0]["transform"]["position"]["base_value"] == {"x": 0.25, "y": 0.5}
    assert snapshot.to_dict()["visual"]["clips"][0]["transform"]["scale"]["base_value"] == {"x": 2, "y": 2}


@pytest.mark.parametrize("placement", ["root", "group", "mask_group"])
def test_scale_point_component_bindings_survive_canonical_lowering(placement: str) -> None:
    composition = project()
    signal = composition.audio.signal.rms()
    defaults = vestra.Transform()
    defaults.scale.bind(signal, operation="multiply")
    defaults.scale.x.bind(signal, operation="replace")
    defaults.scale.y.bind(signal, operation="add")
    defaults.scale_x.bind(signal, operation="add")
    defaults.scale_y.bind(signal, operation="multiply")
    source = Rectangle(width=4, height=4, fill="#ffffff", transform=defaults)
    if placement == "root":
        composition.root.add(source)
    elif placement == "group":
        composition.root.add(Group([source]))
    else:
        composition.root.add(Color("#ffffff")).masks.add(Group([source]))
    clip = composition.snapshot().to_dict()["visual"]["clips"][0]
    if placement == "group":
        clip = clip["source"]["clips"][0]
    elif placement == "mask_group":
        clip = clip["masks"][0]["input"]["source"]["clips"][0]
    assert "component_modifiers" not in clip["transform"]["scale"]
    modifiers = clip["transform"]["component_modifiers"]
    assert [item["operation"] for item in modifiers["scale_x"]] == ["multiply", "replace", "add"]
    assert [item["operation"] for item in modifiers["scale_y"]] == ["multiply", "add", "multiply"]


def test_scale_point_component_bindings_require_a_presentation_adapter() -> None:
    composition = project()
    source = Color("#ffffff")
    source.transform.scale.x.bind(composition.audio.signal.rms())
    assert not source.transform.is_default()
    composition.root.add(source)
    clip = composition.snapshot().to_dict()["visual"]["clips"][0]
    assert clip["source"]["type"] == "group"
    assert clip["transform"]["component_modifiers"]["scale_x"]
