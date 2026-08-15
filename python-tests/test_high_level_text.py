import pytest

from vestra import Crossfade, Image, Rectangle, Text
from vestra.effects import GaussianBlur
from vestra.lowering import source_capabilities


FONT = "tests/assets/VestraTest-Regular.ttf"


def _ink_bounds(pixels: bytes, width: int, height: int) -> tuple[int, int, int, int] | None:
    points = [
        (index % width, index // width)
        for index in range(width * height)
        if pixels[index * 4 : index * 4 + 3] != bytes((0, 0, 0))
    ]
    if not points:
        return None
    xs, ys = zip(*points)
    return min(xs), min(ys), max(xs) + 1, max(ys) + 1


def test_text_is_a_static_direct_raster_source() -> None:
    source = Text(
        "Vestra\nvideo",
        font="tests/assets/test-font.ttf",
        font_size=40,
        fill="#ffffff80",
        max_width=300,
        align="center",
        line_spacing=1.2,
        letter_spacing=2,
    )

    assert source.to_canonical() == {
        "type": "text",
        "text": "Vestra\nvideo",
        "font": "tests/assets/test-font.ttf",
        "font_size": 40.0,
        "fill": "#ffffff80",
        "align": "center",
        "max_width": 300.0,
        "line_spacing": 1.2,
        "letter_spacing": 2.0,
    }
    capabilities = source_capabilities(source)
    assert not capabilities.has_intrinsic_duration
    assert capabilities.supports_direct_transform
    assert capabilities.supports_direct_transition_endpoint
    assert not capabilities.supports_sizing
    assert not capabilities.supports_crop


def test_text_lowers_to_canonical_font_asset_and_text_source() -> None:
    import vestra

    project = vestra.Project(size=(320, 180), fps=1, duration=1)
    project.root.add(Text("hello", font="assets/Regular.ttf", font_size=32))
    data = project.snapshot().to_dict()
    assert data["assets"] == [{"id": "font-000001", "type": "font", "source": "assets/Regular.ttf"}]
    assert data["visual"]["clips"][0]["source"]["type"] == "text"
    assert data["visual"]["clips"][0]["source"]["font"] == "font-000001"


def test_text_renders_from_file_backed_font(tmp_path) -> None:
    import vestra

    project = vestra.Project(size=(320, 120), fps=1, duration=1, base_directory=".")
    layer = project.root.add(
        Text("AV ffi\nΔ", font=FONT, font_size=48),
    )
    layer.transform.position = (0.5, 0.5)
    frame = project.render_frame(0, backend="cpu")
    assert frame.width == 320 and frame.height == 120
    assert any(alpha for alpha in frame.to_bytes()[3::4])


def test_invalid_font_data_fails_deterministically(tmp_path) -> None:
    import vestra

    invalid = tmp_path / "invalid.ttf"
    invalid.write_bytes(b"not a font")
    project = vestra.Project(size=(32, 32), fps=1, duration=1)
    project.root.add(Text("x", font=str(invalid), font_size=20))
    with pytest.raises(Exception, match="font"):
        project.render_frame(0, backend="cpu")


def test_text_uses_generic_wgpu_raster_presentation() -> None:
    import vestra

    project = vestra.Project(size=(160, 80), fps=1, duration=1)
    layer = project.root.add(
        Text("GPU\ntext", font=FONT, font_size=32, fill="#ffffff80", max_width=100, align="center"),
    )
    layer.transform.rotation = 0.1
    layer.opacity = 0.8
    cpu = project.render_frame(0, backend="cpu").to_bytes()
    frame = project.render_frame(0, backend="wgpu")
    gpu = frame.to_bytes()
    assert any(alpha for alpha in gpu[3::4])
    assert max(abs(left - right) for left, right in zip(cpu, gpu)) <= 2


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({"font_size": 0}, "font_size"),
        ({"font_size": float("inf")}, "font_size"),
        ({"max_width": 0}, "max_width"),
        ({"line_spacing": 0}, "line_spacing"),
        ({"letter_spacing": float("nan")}, "letter_spacing"),
        ({"align": "justify"}, "align"),
    ],
)
def test_text_rejects_invalid_layout_values(kwargs: dict[str, object], message: str) -> None:
    values: dict[str, object] = {
        "font": "tests/assets/test-font.ttf",
        "font_size": 40,
    }
    values.update(kwargs)
    with pytest.raises((TypeError, ValueError), match=message):
        Text("hello", **values)


