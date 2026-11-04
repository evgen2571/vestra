"""Source defaults produce the same visible output as their explicit equivalents."""

from collections.abc import Callable

import pytest

from vestra import Circle, Color, Ellipse, Image, Line, Polygon, Project, Rectangle, Source, Text, Video
from vestra.sources import SolidColor


POLYGON_POINTS = [(-50, -50), (50, -50), (0, 50)]
CLOSED_SHAPES = [Rectangle, Ellipse, Circle, lambda **style: Polygon(points=POLYGON_POINTS, **style)]


def _render(source: Source) -> bytes:
    project = Project(size=(128, 128), fps=1, duration=1, background="#ff0000")
    project.root.add(source)
    return project.render_frame(0, backend="cpu").to_bytes()


@pytest.mark.parametrize(
    "factory,explicit",
    [
        (Rectangle, lambda: Rectangle(width=100, height=100, fill="#ffffff")),
        (Ellipse, lambda: Ellipse(width=100, height=100, fill="#ffffff")),
        (Circle, lambda: Circle(radius=50, fill="#ffffff")),
        (lambda: Polygon(points=POLYGON_POINTS), lambda: Polygon(points=POLYGON_POINTS, fill="#ffffff")),
        (Line, lambda: Line(start=(0, 0), end=(100, 0), stroke="#ffffff", stroke_width=2)),
    ],
)
def test_shape_defaults_render_like_explicit_values(
    factory: Callable[[], Source], explicit: Callable[[], Source],
) -> None:
    pixels = _render(factory())
    assert pixels == _render(explicit())
    assert any(pixels[i:i + 3] == b"\xff\xff\xff" for i in range(0, len(pixels), 4))


@pytest.mark.parametrize("factory", [Color, SolidColor])
def test_solid_colour_defaults_to_opaque_black(factory: Callable[[], Source]) -> None:
    pixels = _render(factory())
    assert pixels == bytes((0, 0, 0, 255)) * (128 * 128)


@pytest.mark.parametrize("factory", CLOSED_SHAPES)
def test_omitted_fill_keeps_stroke_only_shapes_unfilled(factory) -> None:
    shape = factory(stroke="#00ff00")
    pixels = _render(shape)
    assert shape.fill is None
    assert shape.stroke_width == 2
    assert pixels == _render(factory(fill=None, stroke="#00ff00", stroke_width=2))
    center = (64 * 128 + 64) * 4
    assert pixels[center:center + 4] == bytes((255, 0, 0, 255))
    assert any(pixels[i:i + 3] == bytes((0, 255, 0)) for i in range(0, len(pixels), 4))


@pytest.mark.parametrize("factory", CLOSED_SHAPES)
def test_explicitly_disabled_fill_still_requires_a_stroke(factory) -> None:
    with pytest.raises(ValueError, match="fill or stroke"):
        factory(fill=None)


@pytest.mark.parametrize("factory", [*CLOSED_SHAPES, Line])
@pytest.mark.parametrize("width", [0, None])
def test_explicit_invalid_stroke_width_is_not_replaced_by_default(factory, width) -> None:
    with pytest.raises((TypeError, ValueError), match="stroke_width"):
        factory(stroke="#00ff00", stroke_width=width)


@pytest.mark.parametrize("factory", [Image, Video, Text, Polygon])
def test_source_content_stays_required(factory) -> None:
    with pytest.raises(TypeError, match="required"):
        factory()
