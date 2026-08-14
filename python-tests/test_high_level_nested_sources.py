from pathlib import Path
from typing import Any

import pytest

import vestra
from vestra.authoring import Color as AuthoringColor
from vestra.sources import (
    Color,
    Crop,
    Image,
    ParticleAudioReactive,
    ParticleSystem,
    PointEmitter,
    Spectrum2D,
    Spectrum2DLinearLayout,
)


def _project() -> vestra.Project:
    return vestra.Project(size=(2, 2), fps=1, duration=2)


def test_nested_compositions_lower_in_authored_order_and_keep_local_timing() -> None:
    project = _project()
    project.root.add(Color("#ff0000"), id="before")
    group = project.root.group("visualizer", start=10, duration=20, id="group")
    group.add(Color("#00ff00"), start=2, duration=1, id="same")
    group.group("inner", duration=1, id="inner").add(Color("#0000ff"), id="same")
    project.root.add(Color("#ffffff"), id="after")

    clips = project.snapshot().to_dict()["visual"]["clips"]
    assert [clip["id"] for clip in clips] == ["before", "group", "after"]
    assert clips[1]["start"] == 10
    assert [clip["id"] for clip in clips[1]["source"]["clips"]] == [
        "group/same",
        "group/inner",
    ]
    assert clips[1]["source"]["clips"][0]["start"] == 2
    assert (
        clips[1]["source"]["clips"][1]["source"]["clips"][0]["id"] == "group/inner/same"
    )


def test_child_omitted_duration_inherits_its_parent_group_span() -> None:
    project = _project()
    group = project.root.group(duration=0.75)
    group.add(Color("#ff0000"))
    assert (
        project.snapshot().to_dict()["visual"]["clips"][0]["source"]["clips"][0][
            "duration"
        ]
        == 0.75
    )


def test_nested_duplicate_local_ids_are_allowed_but_same_scope_is_atomic() -> None:
    project = _project()
    project.root.add(Color("#fff000"), id="same")
    group = project.root.group(id="child")
    group.add(Color("#000000"), id="same")
    before = project.snapshot().to_dict()
    with pytest.raises(ValueError, match="duplicate"):
        group.add(Color("#ffffff"), id="same")
    assert project.snapshot().to_dict() == before


def test_group_failure_is_atomic_and_scope_components_are_escaped() -> None:
    project = _project()
    with pytest.raises(ValueError):
        project.root.group(id="broken", duration=0)
    assert project.root.layers == ()
    first = project.root.group(id="a/b")
    second = project.root.group(id="a~1")
    first.add(Color("#000000"), id="c")
    second.add(Color("#ffffff"), id="b/c")
    data = project.snapshot().to_dict()
    assert data["visual"]["clips"][0]["source"]["clips"][0]["id"] == "a~1b/c"
    assert data["visual"]["clips"][1]["source"]["clips"][0]["id"] == "a~01/b~1c"


def test_nested_id_avoids_collision_with_a_root_id() -> None:
    project = _project()
    project.root.group(id="a").add(Color("#000000"), id="b")
    project.root.add(Color("#ffffff"), id="a/b")
    clips = project.snapshot().to_dict()["visual"]["clips"]
    assert clips[0]["source"]["clips"][0]["id"] == "a/b~2"


def test_image_sizing_and_crop_are_source_properties() -> None:
    project = _project()
    project.root.add(Image("cover.png", sizing="cover", crop=Crop(0, 0, 0.5, 1)))
    source = project.snapshot().to_dict()["visual"]["clips"][0]
    assert source["sizing"] == {"mode": "cover"}
    assert source["crop"]["base_value"] == {
        "x": 0.0,
        "y": 0.0,
        "width": 0.5,
        "height": 1.0,
    }


def test_mutable_image_and_colour_fields_validate_atomically() -> None:
    image = Image("frame.png", sizing="fit")
    image.path = Path("replacement.png")
    image.sizing = "cover"
    assert (
        image.path == "replacement.png"
        and image.sizing == vestra.authoring.Sizing.cover()
    )
    with pytest.raises((TypeError, ValueError)):
        image.path = ""
    with pytest.raises((TypeError, ValueError)):
        image.sizing = "stretch"  # type: ignore[assignment]
    assert (
        image.path == "replacement.png"
        and image.sizing == vestra.authoring.Sizing.cover()
    )

    colour = Color("#ffffff")
    colour.value = "#112233"
    with pytest.raises((TypeError, ValueError)):
        colour.value = "invalid"
    assert colour.value == "#112233"


