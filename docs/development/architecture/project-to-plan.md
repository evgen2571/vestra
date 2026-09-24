# Project to plan

The canonical project is portable JSON, not a render-ready scene. `vestra-core::project` deserializes its schema-v1 model and semantic validation checks IDs, timelines, output constraints, effect scopes, transition endpoints, signals and other relationships without reading assets. The SDK preflight layer then resolves relative paths against the project base directory, probes media, computes media-dependent duration facts, checks the operation target, and passes those facts to compilation.

```text
canonical Project
  -> semantic ValidationReport
  -> SDK preflight and resolved media facts
  -> PlanCompileInput / compiled plan
  -> evaluated frame plan at time t
  -> backend frame work
```

Compilation exists so cross-clip work happens once. It turns the validated visual hierarchy, audio timeline, descriptor-backed effects, transitions, tracks and warnings into renderer-independent plan data. It normalizes timeline authority, frame count and asset references before a renderer can see them. `ValidatedProject::plan_compile_input` is the SDK boundary that carries resolved paths, audio/video duration and dimensions into the core compiler; core itself does not touch files or media tools.

Evaluation remains time-dependent. For each frame it uses the checked frame time, active intervals, keyframes, signal values and transition progress to produce evaluated layers and source work. Renderers receive those evaluated sources and presentation channels, not public source objects. This matters for nested compositions: a group has its own local timeline, compilation preserves that ownership, and evaluation recurses into it instead of flattening away its timing rules.

Transitions lower to generic outgoing/incoming presentation channels. Effects lower through the descriptor/catalog representation and retain ordered attachment scope. Both mechanisms stay in core planning/evaluation so CPU and WGPU apply the same meaning. Planning diagnostics retain canonical JSON pointers; preflight diagnostics add the environment or media fact that failed.
