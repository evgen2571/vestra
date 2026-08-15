"""Minimal recursive Group authoring coverage."""

import os
import pytest

import vestra
from vestra import Crossfade, FrameRate, ProjectSnapshot
from vestra.authoring import (
    BlendMode,
    GroupClip,
    ImageAsset,
    ImageClip,
    AuthoringError,
    Point,
    ParticleSystemClip,
    ProjectBuilder,
    Spectrum2DClip,
    sparks,
)


def builder() -> ProjectBuilder:
    return ProjectBuilder(
        width=32, height=24, frame_rate=FrameRate(24, 1), output_path="group.mp4", duration=3
    )


def test_group_serializes_owned_nested_tree_and_parent_presentation() -> None:
    authored = builder()
    child = authored.add_solid_color_clip(colour="#ff0000", start=0.25, duration=0.5, layer=0, id="local")
    nested = authored.add_group_clip(clips=[child], start=0.0, duration=1.0, layer=0, id="nested")
    outer = authored.add_group_clip(clips=[nested], start=1.0, duration=1.0, layer=1, id="outer")
    outer.opacity.base_value = 0.8
    outer.blend_mode = BlendMode.MULTIPLY
    outer.transform.position.base_value = Point(0.6, 0.5)
    outer.effects.add_brightness(amount=0.1)

    data = authored.to_dict()
    root = data["visual"]["clips"][0]  # type: ignore[index]
    assert isinstance(root, dict)
    assert root["source"]["type"] == "group"
    assert root["source"]["clips"][0]["source"]["type"] == "group"
    assert root["source"]["clips"][0]["source"]["clips"][0]["start"] == 0.25
    assert root["opacity"]["base_value"] == 0.8
    assert root["blend_mode"] == "multiply"
    assert root["effects"][0]["type"] == "brightness"

    native = authored.build()
    assert ProjectSnapshot.from_dict(native.to_dict()).to_dict() == native.to_dict()


def test_group_accepts_particle_and_spectrum_children() -> None:
    authored = builder()
    particle = authored.add_particle_system_clip(
        particle_system=sparks(seed=7, count=2), start=0, duration=1, layer=0
    )
    spectrum = authored.add_spectrum2d_clip(start=0, duration=1, layer=1)
    group = authored.add_group_clip(clips=[particle, spectrum], start=0, duration=1, layer=0)
    assert isinstance(group.clips[0], ParticleSystemClip)
    assert isinstance(group.clips[1], Spectrum2DClip)
    source = group.to_canonical()["source"]
    assert [child["source"]["type"] for child in source["clips"]] == ["particle_system", "spectrum2d"]  # type: ignore[index]


