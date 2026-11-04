"""Standalone sources use the same canonical group and source lowering paths."""

import os
from pathlib import Path
from typing import Any, cast

import pytest

import vestra
from vestra.effects import Ascii, GaussianBlur
from vestra.sources import Color, Group, Image, Rectangle, Source, Text, Video
from test_masks import _write_rgba_png


pytest_plugins = ["test_high_level_video"]

FONT = "tests/assets/VestraTest-Regular.ttf"


def _project() -> vestra.Project:
    return vestra.Project(size=(16, 16), fps=4, duration=4)


def _pixel(frame: vestra.Frame, x: int = 8, y: int = 8) -> bytes:
    offset = (y * frame.width + x) * 4
    return frame.to_bytes()[offset:offset + 4]


def test_group_registration_nesting_ids_snapshots_and_round_trip() -> None:
    from vestra.lowering import source_capabilities

    card = Group()
    card.add(Group([Rectangle(width=8, height=8, fill="#ff0000")]))
    project = _project()
    first = project.root.add(card, id="one")
    second = project.root.group(id="direct").add(card, id="two")
    project.root.add(Color("#000000"), id="one/group-child-0")
    snapshot = project.snapshot()
    assert snapshot.to_dict() == project.snapshot().to_dict()
    data = snapshot.to_dict()
    outer = data["visual"]["clips"][0]
    assert outer["source"]["type"] == "group"
    inner = outer["source"]["clips"][0]
    assert inner["id"] == "one/group-child-0~2"
    assert inner["source"]["clips"][0]["id"] == "one/group-child-0/group-child-0"
    assert source_capabilities(Group).supports_direct_transform
    assert vestra.ProjectSnapshot.from_json(snapshot.to_json()).to_dict() == data
    assert project.validate().is_valid

    card.add(Color("#00ff00"))
    cast(Group, first.source).children[0].transform.rotation = 25
    assert len(cast(Group, second.source).children) == 1
    assert cast(Group, second.source).children[0].transform.rotation.value == 0
    assert snapshot.to_dict() == data


def test_group_children_keep_local_time_order_visibility_and_animation() -> None:
    group = Group()
    group.add(Color("#ff0000"), duration=2, layer=3)
    child = group.add(Color("#00ff00"), start=0.5, duration=0.5, layer=4)
    child.opacity.keyframe(0, 1)
    child.opacity.keyframe(0.5, 1)
    group.add(Color("#0000ff"), layer=2)
    group.add(Color("#ffffff"), layer=5, visible=False)
    project = _project()
    placement = project.root.add(group, start=1, duration=2)
    placement.opacity.keyframe(0, 1)
    placement.opacity.keyframe(2, 1)
    prepared = project.prepare(backend="cpu")
    for time, expected in [(1.75, (0, 255, 0, 255)), (1, (255, 0, 0, 255)),
                           (2, (255, 0, 0, 255)), (3, (0, 0, 0, 255))]:
        assert _pixel(prepared.render_frame_seconds(time)) == bytes(expected)
    children = project.snapshot().to_dict()["visual"]["clips"][0]["source"]["clips"]
    assert [child["layer"] for child in children] == [3, 4, 2, 5]
    assert children[1]["start"] == 0.5
    assert children[2]["duration"] == 2


def test_nested_media_fonts_effects_and_mask_assets_are_deduplicated() -> None:
    group = Group()
    group.add(Image("a/../frame.png", sizing="fit"))
    video = group.add(Video("clip.mp4"), source_start=0.25, playback_rate=2)
    cast(Video, video.source).crop.value = vestra.Crop(0, 0, 0.5, 1)
    text = group.add(Text("V", font=FONT, font_size=8))
    text.effects.add(Ascii(font=FONT))
    text.masks.add(Group([Image("frame.png"), Text("V", font=FONT, font_size=8)]))
    nested = Group([group])
    project = _project()
    project.root.add(nested, id="first")
    project.root.add(nested, id="second")
    project.root.add(Image("frame.png"))
    data = project.snapshot().to_dict()
    assert [asset["type"] for asset in data["assets"]] == ["image", "video", "font"]
    clips = data["visual"]["clips"][0]["source"]["clips"][0]["source"]["clips"]
    assert clips[0]["source"]["asset"] == "image-000001"
    assert clips[1]["source_start"] == 0.25
    assert clips[1]["playback_rate"] == 2
    assert clips[1]["crop"]["base_value"]["width"] == 0.5
    assert clips[2]["source"]["font"] == clips[2]["effects"][0]["font"] == "font-000001"
    assert project.validate().is_valid


