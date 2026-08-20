# The authoring model

Vestra describes a video as a project graph. The graph says what the project
contains and where each item appears. Rendering evaluates that graph at a
particular time.

```text
Project
└── root Composition
    ├── Layer ── Source
    ├── Layer ── Source
    └── CompositionLayer
        └── child Composition
```

## Project

`Project` owns the output-wide settings: size, frame rate, optional duration,
background, output audio policy, and the root composition. It also owns the
audio timeline, project-level visual effects, and root flash overlays.

The high-level Python `Project` is mutable while you author it. Calls such as
`project.root.add(...)` update that authoring graph. `snapshot()` lowers the
graph to Vestra's canonical project model. `validate()`, `prepare()`, and
`render()` operate on that canonical snapshot.

## Composition and layer

A `Composition` is an ordered collection of layers in one local time space. A
`Layer` is a placement, not a piece of media. It has a start, duration, source
offset, playback rate, z position, visibility, opacity, transform, and blend
mode. Its `source` property supplies the visual content evaluated at that
placement.

The distinction matters. Reusing a source does not place it twice. Add it to a
composition twice to create two independent placements with different timing
or presentation properties.

## Sources

A `Source` describes content such as an image, video, colour, shape, text,
particle system, or spectrum. It does not own a timeline position. The layer
created by `composition.add(source, ...)` supplies that position.

The high-level editor copies a source when it creates a layer. Later changes to
the original source object do not turn into a second hidden placement. Treat a
layer as the object you configure for timing and presentation.

## Nested compositions

`composition.group(...)` creates a `CompositionLayer`. The returned layer is
the parent placement. Its `child` composition is where you author the child
layers:

```python
group = project.root.group(start=1, duration=3, id="title-card")
group.child.add(Color("#202020"), duration=3)
```

The child layers keep their own local times. The group placement controls when
the child composition appears in its parent. This is an authoring relationship,
not a claim that `CompositionLayer` and canonical `Group` are the same public
object. Lowering converts the high-level graph to the canonical representation.

## When lower-level objects matter

Most Python projects should stay with `Project`, `Composition`, `Layer`, and
the source classes. Use `ProjectBuilder` when exact canonical authoring and
asset IDs matter. Use `ProjectSnapshot`, `Editor`, `PrepareOptions`, and
`PreparedProject` when you need explicit validation, preparation, repeated
frame rendering, or a lower-level render request.

Those representations share the same project semantics. They are different
interfaces for different jobs.
