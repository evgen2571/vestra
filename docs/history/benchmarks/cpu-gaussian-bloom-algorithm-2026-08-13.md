# CPU Gaussian/Bloom algorithm investigation

## Summary

This subphase revalidated the accepted Color Adjust LUT profile, rebuilt the
post-LUT CPU ranking, measured Gaussian radius and Bloom stage scaling, and
evaluated one exact Gaussian optimization experiment. No production algorithm
was retained: the opaque-input specialization was byte-identical but produced
only a sub-2% change, so its complexity did not earn a place in the renderer.
The existing exact separable Gaussian remains unchanged. No Bloom quality,
CPU/WGPU semantics, or public API was changed.

## Environment and validation

The measurements used the existing Linux x86_64 host, CPU backend, automatic
8-worker policy, Cargo release/bench profile, 1280x720 focused fixtures and
the canonical 1920x1080 combined fixture. The canonical mixed run used two
warmups and five measured runs. The pre-edit repository check was:

```text
./scripts/check.sh
exit 0
```

The documented relative `TMPDIR=target/tmp` was missing from the crate's
working directory, so the first benchmark invocation failed before rendering
with `PathError: .../crates/vestra/target/tmp/... No such file or directory`.
The rerun used the scoped absolute workspace directory
`/home/agent/dev/video-editor/target/tmp` and completed normally. This was an
environment/setup issue, not a source failure.

## Mixed-profile verification

The earlier LUT audit paired a pre/post mixed result that reported roughly
208 CPU-ms/frame for Color Adjust in both cases. That was inconsistent with
the focused LUT reduction and was not used as evidence here.

The fresh current mixed run emitted five measured post-LUT profiles after two
warmups. Color Adjust was 10.4–11.2 aggregate worker CPU-ms/frame, with a
measured median of **10.872 CPU-ms/frame** in this combined fixture. The
focused LUT audit's approximately 2.49 CPU-ms/frame remains a separate,
isolated kernel measurement; the mixed aggregate includes the operation's
surrounding worker/effect accounting. The old approximately 208 value is not
present in the fresh profile.

The mixed summary was:

```text
combined 1920x1080
frames=180, warmups=2, samples=5, workers=8
total wall median=46997 ms, range=46784..47115 ms, frames=180,
ms/frame median=261.09, effective FPS=3.83
render total median=46997 ms, aggregate worker CPU=360765 ms
```

## Concrete post-LUT ranking

The following are medians from the five measured profile lines. Values are
aggregate worker CPU-ms/frame, not wall milliseconds. Parent timings such as
`global_post_effect`, `effect_execution`, `bloom_glow`, and `sharpen` are
inclusive and are excluded from this exclusive ranking to avoid double
counting. The listed pass timers are exclusive concrete work.

The denominator is the sum of the listed non-overlapping exclusive timings:
2000.4 aggregate CPU-ms/frame. Inclusive parent timings are not included.

| Exclusive work | CPU-ms/frame | Share of listed exclusive work |
| --- | ---: | ---: |
| ZoomBlur | 714.2 | 35.7% |
| Source rasterization | 302.8 | 15.1% |
| Chromatic aberration | 208.2 | 10.4% |
| Sharpen Gaussian H/V | 237.2 | 11.9% |
| Layer composition | 116.3 | 5.8% |
| Vignette | 97.9 | 4.9% |
| Sharpen Unsharp composite | 43.3 | 2.2% |
| Bloom additive composite | 57.1 | 2.9% |
| Bloom Gaussian H/V | 182.1 | 9.1% |
| Bloom highlight extraction | 28.1 | 1.4% |
| Color Adjust | 10.9 | 0.5% |
| Surface-copy traffic | 2.3 | 0.1% |

The rounded shares sum to 100.0%. Because ZoomBlur and several other effects
dominate this particular mixed fixture, Gaussian-derived work is not the
largest single mixed kernel, but Bloom and Sharpen Gaussian remain the largest
shared Gaussian family and the focused scaling data identifies the algorithmic
cost.

## Current Gaussian/Bloom architecture

The core pass plan is unchanged:

- Standalone Gaussian uses two passes: horizontal then vertical.
- Each pass uses a canonical radius, support `ceil(radius)`, and a normalized
  Gaussian kernel with `2 * support + 1` weights. The bounded thread-local
  cache retains at most 16 kernels.
- Bloom/Glow uses highlight extraction from the retained original, horizontal
  Gaussian, vertical Gaussian, and an additive alpha-aware composite.
- Sharpen reuses the same two Gaussian directions against the retained
  original, then applies an Unsharp composite.
- CPU surfaces use the existing three-surface pool and ordered liveness; no
  pass-order or resource topology changed.

## Radius scaling

The focused Gaussian fixture animates to radius 4; the large variant changes
the authored value to radius 16. At 1280x720, 150 total rendered frames per
benchmark process, the repeated release summaries were:

| Workload | Radius | Support/kernel | Frames | Total wall median | ms/frame | FPS | Before Gaussian CPU median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Gaussian small | 4 | 9 taps per direction | 150 | 4603 ms | 30.69 | 32.59 | about 10.2 ms/frame |
| Gaussian large | 16 | 33 taps per direction | 150 | 6500 ms | 43.33 | 23.08 | about 25.5 ms/frame |

The cost increases with the expected width × height × radius separable
convolution work. The result did not support another indexing-only cleanup.

## Bloom stage breakdown

For the 1280x720 Glow fixture (radius 4, 150 frames), the five measured
profiles reported approximately these aggregate worker CPU medians:

```text
highlight extraction: 1.61 CPU-ms/frame
Gaussian H/V:         10.40 CPU-ms/frame
composite:             3.26 CPU-ms/frame
Bloom/Glow total:     15.22 CPU-ms/frame
```

Blur is the largest Bloom stage, but extraction and composite are material
parts of the total, so a blur-only speedup would not equal a whole-effect
speedup.

## Candidate strategies

The following strategies were evaluated against the current implementation:

1. Another raw-buffer/indexing or small inner-loop Gaussian cleanup. This is
   exact and low-risk, but the accepted prior experiment already measured only
   about 1.6% kernel reduction, so it was rejected as insufficient.
2. An exact opaque-input Gaussian specialization. This reduces alpha and
   premultiplication work only after scanning the source and preserves the
   existing exact path for transparent and semi-transparent inputs. The
   experiment passed byte-equality tests for both directions and radii 1,
   2.5, 16, and 32, but changed focused wall medians by only 0.6–1.2%:
   Gaussian small 4603→4552 ms, large 6500→6458 ms, and Sharpen 4290→4237
   ms. It was reverted because the gain was within benchmark variance and
   below the value needed for an additional branch and full-image alpha scan.
3. Bloom-only downsample/blur/upsample. Its theoretical pixel-work reduction
   is strong and its temporary memory can be bounded, but it is an
   approximate Bloom algorithm requiring explicit golden/difference validation
   and CPU/WGPU semantic treatment. It was not promoted without a justified
   parity contract and would risk silent changes to small highlights, edges,
   alpha, and intensity.
4. Repeated box blur or recursive Gaussian approximation. These are linear or
   near-linear in radius and could reduce large-radius work, but they are not
   byte-identical, alter standalone Gaussian semantics, and would require a
   quality/parity decision outside this CPU-only subphase.

## Selected strategy

No production strategy was selected. The exact candidate did not materially
improve throughput, while the approximate candidates would require a larger
quality and parity change than the measured evidence currently justifies.
The repository retains the accepted exact separable implementation.

## Correctness, quality, and compatibility

The experiment's focused tests passed, including existing transparent,
semi-transparent, edge, Gaussian, Glow, and Sharpen tests plus the temporary
opaque equality coverage. The experiment was reverted, so final renderer
semantics are unchanged and no Bloom difference image or tolerance change is
needed. Standalone Gaussian remains byte-identical to its pre-subphase output.

No WGPU implementation or parity tolerance changed. No new surfaces,
allocations, caches, locks, threads, nested parallelism, or public Rust,
Python, CLI, or project-schema changes were retained. Random-access rendering,
effect ordering, worker policy, Color Adjust LUT code, ZoomBlur code, and
rasterization code are unchanged.

## Before and experiment benchmark comparison

All headline rows used two warmups and five measured processes at 1280x720.
The experiment was removed after measurement; therefore these are evidence for
the rejection, not final after-production results.

| Workload | Frames | Before total wall | Before ms/frame, FPS | Experiment total wall | Experiment ms/frame, FPS | Wall change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Gaussian small | 150 | 4603 ms | 30.69, 32.59 | 4552 ms | 30.35, 32.95 | 1.1% faster |
| Gaussian large | 150 | 6500 ms | 43.33, 23.08 | 6458 ms | 43.05, 23.23 | 0.6% faster |
| Sharpen | 150 | 4290 ms | 28.60, 34.97 | 4237 ms | 28.25, 35.40 | 1.2% faster |

The experiment's kernel changes were similarly small and did not establish a
material whole-render gain. No after-production benchmark claim is made.

## Validation status and remaining bottleneck

Focused CPU-effects tests passed during the experiment, and the experiment was
reverted cleanly. The audit-only correction was followed by the canonical
validation:

```text
./scripts/check.sh
exit 0
```

The corrected post-LUT mixed ranking points to ZoomBlur as the next concrete
mixed-workload target, with source rasterization and Sharpen Gaussian behind
it. The recommended next subphase is **ZoomBlur/mixed-workload investigation**;
it is not implemented here.
