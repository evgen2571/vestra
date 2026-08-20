# Nested compositions

High-level Python uses `CompositionLayer`. The canonical JSON model uses a
visual source with type `group`. These are two names for two authoring layers,
not two unrelated runtime concepts.

`Composition.group(name=None, *, start=0, duration=None, z=0, visible=True,
opacity=1, id=None, blend_mode=BlendMode.NORMAL)` creates a parent placement
with a child composition. Child layer times are local to the child.
The group start and duration are local to the parent. Child transforms are
evaluated in the child, then the parent placement applies its placement and
transform.

Transitions connect layers owned by one composition. A child composition can
therefore contain its own transitions. Current tests cover nested transition
compilation. Endpoints must be owned by the same
composition and satisfy the transition's timing and fit constraints.

Groups serialize inside the canonical visual tree. Recursive nesting is
supported subject to validation limits and timing constraints. CPU and WGPU
support nested composition evaluation; adapter selection and media readiness
remain backend and environment concerns.
