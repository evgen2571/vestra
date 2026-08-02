# Phase 10A — render-plan normalization

## Scope

Phase 10A changes only compiler-owned visual-plan semantics. It adds no cache,
frame reuse, buffer pool, output/API, schema, or audio changes.

`TemporalDependency::Static` means the content of an operation is invariant at
every time for which that operation is active. `Dynamic` means a compiled track
or an operation's own active interval can change its content. Clip activity on
the timeline is not itself dynamic: a constant clip scheduled from 10s to 20s
remains static.

The compiler normalizes private `Track` copies. A track with every keyframe
equal to its value at zero becomes a constant track; authored project data and
keyframe order are untouched. Crop, opacity, transform, generated transform
contributions, local effects, timed effect intervals, and post effects all feed
the dependency result. Timed effects and transform contributions use the same
half-open interval rule: `start <= time < end`. A partial-time otherwise-
constant effect or non-identity transform contribution is dynamic because
entering or leaving its interval changes the layer; it is interval-static only
when `start == 0 && end >= owner_duration`. A constant identity transform
contribution is discarded before dependency propagation.

## Normalization

Exact compiler-eliminated identities are brightness `0`, contrast `1`,
saturation `1`, tint `0`, Gaussian blur whose canonical radius is at most
`0.01`, directional/zoom/chromatic blur radius at most `0.01`, glow with zero
intensity or identity Gaussian radius, vignette `0`, sharpen with zero amount
or identity Gaussian radius, default color-adjust values, and motion blur with
zero intensity, and CameraShake with constant `position_amount == 0`,
`rotation_degrees == 0`, and `scale_amount == 0`. CameraShake frequency, seed,
and envelope cannot affect pixels without an amplitude, so this exact identity
is removed before evaluation. Near-zero amplitudes remain. Validation happens
before compilation, so invalid zero-amplitude authored shake data is still
rejected.

The only fused family is a complete contiguous static normal-blend chain of
brightness, contrast, saturation, and tint. It becomes one internal compiled
`ColourTransform` and one backend-neutral colour pass. It is never serialized
or exposed as a project effect. Fusion is intentionally not applied across
blur, advanced effects, non-normal blend composition, or a partial/dynamic
effect interval: those paths observe intermediate byte-space rounding and
clamping, so broader affine fusion would not be exact.

Compilation is a single linear scan of tracks and effect chains; no
duration-proportional frame table is created. `CompilationStats` records
`effect_count_before_normalization` (authored executable local and post effects),
`effect_count_after_normalization` (remaining compiled effects),
`static_layer_count` and `dynamic_layer_count` (compiled visual layers by final
`content_dependency`), and `constant_track_normalization_count` (track-shaped
values with one or more authored keyframes that compile to an empty-keyframe
constant track). These are structural counts, not CPU/GPU work or time saved.

## Propagation and ordering

- Image source crop, base transform, base opacity, and opacity contributions
  are static only after their tracks normalize to constants. A non-identity
  transform contribution is also static only when its tracks are constant and
  its half-open interval covers the complete owner interval. A solid source has
  no time-varying source property.
- A timed local effect is static only when its parameters are static and it
  covers the full clip. A partial effect is dynamic even with constant
  parameters because its activation changes the clip.
- Transition-generated opacity/transform/effect work is dynamic. Static clips
  otherwise remain static while merely scheduled on the timeline.
- Post effects expose the combined `RenderPlan::post_effect_dependency`; a
  constant full-project post chain is static and an animated or partial one is
  dynamic.
- Fusion preserves declaration order by composing the affine transforms in
  order. It never crosses an incompatible operation. The implemented safe set
  is deliberately narrower than arbitrary contiguous groups: an entire
  normal-blend layer chain must be static basic colour work. Advanced and
  non-normal paths retain their existing per-pass byte rounding/clamping.

The four supported static/dynamic fusion combinations are therefore: static +
static is fused only when it forms that complete compatible layer chain;
static + dynamic, dynamic + static, and dynamic + dynamic remain separate.
This avoids changing the observable intermediate-clamping semantics of those
paths.

## Structural evidence

The representative compiler fixture has two static layers, one dynamic layer,
two constant-track normalizations, nine effects before normalization, and three
after normalization. It includes an exact zero CameraShake, a three-operation
basic-colour chain that becomes one `ColourTransform`, and a single non-fused
effect. Focused core tests also cover partial constant transform contributions
(Dynamic), full-duration constant contributions (Static), animated full-duration
contributions (Dynamic), exact/near CameraShake identities, and deterministic
recompilation. Exact identities do not enter `PlanEvaluator`, CPU execution, or
WGPU frame planning.

## Classification is not reuse

Phase 10A proves semantic `Static` versus `Dynamic`, normalizes constants,
removes identities, and safely fuses compatible colour work. It does not cache
or persist rendered CPU RGBA output, WGPU output, subtrees, or frames; static
layers may still be traversed and rendered per frame. `TemporalDependency` is
the correctness foundation for Phase 10B cacheability, not a claim that runtime
work is already skipped.

`PreparedProject` repeat rendering, random frame access, cancellation, audio,
and lifecycle ownership are unchanged; they remain covered by the existing
workspace staged-render and prepared-project suites. This phase creates no
resources, so it has no teardown state.

## Deferred

Phase 10B owns cache keys, invalidation, and surface reuse. Phase 10C owns
buffer/resource pools and copy reduction. Phase 10D owns final benchmark
matrices and performance claims.

## Verification

- `cargo fmt --all -- --check`, `cargo check --workspace`, strict workspace
  Clippy, and `cargo test --workspace` passed. Schema validation passed.
- `cargo test -p video-editor-core` passed: 54 tests. `cargo test -p
  video-editor-render` passed: 120 tests, including WGPU compilation and
  adapter-aware test coverage; four Python adapter-gated cases were skipped
  because this environment has no compatible runtime adapter.
- `maturin develop`, `python -m pytest python-tests` (259 passed, 4 skipped),
  `python -m mypy python/video_editor python-tests/test_typing.py`, and
  repository-allowlisted `mypy.stubtest` passed on CPython 3.13.5.
- `maturin build --release` passed. An isolated wheel install imported
  `video_editor` and `video_editor.authoring`, confirmed `py.typed`, and used
  public `to_dict`, `build`, `validate`, `inspect`, CPU `prepare`, and frame
  rendering successfully.
