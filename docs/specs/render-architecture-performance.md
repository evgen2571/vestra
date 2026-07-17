# Staged render architecture and performance plan

## Scope

This document records the v1 renderer migration. It keeps one Cargo crate and
the existing JSON schema, diagnostics, CLI commands, codecs, frame rounding,
and deterministic draw order. The public project format remains version 1.

## Finalization corrections

This document originally described an in-progress migration. The final pass
keeps that design and fixes the gaps found during review.

* Validation retains all v1 clips, including hidden clips, so duration and
  inspection remain compatible. Plan compilation filters hidden clips before
  image preparation and schedule construction.
* Output paths with no parent component use `.` as their effective parent.
  The rule applies to configured paths, CLI overrides, temporary files, and
  publication checks.
* The timeline stores sorted activation and deactivation events. Its memory is
  proportional to scheduled items, not rendered frames. At a shared frame,
  deactivations run before activations, preserving half-open intervals.
* Application timing starts before loading and retains `Duration` values until
  report serialization. The report includes loading, validation, compilation,
  render stages, publication, and end-to-end totals.
* Decoded RGBA frame hashes, never MP4 container bytes, are the compatibility
  fixture. Golden values are source-controlled and updated only by an explicit
  maintenance command documented with the tests.

## Baseline

The baseline was recorded on 2026-07-17 with the repository's supplied assets,
FFmpeg 7.1.5, and a debug `cargo run -- render` invocation. The elapsed figure
includes compilation/process startup, project loading, composition, and
encoding, so it is useful for comparison but not a benchmark.

| Project | Input images | Clips, animated clips | Output | Frames | Decoded frame SHA-256 | Total elapsed |
| --- | --- | --- | --- | ---: | --- | ---: |
| `static-image.json` | 320x180 | 1, 0 | 320x180, 24/1 | 24 | `a42b131016c921e35e0a882ed4253c294ec422c45151d126467e5b79680199eb` | 436 ms |
| `hard-cuts.json` | 320x180 | 2, 0 | 320x180, 24/1 | 48 | `989edadfcc09021aed5633beb2364ef096766086c7cd7957f7eb397c23339585` | 1934 ms |
| `showcase.json` | 320x180 | 3, 3 | 320x180, 24/1 | 80 | `0fbc36457d1c9aba47377627d7718d37d55ac3dc51f1806055f87109ba5e9e3a` | 6323 ms |

The old renderer does not expose per-stage timings. Image decoding happens
before the frame loop, and crop, resize, animation sorting and JSON decoding
all happen in the frame path. FFmpeg write and wait time are also mixed with
composition time. The migration adds those measurements to render reports.

## Current behaviour to preserve

The old compositor builds an item list for every frame, scans clips and
flashes, then sorts by `(layer, start_ns, id, item_kind)`. A clip and flash use
half-open activity intervals. Animation time is clip-relative. Crops floor the
left/top source pixels and ceil the right/bottom pixels. Resizing uses
Lanczos3. Video is H.264, `yuv420p`, and uses CRF 30, 23, or 18. Audio uses
AAC at 192k with the existing trim, fade, delay, pad, and trim filter.

## Target pipeline

```text
CLI
  -> application command
  -> project loader and semantic validation
  -> RenderPlan compiler
  -> prepared image assets and compositor
  -> FFmpeg encoder
  -> temporary output publication and report formatting
```

`project` owns only v1 data, loading, path resolution, and validation.
`plan` converts a `ValidatedProject` into typed, resolved rendering data.
`render` consumes only that plan. `media` owns FFprobe and FFmpeg process
control. `application` owns command workflows. `output` owns paths, progress,
and reports. Shared diagnostics and rational timeline operations remain at the
bottom of the dependency graph.

## Render plan

`RenderPlan` contains the effective canvas dimensions, parsed background,
normalized rational frame rate, nanosecond duration, total frame count,
resolved indexed image and audio assets, clips, flashes, audio settings, and a
frame schedule. A clip stores typed curves, direct transition associations, a
stable draw key, and an image-preparation classification. External
`serde_json::Value` is allowed only in the v1 `Animation` model. The compiler
parses it once into `Point`, `Crop`, or scalar curves.

