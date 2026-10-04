# Use nested compositions

Nesting groups related layers and gives the group one parent placement. Create
the group in the parent, then author through its `child` composition.

```python
group = project.root.group(start=1, duration=3, z=2, id="scene")
group.child.add(Color("#0f172a"), duration=3)
# This starts 0.5 seconds into the group, at project time 1.5 seconds.
badge = group.child.add(Color("#f97316"), start=0.5, duration=1.5, z=1)
badge.transform.scale.value = 0.5
```

The group starts at parent time one. `badge` starts at child time 0.5, so it
appears at parent time 1.5. Transforms, opacity, effects, and transitions on
the `CompositionLayer` affect the child placement in the parent. The child
layers retain their own local properties.

Use `group.child.transitions` for transitions between child siblings. Use
`project.root.transitions` when the child composition itself is one endpoint
of a parent transition.

Nesting is recursive, subject to the project's validation limits. Keep a
composition's timing explicit and validate after building deep or dynamically
generated graphs. In the high-level API, `CompositionLayer` is the authoring
object. Lowering turns it into the canonical group form; those names describe
related concepts, not interchangeable public objects.
