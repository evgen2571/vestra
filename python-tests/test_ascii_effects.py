"""ASCII controls, portable font resources and prepared random-access output."""
from pathlib import Path

import pytest

from vestra import Project, Ascii, PseudoAscii, AsciiMode, AsciiColorMode, FrameRate
from vestra.authoring import ProjectBuilder
from vestra.sources import Color


def test_ascii_defaults_custom_controls_and_copy_are_descriptor_backed():
    effect = Ascii(color_mode="source", background="#00000000")
    assert effect.characters == " .:-=+*#%@"
    assert effect.mode is AsciiMode.HYBRID
    assert effect.color_mode is AsciiColorMode.SOURCE
    assert effect.cell_width.value == 8
    assert PseudoAscii().to_canonical()["glyph_style"] == "geometric"
    project = Project(size=(26, 20), fps=10, duration=2)
    layer = project.root.add(Color("#8f7766"), duration=2)
    copied = layer.effects.add(effect)
    copied.cell_width.keyframe(1, 12)
    copied.source_mix.keyframe(1, 0.5)
    assert not effect.cell_width.keyframes
    assert project.validate().is_valid


@pytest.mark.parametrize("kwargs", [
    {"characters": ""}, {"characters": "a" * 257}, {"characters": "a\n"},
    {"edge_characters": "xyz"}, {"cell_width": 1}, {"cell_height": 129},
    {"edge_strength": 5}, {"source_mix": 1.01}, {"period": 0},
    {"mode": "unknown"}, {"font": ""},
])
def test_ascii_rejects_invalid_public_controls(kwargs):
    with pytest.raises((ValueError, TypeError)):
        Ascii(**kwargs)


def test_ascii_custom_font_is_registered_and_shared_between_clip_and_global():
    font = Path(__file__).resolve().parents[1] / "tests/assets/VestraTest-Regular.ttf"
    project = Project(size=(26, 20), fps=10, duration=4)
    layer = project.root.add(Color("#b0a090"), duration=4)
    layer.effects.add(Ascii(" .#", font=font, cell_width=4, cell_height=6))
    project.post_effects.add(Ascii(" .#", font=font, color_mode="rainbow", period=2))
    canonical = project.snapshot().to_dict()
    fonts = [asset for asset in canonical["assets"] if asset["type"] == "font"]
    assert len(fonts) == 1
    font_id = fonts[0]["id"]
    assert canonical["visual"]["post_effects"][0]["font"] == font_id
    assert project.validate().is_valid
    prepared = project.prepare(backend="cpu")
    late = prepared.render_frame_seconds(2.25).to_bytes()
    assert prepared.render_frame_seconds(0.25).to_bytes() == late
    prepared.render_frame_seconds(1.75)
    assert prepared.render_frame_seconds(0.25).to_bytes() == late


def test_ascii_advanced_builder_preserves_font_ownership_and_tracks():
    first = ProjectBuilder(width=26, height=20, frame_rate=FrameRate(10,1), duration=2, output_path="out.mp4")
    second = ProjectBuilder(width=26, height=20, frame_rate=FrameRate(10,1), duration=2, output_path="other.mp4")
    first.add_solid_color_clip(colour="#808080", start=0, duration=2, layer=0)
    font_path = Path(__file__).resolve().parents[1] / "tests/assets/VestraTest-Regular.ttf"
    font = first.add_font_asset(font_path)
    effect = first.post_effects.add_ascii(font=font, characters=" .#")
    effect.amount.keyframe(time=1, value=0.5)
    effect.mode = "fill"
    effect.characters = " .o#"
    assert effect.mode is AsciiMode.FILL
    assert first.validate().is_valid
    with pytest.raises(ValueError, match="belonging"):
        second.post_effects.add_ascii(font=font)


@pytest.mark.parametrize("characters", ["\U0010ffff", "\u200b"])
def test_ascii_missing_or_invisible_glyphs_fail_preparation(characters):
    project = Project(size=(16, 16), fps=1, duration=1)
    project.root.add(Color("#ffffff"), duration=1).effects.add(Ascii(characters))
    with pytest.raises(Exception, match="glyph"):
        project.prepare(backend="cpu")


@pytest.mark.parametrize("nested", [False, True])
def test_custom_ascii_transition_fonts_lower_to_shared_assets_and_prepare(nested):
    from vestra import Image
    from vestra.transitions import CustomTransition, TransitionLayer

    root = Path(__file__).resolve().parents[1]
    font = root / "tests/assets/VestraTest-Regular.ttf"
    project = Project(size=(26, 20), fps=10, duration=5)
    composition = project.root.group(start=0, duration=5).child if nested else project.root
    first = composition.add(Image(root / "examples/assets/red.png"), start=0, duration=3)
    second = composition.add(Image(root / "examples/assets/blue.png"), start=2, duration=3)
    composition.transitions.add(first, second, CustomTransition(
        outgoing=TransitionLayer(effects=[Ascii(" .#", font=font, cell_width=4, cell_height=6)]),
        incoming=TransitionLayer(effects=[Ascii(" .#", font=font, cell_width=4, cell_height=6)]),
    ), start=2, duration=1)
    snapshot = project.snapshot().to_dict()
    fonts = [asset for asset in snapshot["assets"] if asset["type"] == "font"]
    assert len(fonts) == 1
    visual = snapshot["visual"]["clips"][0]["source"] if nested else snapshot["visual"]
    definition = visual["transitions"][0]["definition"]
    assert definition["outgoing"]["effects"][0]["font"] == fonts[0]["id"]
    assert definition["incoming"]["effects"][0]["font"] == fonts[0]["id"]
    assert project.validate().is_valid
    prepared = project.prepare(backend="cpu")
    frame = prepared.render_frame_seconds(2.25).to_bytes()
    prepared.render_frame_seconds(4.5)
    assert prepared.render_frame_seconds(2.25).to_bytes() == frame


def test_custom_ascii_font_in_owned_mask_group_is_registered_and_prepared():
    from vestra.sources import Group, Rectangle

    font = Path(__file__).resolve().parents[1] / "tests/assets/VestraTest-Regular.ttf"
    project = Project(size=(26, 20), fps=10, duration=2)
    layer = project.root.add(Color("#b0a090"), duration=2)
    mask = Group()
    child = mask.add(Rectangle(width=26, height=20, fill="#ffffff"), duration=2)
    child.effects.add(Ascii(" .#", font=font, background="#00000000"))
    layer.masks.add(mask)
    snapshot = project.snapshot().to_dict()
    fonts = [asset for asset in snapshot["assets"] if asset["type"] == "font"]
    assert len(fonts) == 1
    owned = snapshot["visual"]["clips"][0]["masks"][0]["input"]["source"]
    assert owned["clips"][0]["effects"][0]["font"] == fonts[0]["id"]
    assert project.validate().is_valid
    prepared = project.prepare(backend="cpu")
    assert len(prepared.render_frame_seconds(0.25).to_bytes()) == 26 * 20 * 4
