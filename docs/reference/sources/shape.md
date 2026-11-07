# Shape source

Shapes use ordinary source-local pixel coordinates and dimensions. They do
not use normalized coordinates. Normalized points exist elsewhere in Vestra,
including particle emitters and selected effect parameters.

Use shapes for procedural panels, outlines and geometric masks. Python exports
`Rectangle`, `Ellipse`, `Circle`, `Line`, and `Polygon`; all lower to a
`{"type":"shape"}` source.

## API signatures

```text title="Geometry signatures (style and presentation keywords omitted)"
Rectangle(*, width=100, height=100, corner_radius=0.0)
Ellipse(*, width=100, height=100)
Circle(*, radius=50)
Line(*, start=(0, 0), end=(100, 0), stroke="#ffffff", stroke_width=2)
Polygon(*, points)
```

```text title="Closed-shape style keywords"
fill=<automatic>, stroke=None, stroke_width=<automatic>
```

## Fill and stroke

Closed shapes default to a white fill when no fill or stroke is supplied.
Supplying only `stroke` creates an outline without adding a fill. Explicit
`fill=None` disables the fill; each shape must still have a fill or stroke.
Colours are canonical `#RRGGBB`/`#RRGGBBAA`. Omitted `stroke_width` becomes
`2` when a stroke is enabled, otherwise `0`. Explicit widths cannot be negative
and must be positive when a stroke is enabled.

## Geometry and coordinates

| Shape | Geometry defaults and constraints | Canonical geometry |
| --- | --- | --- |
| Rectangle | `width=100`, `height=100`, `corner_radius=0`; dimensions positive, radius from 0 to half the shorter side. | `rectangle`, width, height, optional `corner_radius` |
| Ellipse | `width=100`, `height=100`; dimensions positive. | `ellipse`, width, height |
| Circle | `radius=50`; radius positive, serialized as an ellipse with equal diameter. | `ellipse`, width, height |
| Line | `start=(0, 0)`, `end=(100, 0)`, `stroke="#ffffff"`, `stroke_width=2`; points are finite source-local positions, endpoints differ. | `line`, start, end |
| Polygon | `points` is required, with at least three points; shares the closed-shape style defaults above. | `polygon`, points |

`Rectangle()`, `Ellipse()`, `Circle()`, and `Line()` are ready to render.
`Polygon(points=[(0, 0), (100, 0), (0, 100)])` creates a filled white triangle.
Explicit geometry, colours and stroke widths override the defaults.

## Placement, validation and support

Invalid numeric geometry, colors and stroke widths fail during construction.
Polygon points must be finite; line endpoints must differ. Shape generation
requires no media file preparation. Layer timing, transform, animation, effects,
and transitions apply normally.
Shapes support canonical JSON, high-level Python, CPU, WGPU, nesting, and
direct transition endpoints. These are Python constructor defaults; canonical
JSON keeps its existing requirements and receives the resolved values.

Python constructors also accept [presentation transform defaults](../../guides/python/sources-and-layers.md),
which are copied into each placement and can be individually overridden by `add()`.