def test_particle_template_is_copied_for_each_placement() -> None:
    project = _project()
    template = ParticleSystem(emitter=PointEmitter(), rate=1)
    first = project.root.add(template, id="first")
    second = project.root.add(template, id="second")
    template.definition.rate = 99
    assert first.source.definition.rate == 1
    assert second.source.definition.rate == 1
    assert first.source.definition is not second.source.definition


def test_particle_source_lowers_canonical_fields_and_reports_audio_capability() -> None:
    from vestra.lowering import source_capabilities

    source = ParticleSystem(rate=3, seed=7)
    project = _project()
    project.root.add(source)
    particle = project.snapshot().to_dict()["visual"]["clips"][0]["source"]
    assert particle["type"] == "particle_system"
    assert particle["seed"] == 7
    assert particle["emission"]["rate"] == 3.0
    assert not source_capabilities(source).requires_audio
    reactive = ParticleSystem(audio_reactive=ParticleAudioReactive())
    assert source_capabilities(reactive).requires_audio
    source.rate = 4
    assert source.rate == 4
    with pytest.raises(ValueError):
        source.rate = -1
    assert source.rate == 4


def test_nested_image_assets_are_deduplicated_and_snapshot_ids_are_stable() -> None:
    project = _project()
    project.root.group(id="one").add(Image("a/../frame.png"), id="frame")
    project.root.group(id="two").add(Image("frame.png"), id="frame")
    first = project.snapshot().to_dict()
    second = project.snapshot().to_dict()
    assert first == second
    assert len(first["assets"]) == 1


def test_nested_cpu_render_executes_group_clip() -> None:
    project = _project()
    project.root.group(duration=2).add(Color("#ff0000"), duration=2)
    assert project.render_frame(0, backend="cpu").to_bytes()[:4] == bytes(
        (255, 0, 0, 255)
    )


def test_spectrum_preset_keeps_effects_and_explicit_overrides() -> None:
    project = _project()
    project.root.add(
        Spectrum2D(preset="neon", band_count=7, layout=Spectrum2DLinearLayout("top"))
    )
    source = project.snapshot().to_dict()["visual"]["clips"][0]
    assert source["source"]["band_count"] == 7
    assert source["source"]["layout"] == {
        "type": "linear",
        "anchor": "top",
        "band_mapping": "forward",
    }
    assert len(source["effects"]) == 2
    assert not project.validate().is_valid

    cleared = _project()
    cleared.root.add(Spectrum2D(preset="neon_circle", gradient=None))
    assert (
        "gradient" not in cleared.snapshot().to_dict()["visual"]["clips"][0]["source"]
    )


def test_capability_query_reports_registered_source_facts() -> None:
    from vestra.lowering import SourceCapabilities, source_capabilities

    capabilities = source_capabilities(Image)
    assert isinstance(capabilities, SourceCapabilities)
    assert capabilities.supports_sizing and capabilities.supports_crop
    assert capabilities.supports_direct_transform
    assert capabilities.supports_direct_transition_endpoint
    assert not source_capabilities(ParticleSystem).supports_direct_transform
    assert not source_capabilities(ParticleSystem).supports_direct_transition_endpoint
    with pytest.raises(TypeError, match="not registered"):
        source_capabilities(type("Unknown", (vestra.Source,), {}))


def test_custom_source_can_extend_lowering_through_the_registry_only() -> None:
    from vestra.lowering import SourceCapabilities, register_source

    class RegisteredSolid(vestra.Source):
        def __init__(self, value: str) -> None:
            self.value = value

    def lower_registered_solid(
        context: Any, layer: Any, native_id: str, placement: Any
    ) -> Any:
        assert isinstance(layer.source, RegisteredSolid)
        return context.builder.add_solid_color_clip(
            colour=AuthoringColor(layer.source.value),
            start=placement.start,
            duration=placement.duration,
            layer=placement.layer,
            visible=placement.visible,
            opacity=placement.opacity,
            id=native_id,
        )

    register_source(
        RegisteredSolid,
        lower_registered_solid,
        SourceCapabilities(supports_transition_adapter=True),
    )
    project = _project()
    project.root.add(RegisteredSolid("#123456"), id="custom")

    clip = project.snapshot().to_dict()["visual"]["clips"][0]
    assert clip["id"] == "custom"
    assert clip["source"] == {"type": "solid_color", "colour": "#123456"}
