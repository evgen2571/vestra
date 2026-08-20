# Projects and compositions

Use `Project` for normal Python authoring. Set the output-wide values once,
then add placements to `project.root`.

```python
import vestra
from vestra.sources import Color

project = vestra.Project(size=(1280, 720), fps=30, duration=5)
background = project.root.add(Color("#111827"), duration=5, id="background")
title = project.root.add(Color("#2563eb"), start=1, duration=2, z=1, id="title")

report = project.validate()
if not report.is_valid:
    raise RuntimeError("project is not valid")
```

The root composition is the normal place for top-level visual layers. Layer
order is controlled by `z`; timing is controlled by `start` and `duration`.
Give important layers IDs so diagnostics and later edits can identify them.

## Create a child composition

Use `group()` when a set of layers should move through the parent as one
placement:

```python
card = project.root.group(start=1, duration=3, z=2, id="card")
card.child.add(Color("#f9fafb"), duration=3, id="card-bg")
card.child.add(Color("#111827"), start=0.25, duration=2, z=1, id="card-text")
```

The child layers use child-local seconds. The group's `start` and `duration`
are parent-local. Use nesting to keep a reusable scene or coordinated set of
layers together, not as a substitute for ordinary layer placement.

`project.validate()` checks canonical project semantics only. It does not read
media paths or check FFmpeg, the output path, or renderer availability. Use the
lower-level `Editor.preflight()` when those environment checks are needed. The
CLI `ve validate` command performs its validation preflight and therefore has a
stronger environment-dependent meaning.

When a project has no explicit duration, non-video layers need a composition
duration from the project or an explicit layer duration.
