# Add a transition

Transitions are composition-owned placements between sibling layers. A high-level definition in `python/vestra/transitions.py` lowers to the canonical transition definition and placement. The core compiler turns it into generic outgoing and incoming presentation channels. CPU and WGPU consume those channels; they should not own endpoint or local-timeline rules.

Add a typed public definition with exact parameters and defaults. Use `Interpolation.EASE_IN_OUT` only where it is the actual default, and validate each unit such as degrees, pixels or normalized progress at the public/core boundary. Add canonical serialization, schema form and semantic validation for placement duration, endpoint ownership, fit/overlap and the definition's tracks/effects.

Then extend compilation/evaluation so easing and normalized progress produce the intended presentation properties. A transition inside a group belongs to that group's child-local timeline, so cover nested compositions before calling it supported. Add CPU and WGPU consumption of every generated channel and transition effect.

Tests should cover definition defaults, invalid parameters, placement endpoints and timing, canonical JSON, a nested owner where supported, and both render paths. Update [Transitions](../../reference/transitions.md) with types, defaults, units and exceptions. Keep one ownership path for transition definitions and placements.
