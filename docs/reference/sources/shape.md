# Shape source

Shapes use ordinary source-local pixel coordinates and dimensions. They do
not use normalized coordinates. Normalized points exist elsewhere in Vestra,
including particle emitters and selected effect parameters.

Python exports `Rectangle`, `Ellipse`, `Circle`, `Line`, and `Polygon`; all
lower to a `{"type":"shape"}` source with the geometry listed below. Each shape needs a fill or a
stroke. Colours are canonical `#RRGGBB`/`#RRGGBBAA`; `stroke_width` defaults to
`0`, cannot be negative, and must be positive when a stroke is enabled.

| Shape | Signature and constraints | Canonical geometry |
| --- | --- | --- |
| Rectangle | `Rectangle(*, width, height, fill=None, stroke=None, stroke_width=0, corner_radius=0)`; width/height positive, radius from 0 to half the shorter side. | `rectangle`, width, height, optional `corner_radius` |
| Ellipse | `Ellipse(*, width, height, fill=None, stroke=None, stroke_width=0)`; dimensions positive. | `ellipse`, width, height |
| Circle | `Circle(*, radius, fill=None, stroke=None, stroke_width=0)`; radius positive, serialized as an ellipse with equal diameter. | `ellipse`, width, height |
| Line | `Line(*, start, end, stroke, stroke_width)`; points are finite source-local positions, endpoints differ. | `line`, start, end |
| Polygon | `Polygon(*, points, fill=None, stroke=None, stroke_width=0)`; at least three points. | `polygon`, points |

Layer timing, transform, animation, effects, and transitions apply normally.
Shapes support canonical JSON, high-level Python, CPU, WGPU, nesting, and
direct transition endpoints.