def test_text_layout_box_controls_alignment_and_wrapping() -> None:
    import vestra

    def render(source: Text) -> tuple[int, int, int, int] | None:
        project = vestra.Project(size=(240, 180), fps=1, duration=1)
        project.root.add(source)
        return _ink_bounds(project.render_frame(0, backend="cpu").to_bytes(), 240, 180)

    left = render(Text("A", font=FONT, font_size=48, max_width=120, align="left"))
    center = render(Text("A", font=FONT, font_size=48, max_width=120, align="center"))
    right = render(Text("A", font=FONT, font_size=48, max_width=120, align="right"))
    assert left and center and right
    assert left[0] < center[0] < right[0]

    one_line = render(Text("one two three four", font=FONT, font_size=28))
    wrapped = render(Text("one two three four", font=FONT, font_size=28, max_width=100))
    assert one_line and wrapped
    assert wrapped[3] - wrapped[1] > one_line[3] - one_line[1]


def test_text_shaping_and_tracking_are_not_independent_character_widths() -> None:
    def width(text: str, letter_spacing: float = 0.0) -> int:
        project = __import__("vestra").Project(size=(240, 120), fps=1, duration=1)
        project.root.add(Text(text, font=FONT, font_size=48, letter_spacing=letter_spacing))
        bounds = _ink_bounds(project.render_frame(0, backend="cpu").to_bytes(), 240, 120)
        assert bounds
        return bounds[2] - bounds[0]

    assert width("AV") < width("A") + width("V")
    assert width("AV", 8) > width("AV")


def test_text_multiline_spacing_and_empty_content_are_deterministic() -> None:
    import vestra

    def render(text: str, line_spacing: float = 1.0) -> bytes:
        project = vestra.Project(size=(160, 120), fps=1, duration=1)
        project.root.add(Text(text, font=FONT, font_size=32, line_spacing=line_spacing))
        return project.render_frame(0, backend="cpu").to_bytes()

    normal = _ink_bounds(render("A\nA"), 160, 120)
    spaced = _ink_bounds(render("A\nA", 1.5), 160, 120)
    assert normal and spaced and spaced[3] - spaced[1] > normal[3] - normal[1]
    assert _ink_bounds(render(""), 160, 120) is None
    assert _ink_bounds(render("   \n"), 160, 120) is None


def test_text_uses_ordinary_layer_effects_animation_and_random_access() -> None:
    import vestra

    project = vestra.Project(size=(160, 120), fps=2, duration=2)
    layer = project.root.add(Text("Vestra", font=FONT, font_size=42))
    layer.opacity = 0.75
    layer.effects.add(GaussianBlur(radius=1))
    layer.transform.scale.keyframe(0, (1.0, 1.0))
    layer.transform.scale.keyframe(1, (1.5, 1.5))
    first = project.render_frame(0, backend="cpu").to_bytes()
    changed = project.render_frame(1, backend="cpu").to_bytes()
    again = project.render_frame(0, backend="cpu").to_bytes()
    assert first != changed
    assert first == again


@pytest.mark.parametrize("incoming_factory", [
    lambda: Rectangle(width=160, height=120, fill="#ff0000"),
    lambda: Image("tests/assets/wgpu-small-rgba.png"),
])
def test_text_works_in_groups_and_generic_transition_endpoints(incoming_factory) -> None:
    import vestra

    grouped = vestra.Project(size=(160, 120), fps=1, duration=1)
    group = grouped.root.group(duration=1)
    nested = group.group(duration=1)
    nested.add(Text("Group", font=FONT, font_size=32), duration=1)
    group.transform.scale = (1.2, 1.2)
    assert _ink_bounds(grouped.render_frame(0, backend="cpu").to_bytes(), 160, 120)

    transitioned = vestra.Project(size=(160, 120), fps=2, duration=2)
    outgoing = transitioned.root.add(Text("Text", font=FONT, font_size=32), duration=2)
    incoming = transitioned.root.add(incoming_factory(), duration=2)
    transitioned.root.transitions.add(outgoing, incoming, Crossfade(), start=0.5, duration=1)
    assert transitioned.render_frame(0.25, backend="cpu").to_bytes()
    assert transitioned.render_frame(1.0, backend="cpu").to_bytes()
    assert transitioned.render_frame(1.75, backend="cpu").to_bytes()
