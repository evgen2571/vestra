from vestra import (
    Circle, Color, Ellipse, Line, ParticleSystem, Point, PointKeyframe, Polygon, Rectangle, Source,
)
from vestra.authoring import ParticleSystem as AdvancedParticleSystem
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

keyframes = (PointKeyframe(0, Point(0.25, 0.5)),)
system = ParticleSystem(emitter_keyframes=keyframes, pre_roll=0.5)
configured_frames: tuple[PointKeyframe, ...] = system.emitter_keyframes
pre_roll: float = system.pre_roll
advanced = AdvancedParticleSystem(emitter_keyframes=keyframes, pre_roll=pre_roll)
