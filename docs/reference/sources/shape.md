# Shape source

Python exposes `Rectangle`, `Ellipse`, `Circle`, `Line`, and `Polygon` through
`vestra.sources`. The canonical source tag is `shape` with a geometry object.
Shapes use normalized source-local coordinates and can have `fill`, `stroke`,
and `stroke_width` where the geometry supports them.

Rectangles have width, height, optional radius, fill, and stroke. Ellipses have
width and height. Circles use a positive radius. Lines have distinct `start`
and `end` points and stroke data. Polygons require at least three points.
Layer transforms, animation, effects, and transitions apply normally. Shape
sources are supported by CPU and WGPU; validation rejects malformed geometry,
invalid colors, and invalid dimensions.
