# CPU rendering and work reuse

The CPU renderer consumes evaluated frame plans. Optimization must preserve
pixel rounding, effect order, group opacity, mask/matte coverage and returned
frame ownership. Core remains the authority for normalization and timeline
semantics. See [CPU architecture](architecture/cpu-renderer.md).

## Eliminate invariant work

Compilation normalizes constant tracks and identity effects and fuses compatible
colour operations. Exact identity matters: nearby transforms still resample,
and opacity near one is not opaque. An opaque Normal source at opacity exactly
one can be composed directly. A full-size group with an identity inverse
transform and no motion tiling can avoid bilinear resampling.

Static visual plans render once and reuse pixels. Automatic CPU construction
uses one worker for a fully static visual plan, avoiding unused worker pools.
Dynamic plans retain the CPU and frame-memory limits. Direct backend callers
must use the reported backend capacity rather than assume a worker count.

Complete static layers and groups can be cached subject to their budgets and
all inputs that affect the result. Masks participate in static/dynamic
classification. Animated or signal-driven properties must invalidate static
reuse. Shared decoded images and prepared text/shape resources avoid repeated
I/O and preparation; dynamically transformed images can still resample per frame.

## Scratch and sparse convolution

Effect scratch and depth-indexed composition surfaces are reused across frames.
Every returned RGBA image owns its pixels. Isolated group/matte rendering must
clear shared scratch after child composition before applying group presentation,
so a child effect cannot contaminate the group's coverage.

Gaussian passes precompute byte-to-unit normalization and restrict pixel work
to nonzero-alpha bounds expanded by finite kernel support. Sampling and clamping
stay in original canvas coordinates. When work covers less than the canvas,
reused output must be cleared. A nontransparent corner bypasses the bounds scan.
Fully transparent input can avoid convolution.

Sparse glow/bloom can benefit, but scanning an almost full-frame bound adds
cost without much saved work. Full-frame scratch dimensions remain unchanged.
These mechanisms do not imply generic subtree culling or copy-free rendering.

## Measure cost and resource tradeoffs

Use [performance methodology](performance.md) with release builds, paired runs,
raw samples and identical workloads. Separate preparation, evaluation,
renderer-only work, decode, encoding and publication. A prepared/null-sink
measurement is not comparable with a one-shot encoded render.

Worker count trades throughput against scratch memory and decoder resources.
Cache-hit and copy counters cover engine operations, not every allocation or
process RSS. Validate actual pixel output and cache-disabled references before
keeping an optimization; a counter change alone does not prove a speedup.

Focused manual benchmarks live alongside renderer/SDK tests. Canonical suites
cover effects, masks/mattes, nested groups, particles, blending, concurrent video
and an editorial workload. Keep local raw reports for review, but do not commit
personal machine captures as portable performance targets.