def test_python_authored_nested_group_reaches_cpu_render() -> None:
    authored = builder()
    leaf = authored.add_solid_color_clip(
        colour="#ff0000", start=0, duration=2, layer=0,
    )
    nested = authored.add_group_clip(clips=[leaf], start=0, duration=2, layer=0)
    authored.add_group_clip(clips=[nested], start=0, duration=2, layer=0)
    prepared = vestra.Editor().prepare(
        authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    pixels = prepared.render_frame_number(0).to_bytes()
    assert any(pixels[index:index + 4] == bytes((255, 0, 0, 255)) for index in range(0, len(pixels), 4))


def test_python_authored_group_reaches_wgpu_when_adapter_is_available() -> None:
    authored = builder()
    leaf = authored.add_solid_color_clip(
        colour="#ff0000", start=0, duration=2, layer=0,
    )
    authored.add_group_clip(clips=[leaf], start=0, duration=2, layer=0)
    try:
        prepared = vestra.Editor().prepare(
            authored.build(), vestra.PrepareOptions(backend=vestra.BackendPreference.WGPU),
        )
    except vestra.PreparationError as error:
        unavailable = {"WGPU-ADAPTER-NOT-FOUND", "WGPU-NO-COMPATIBLE-ADAPTER"}
        if os.environ.get("VESTRA_REQUIRE_WGPU") == "1" or any(
            diagnostic.code not in unavailable for diagnostic in error.diagnostics
        ):
            raise
        pytest.skip("no compatible WGPU adapter")
    assert prepared.preparation_report.adapter is not None
    assert prepared.preparation_report.selected_backend is vestra.BackendKind.WGPU
    assert len(prepared.render_frame_number(0).to_bytes()) == 32 * 24 * 4


def test_nested_image_asset_is_serialized_once() -> None:
    authored = builder()
    asset = authored.add_image_asset("tests/assets/wgpu-small-rgba.png", id="nested-image")
    image = authored.add_image_clip(source=asset, start=0, duration=1, layer=0)
    group = authored.add_group_clip(clips=[image], start=0, duration=1, layer=0)
    data = authored.to_dict()
    assert data["assets"] == [{"id": "nested-image", "type": "image", "source": "tests/assets/wgpu-small-rgba.png"}]
    assert group.to_canonical()["source"]["clips"][0]["source"] == {"type": "image", "asset": "nested-image"}  # type: ignore[index]


def test_group_child_ids_are_local_to_each_group() -> None:
    authored = builder()
    first_child = authored.add_solid_color_clip(colour="#ff0000", start=0, duration=1, layer=0, id="child")
    first = authored.add_group_clip(clips=[first_child], start=0, duration=1, layer=0)
    second_child = authored.add_solid_color_clip(colour="#00ff00", start=0, duration=1, layer=0, id="child")
    second = authored.add_group_clip(clips=[second_child], start=1, duration=1, layer=1)
    assert first.clips[0].id == second.clips[0].id == "child"


def test_group_can_reuse_id_released_by_newly_nested_child() -> None:
    authored = builder()
    child = authored.add_solid_color_clip(
        colour="#ff0000", start=0, duration=1, layer=0, id="x",
    )

    group = authored.add_group_clip(clips=[child], start=0, duration=1, layer=0, id="x")

    assert group.id == "x"
    assert group.clips[0].id == "x"
    assert authored.build()


def test_empty_group_serializes_and_builds() -> None:
    authored = builder()

    group = authored.add_group_clip(clips=[], start=0, duration=1, layer=0, id="empty")

    assert group.to_canonical()["source"] == {"type": "group", "clips": []}
    assert authored.build()


@pytest.mark.parametrize("endpoint", ["outgoing", "incoming"])
def test_grouping_transition_endpoint_is_rejected_without_mutation(endpoint: str) -> None:
    authored = builder()
    asset = authored.add_image_asset("tests/assets/wgpu-small-rgba.png")
    outgoing = authored.add_image_clip(source=asset, start=0, duration=2, layer=0, id="out")
    incoming = authored.add_image_clip(source=asset, start=0, duration=2, layer=1, id="in")
    transition = authored.transitions.add_transition(
        outgoing=outgoing, incoming=incoming, start=0.5, duration=1, id="transition", definition=Crossfade().to_canonical(),
    )
    before = authored.to_dict()

    endpoint_clip = outgoing if endpoint == "outgoing" else incoming
    with pytest.raises(AuthoringError, match="transition endpoints"):
        authored.add_group_clip(clips=[endpoint_clip], start=0, duration=2, layer=0, id="group")

    assert authored.to_dict() == before
    assert authored.clips == (outgoing, incoming)
    assert authored.transitions.items == (transition,)


@pytest.mark.parametrize("outgoing_kind,incoming_kind", [("image", "image"), ("image", "group"), ("group", "image"), ("group", "group")])
def test_group_is_a_supported_top_level_transition_endpoint(outgoing_kind: str, incoming_kind: str) -> None:
    authored = builder()
    asset: ImageAsset = authored.add_image_asset("tests/assets/wgpu-small-rgba.png")

    def make(kind: str, layer: int) -> ImageClip | GroupClip:
        if kind == "image":
            return authored.add_image_clip(source=asset, start=0, duration=2, layer=layer)
        child = authored.add_solid_color_clip(colour="#ffffff", start=0, duration=2, layer=0)
        return authored.add_group_clip(clips=[child], start=0, duration=2, layer=layer)

    outgoing = make(outgoing_kind, 0)
    incoming = make(incoming_kind, 1)
    transition = authored.transitions.add_transition(outgoing=outgoing, incoming=incoming, start=0.5, duration=1, definition=Crossfade().to_canonical())
    assert transition.to_canonical()["outgoing"] == outgoing.id
    assert transition.to_canonical()["incoming"] == incoming.id


def test_nested_group_children_are_not_transition_endpoints() -> None:
    authored = builder()
    asset = authored.add_image_asset("tests/assets/wgpu-small-rgba.png")
    child = authored.add_image_clip(source=asset, start=0, duration=2, layer=0)
    authored.add_group_clip(clips=[child], start=0, duration=2, layer=0)
    other = authored.add_image_clip(source=asset, start=0, duration=2, layer=1)

    with pytest.raises(AuthoringError, match="nested Group children"):
        authored.transitions.add_transition(
            outgoing=child, incoming=other, start=0, duration=1, definition=Crossfade().to_canonical(),
        )