def test_group_uses_registered_handlers_for_custom_sources() -> None:
    from vestra.lowering import register_source

    class Custom(Source):
        pass

    def lower(context: Any, layer: Any, identifier: str, placement: Any) -> Any:
        return context.builder.add_solid_color_clip(
            colour="#123456", id=identifier, start=placement.start,
            duration=placement.duration, layer=placement.layer,
        )

    register_source(Custom, lower)
    project = _project()
    project.root.add(Group([Group([Custom()])]))
    assert _pixel(project.render_frame(0, backend="cpu")) == bytes((18, 52, 86, 255))


def test_recursive_group_reports_a_useful_error() -> None:
    project = _project()
    layer = project.root.add(Group([Color("#ffffff")]))
    source = cast(Group, layer.source)
    source.children[0].source = source
    with pytest.raises(ValueError, match="recursive Group"):
        project.snapshot()


def test_group_placement_transforms_apply_once_and_animate_independently() -> None:
    source = Group(position=(0.5, 0.5))
    source.add(Rectangle(width=4, height=4, fill="#ff0000", position=(0.25, 0.5)))
    project = _project()
    project.root.add(source, duration=2)
    second = project.root.add(source, position=(0.75, 0.5), duration=2)
    second.transform.position.keyframe(0, (0.75, 0.5))
    second.transform.position.keyframe(1, (1, 0.5))
    prepared = project.prepare(backend="cpu")
    initial = prepared.render_frame_seconds(0)
    later = prepared.render_frame_seconds(1)
    assert _pixel(initial, 4) == _pixel(initial, 8) == bytes((255, 0, 0, 255))
    assert _pixel(initial, 12) == bytes((0, 0, 0, 255))
    assert _pixel(later, 4) == _pixel(later, 12) == bytes((255, 0, 0, 255))
    assert _pixel(later, 8) == bytes((0, 0, 0, 255))


def test_group_mask_compatibility_and_visual_child_effects() -> None:
    source = Group()
    child = source.add(Rectangle(width=8, height=8, fill="#ffffff"))
    child.effects.add(GaussianBlur(radius=1))
    child.masks.add(Rectangle(width=4, height=16, fill="#ffffff"))
    visual = _project()
    visual.root.add(source)
    direct = _project()
    direct_child = direct.root.group().add(Rectangle(width=8, height=8, fill="#ffffff"))
    direct_child.effects.add(GaussianBlur(radius=1))
    direct_child.masks.add(Rectangle(width=4, height=16, fill="#ffffff"))
    assert visual.render_frame(0, backend="cpu").to_bytes() == direct.render_frame(0, backend="cpu").to_bytes()
    masked = _project()
    masked.root.add(Color("#ffffff")).masks.add(Group([Rectangle(width=4, height=4, fill="#ffffff")]))
    frame = masked.render_frame(0, backend="cpu")
    assert _pixel(frame) == bytes((255, 255, 255, 255))
    assert _pixel(frame, 0, 0) == bytes((0, 0, 0, 255))


