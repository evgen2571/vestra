from vestra import Circle, Color, Ellipse, Line, Polygon, Rectangle, Source
from vestra.sources import SolidColor

sources: list[Source] = [
    Color(),
    SolidColor(),
    Rectangle(),
    Ellipse(),
    Circle(),
    Line(),
    Polygon(points=[(0, 0), (100, 0), (0, 100)]),
    Rectangle(stroke="#00ff00"),
    Circle(fill=None, stroke="#00ff00"),
    Ellipse(fill="#ff0000", stroke_width=0),
]
