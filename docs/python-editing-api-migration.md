# Migrating to the Python editing API

The v2 Python package has one clear recommendation: use `vestra.Project` for
mutable editor code. The native project type keeps its old capabilities under a
new, explicit name so code can say whether it is editing or executing a
snapshot.

## Name mapping

| Previous entry point | v2 name and role |
| --- | --- |
| `vestra.Project` as the immutable native JSON project | `vestra.ProjectSnapshot` |
| `video_editor.Project` | `vestra.ProjectSnapshot` for the old immutable/native project role |
| `vestra.authoring.ProjectBuilder` | Still supported advanced and canonical authoring |
| `vestra._native.Project` | Implementation-level native binding, not normal authoring |
| CLI subprocess rendering | Replace with `Project.render()` or native `Editor`/`PreparedProject` workflows |

The legacy `video_editor` package was removed before the v2 API became stable.
It is no longer shipped or supported. This guide preserves the semantic
mapping for migration: old native-project imports become
`vestra.ProjectSnapshot`, while new mutable editing projects use
`vestra.Project`.

## From a native project

Change imports and make the immutable role explicit:

```python
import vestra

snapshot = vestra.ProjectSnapshot.load("project.json")
prepared = vestra.Editor().prepare(
    snapshot,
    vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
)
frame = prepared.render_frame_number(0)
```

`ProjectSnapshot.from_json()`, `from_dict()`, `to_json()`, `to_dict()`, and
`save()` retain the native project workflow. Do not replace the old import with
mutable `vestra.Project` blindly: the types have different semantics.

## From `ProjectBuilder`

`ProjectBuilder` remains the right choice when exact canonical details matter,
when creating schema fixtures, or when implementing lowering. Existing builder
code does not need a forced rewrite. New mutable application code can move in
small steps:

```python
from vestra import Project
from vestra.sources import Color, Image

project = Project(size=(1280, 720), fps=30, duration=10)
scene = project.root
scene.add(Image("examples/assets/red.png", sizing="cover"), duration=10)
scene.add(Color("#101018"), start=0, duration=10, z=-1)

snapshot = project.snapshot()
```

Use `snapshot.to_dict()` to inspect the canonical result during a migration.
Do not build a second JSON format beside the builder. The editor lowers through
the existing canonical representation and native validator.

The main mechanical mapping is:

| Builder concept | Editor concept |
| --- | --- |
| `ProjectBuilder` | `Project` |
| builder root timeline | `project.root` |
| image/solid/procedural clip factory | `scene.add(Source(...), ...)` |
| group clip | `scene.group(...); group.add(...)` |
| clip-local timing and layer | `Layer.start`, `duration`, `z` |
| builder-owned transform/opacity tracks | `Layer.transform`, `Layer.opacity` |
| clip effect collection | `Layer.effects` |
| global post-effect collection | `Project.post_effects` |
| coordinated root transitions | `Composition.transitions` |
| root flashes | `Project.flashes` |
| builder audio tracks and clips | `Project.audio.track(...).add(...)` |
| `builder.build()` | `Project.snapshot()` |

Builder-specific operations such as exact canonical IDs and low-level asset
handles remain advanced-only. Keep those calls in `vestra.authoring` rather
than reaching into `_native`.

## Migrating from legacy `video_editor`

The old package is no longer importable from Vestra. Choose the modern type
that matches the old code’s role explicitly:

```python
import vestra

snapshot = vestra.ProjectSnapshot.load("project.json")
editor_project = vestra.Project(size=(1280, 720), fps=30, duration=10)
```

For old native immutable projects, replace `video_editor.Project` with
`vestra.ProjectSnapshot`. For new mutable editing graphs, use
`vestra.Project`; these are not interchangeable aliases.

There is no CLI subprocess step in the editor API. A render call lowers and
executes through the Rust SDK directly:

```python
result = editor_project.render("output.mp4", backend="cpu", overwrite=True)
```

For repeated frames, prepare once and reuse the native prepared object:

```python
prepared = editor_project.prepare(backend="cpu")
first = prepared.render_frame_seconds(0.0)
later = prepared.render_frame_seconds(2.5)
```

## Compatibility and deprecation policy

The public split is intentional and documented as follows:

1. `vestra.Project` is the recommended mutable editor API.
2. `vestra.ProjectSnapshot` is the immutable native project API.
3. `vestra.authoring.ProjectBuilder` remains supported as an advanced API.
4. `vestra._native` remains implementation-level and may change with the
   binding, so application code should not make it its primary import.
5. The legacy `video_editor` package is removed and is not part of the
   supported API.

The rule for new features is additive. A feature that is not available in the
editor graph is documented as advanced-only or unsupported; it is not silently
discarded during a snapshot.