def test_nested_image_video_and_text_render_through_group(video_path: Path, tmp_path: Path) -> None:
    image_path = tmp_path / "image.png"
    _write_rgba_png(image_path, 2, 2, [(0, 255, 0, 255)] * 4)
    group = Group()
    group.add(Image(image_path, sizing="cover"), duration=0.5)
    group.add(Video(video_path, sizing="cover"), start=0.5, duration=0.25,
              source_start=0.5, playback_rate=2)
    group.add(Text("V", font=FONT, font_size=12), start=1, duration=1)
    project = _project()
    project.root.add(Group([group]), start=1, duration=2)
    prepared = project.prepare(backend="cpu")
    assert _pixel(prepared.render_frame_seconds(1)) == bytes((0, 255, 0, 255))
    red = _pixel(prepared.render_frame_seconds(1.5))
    assert red[0] >= 250 and red[1] <= 2 and red[2] <= 2
    text = prepared.render_frame_seconds(2).to_bytes()
    assert max(text[0::4]) > 0


@pytest.mark.parametrize("direct", [False, True])
def test_standalone_group_transition_endpoints_keep_composition_ownership(direct: bool) -> None:
    project = _project()
    composition = project.root.group().child if direct else project.root
    first = composition.add(Group([Color("#ff0000")]))
    second = composition.add(Group([Color("#0000ff")]))
    composition.transitions.add(first, second, vestra.Crossfade(), start=1, duration=1)
    data = project.snapshot().to_dict()["visual"]
    if direct:
        data = data["clips"][0]["source"]
    assert data["transitions"][0]["outgoing"] == data["clips"][0]["id"]
    assert project.validate().is_valid
    frame = project.render_frame(1.5, backend="cpu")
    assert _pixel(frame) == bytes((64, 0, 128, 255))


def test_standalone_group_wgpu_sequence_matches_cpu_when_available() -> None:
    project = _project()
    group = Group(position=(0.75, 0.5))
    group.add(Rectangle(width=4, height=4, fill="#ff0000", position=(0.25, 0.5)))
    child = group.add(Color("#00ff00"), start=1, duration=1)
    child.masks.add(Rectangle(width=4, height=4, fill="#ffffff"))
    child.effects.add(GaussianBlur(radius=0))
    layer = project.root.add(Group([group]))
    layer.transform.rotation.keyframe(0, 0)
    layer.transform.rotation.keyframe(2, 90)
    cpu = project.prepare(backend="cpu")
    try:
        wgpu = project.prepare(backend="wgpu")
    except vestra.PreparationError as error:
        unavailable = {"WGPU-ADAPTER-NOT-FOUND", "WGPU-NO-COMPATIBLE-ADAPTER"}
        if os.environ.get("VESTRA_REQUIRE_WGPU") == "1" or any(
            diagnostic.code not in unavailable for diagnostic in error.diagnostics
        ):
            raise
        pytest.skip("no compatible WGPU adapter")
    assert wgpu.preparation_report.adapter is not None
    for time in (1.5, 0, 1, 2, 0):
        expected = cpu.render_frame_seconds(time).to_bytes()
        actual = wgpu.render_frame_seconds(time).to_bytes()
        # Bilinear transforms may round edge coverage by one byte.
        assert max(abs(a - b) for a, b in zip(expected, actual)) <= 1


@pytest.mark.parametrize("group", [False, True])
def test_mask_placement_copies_source_defaults_once(group: bool) -> None:
    source: Source = Rectangle(width=4, height=4, fill="#ffffff")
    if group:
        source = Group([source])
    source.transform.position = (0.25, 0.5)
    source.transform.scale = 2
    project = _project()
    mask = project.root.add(Color("#ffffff")).masks.add(source)
    reference = _project()
    plain = Group([Rectangle(width=4, height=4, fill="#ffffff")]) if group else Rectangle(width=4, height=4, fill="#ffffff")
    explicit = reference.root.add(Color("#ffffff")).masks.add(plain)
    explicit.transform.position = (0.25, 0.5)
    explicit.transform.scale = 2
    assert project.snapshot().to_dict() == reference.snapshot().to_dict()
    assert project.render_frame(0, backend="cpu").to_bytes() == reference.render_frame(0, backend="cpu").to_bytes()
    source.transform.position = (0.75, 0.5)
    assert mask.transform.position.value == vestra.Point(0.25, 0.5)
