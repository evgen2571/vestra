# Add a transition

Transitions are composition-owned placements between sibling layers. Most new
looks can be defined in Python using existing generic presentation channels;
they do not need a new canonical transition kind or a renderer branch. Follow
[Crossfade and DirectionalPush](https://github.com/evgen2571/vestra/blob/develop/python/vestra/transitions.py)
from their public definitions to their outgoing/incoming channel construction.

## Find the owners

| Responsibility | Location |
| --- | --- |
| Public definitions, parameters and channel construction | `python/vestra/transitions.py` |
| Placement authoring and lowering | `python/vestra/authoring/transitions.py`, `python/vestra/lowering.py` |
| Canonical definitions/placements | `crates/vestra-core/src/project/model/transitions.rs` |
| Endpoint, duration and overlap rules | `crates/vestra-core/src/validation/transitions.rs` |
| Placement-time mapping and compiled channels | `crates/vestra-core/src/plan/compiler/transitions.rs` |
| Channel/effect consumption | CPU and WGPU presentation paths; see [render pipeline](../architecture/render-pipeline.md) |

## Implement in order

1. Define exact public parameters and defaults. Validate units such as degrees,
   pixels or normalized progress. Use `Interpolation.EASE_IN_OUT` only where
   it is the actual default.
2. Construct the generic outgoing/incoming channels and lower the definition
   and placement. Reuse canonical serialization/schema when those channels
   suffice. Extend the core contract only if a required channel is missing.
3. For core changes, update serialization, schema and semantic rules for
   placement duration, endpoint ownership, fit/overlap and tracks/effects.
   Compilation/evaluation owns easing and normalized-progress mapping; CPU
   and WGPU must not reinterpret endpoint or timeline rules.
4. Cover every generated channel and effect on both render paths. A transition
   inside a group uses that group's child-local timeline; test nested ownership
   before claiming it supported.

## Verify and document

Test definition defaults, invalid parameters, canonical JSON, placement endpoints
and timing, nested ownership where supported, and actual CPU/WGPU output. Reuse
core tests beside the model/compiler and Python transition tests; choose commands
from [Testing](../testing.md). Keep software and named hardware evidence separate
as described in [GPU validation](../gpu-validation.md).

Update [Transitions](../../reference/transitions.md) with types, defaults, units
and exceptions. The change is ready when public lowering and core timing agree,
both backends consume every claimed channel, and unsupported or unverified cases
are explicit. Keep one ownership path for definitions and placements.