The renderer does not look up assets by string, parse colours, inspect raw JSON
values, sort curves, or scan all transitions. The schedule turns clip and flash
intervals into ordered activation/deactivation events, so each frame renders
only the active items.

## Asset preparation and cache policy

Each referenced image is decoded once. A clip without crop or scale animation
has its crop and resize prepared once. Position, opacity, and transition-only
clips reuse that bitmap. Crop and scale animations use a bounded cache keyed by
asset index, integer crop rectangle, and final dimensions. Frames themselves
are never cached and flow directly to FFmpeg stdin.

Flashes use direct source-over blending over the canvas. They do not allocate a
second canvas-sized image.

## Compatibility and error handling

The loader retains v1 version detection, Serde shapes, unknown-field rejection,
relative paths, and existing diagnostics. Application results retain command
names, result schema version 1, progress schema version 1, exit mapping, and
line-oriented JSON progress. Output publication continues to render to a
temporary path, preserve an existing target until success, and clean temporary
files on failures or cancellation.

## Migration and tests

1. Split project and media boundaries without changing observable behaviour.
2. Compile and test typed plans and the active schedule.
3. Prepare assets and move composition to typed plan data.
4. Isolate encoder and output publication, then move command workflows out of
   the binary.
5. Add timings, operation counters, decoded-frame equivalence tests, and run
   CLI/FFmpeg verification.

Boundary tests cover validation, typed animation compilation, schedule
boundaries and order, static preparation, direct flash blending, output
publication, and decoded-frame equivalence for all supplied projects.

## Golden decoded frames

Integration tests render the supplied static-image, hard-cuts, and showcase
projects, decode their video streams as RGBA with `ffmpeg -f framemd5`, and
compare a SHA-256 hash of that decoded manifest. MP4 bytes are not compared
because muxer metadata can change without changing pixels. The expected hashes
are source code fixtures. Update them only after reviewing a deliberate visual
change with a known-good renderer, then run the full integration suite.

## Git and Syncthing

Do not synchronize an actively edited `.git` directory with Syncthing. Use
Git push and pull for repository history, or exclude `.git` when file
synchronization is unavoidable. If Syncthing leaves `index.sync-conflict-*`
copies behind, confirm `.git/index` is readable with `git status` and
`git fsck --full`, remove only those named copies, then repeat both checks.

## Completion criteria

The final report will include stage timings and stable counters for decoded
images, static preparations, dynamic cache activity, animation parsing and
sorting, and scheduled-item consideration. These prove the structural changes
without brittle time limits. The same representative projects will be decoded
and compared to the baseline hashes above.

## Final measurements

The final measurements were collected on the same machine after the migration.
`total_ms` below is the renderer timing from the report, not the outer `cargo
run` time. Millisecond fields can read zero for very small stages.

| Project | Final renderer time | Decode | Static preparation | Composition | FFmpeg write/finalize | Structural evidence |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `static-image.json` | 193 ms | 1 ms | 0 ms | 97 ms | 60 / 11 ms | 1 decode, 1 static crop, 1 static resize |
| `hard-cuts.json` | 505 ms | 2 ms | 52 ms | 341 ms | 60 / 12 ms | 2 decodes, 2 static crops, 2 static resizes |
| `showcase.json` | 3034 ms | 4 ms | 69 ms | 2827 ms | 70 / 17 ms | 3 decodes, 2 static preparations, 28 dynamic misses, 7 hits, peak 28 entries |

All three final decoded-frame hashes match the baseline exactly. This is the
strong compatibility check. The wall-clock comparison is intentionally not a
claim of speedup because the old baseline included `cargo run` overhead and
host load varies. The operation counters establish the avoided work: static
clips crop and resize once, images decode once, animated bitmaps use a bounded
128-entry cache, and the active schedule admits only current timeline items.

Composition remains the dominant stage for the animated showcase. The next
useful performance step is profile-guided work on dynamic Lanczos resizing,
then a carefully bounded ordered composition/encoder queue if measurements
justify it.

## Non-goals and future work

This change adds no effects, formats, video inputs, extra audio tracks,
parallelism, GPU code, plugins, or crate split. A future crate split only makes
sense if a reusable schema package, independently versioned renderer, or
language bindings appear. Parallel composition should wait until the staged
measurements identify a CPU bottleneck and a bounded ordered frame queue plus
cancellation semantics are designed.
