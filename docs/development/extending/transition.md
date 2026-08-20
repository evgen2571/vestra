# Add a transition

Transitions compile to generic outgoing/incoming presentation channels. Define Python parameters/defaults and canonical representation, validate placement/endpoints/timing, compile normalized tracks and easing, implement CPU/WGPU consumption, regenerate schema where needed, and add high-level, canonical, nested, and renderer tests.

Keep endpoint and local-timeline semantics in core. A built-in transition is a convenience definition, not a separate renderer-only protocol.
