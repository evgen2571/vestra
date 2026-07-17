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

The live code is organized around `application`, `project`, `plan`, `domain`,
`timeline`, `render`, `media`, and `output`. The JSON model stays at the
project boundary. The compiler converts it into a typed plan, then rendering
uses plan and domain types only. FFmpeg receives `EncoderSettings`, not clips,
animations, or the full plan. `main.rs` remains a small adapter to the CLI.

`visible: false` is a validation and inspection concern, not a render item.
Hidden clips may determine automatic duration, but they create no image assets,
prepared bitmaps, cache accesses, or schedule events. A filename-only output
such as `output.mp4` has `.` as its effective parent. This applies equally to
temporary publication paths and CLI overrides.

Reports serialize whole-millisecond values only after accumulating each stage
as `Duration`. `project_load_and_validation_ms`, `plan_compile_ms`, asset
decode and preparation, composition, encoder, publication, and total timings
are available. Stage values need not sum exactly to the end-to-end total.

## Baseline

The baseline was recorded on 2026-07-17 with the repository's supplied assets,
FFmpeg 7.1.5, and a debug `cargo run -- render` invocation. The elapsed figure
includes compilation/process startup, project loading, composition, and
encoding, so it is useful for comparison but not a benchmark.

| Project | Input images | Clips, animated clips | Output | Frames | Normalized decoded frame SHA-256 | Total elapsed |
| --- | --- | --- | --- | ---: | --- | ---: |
| `static-image.json` | 320x180 | 1, 0 | 320x180, 24/1 | 24 | `fc7569a6e6fee60f72ef9a75b7d105e9bd88d701a1bdb80cfea0dba13e35c68f` | 436 ms |
| `hard-cuts.json` | 320x180 | 2, 0 | 320x180, 24/1 | 48 | `9e5c8ac6690da827a1df6c693a91208e4216591c701aec3250ae2add59df20ce` | 1934 ms |
| `showcase.json` | 320x180 | 3, 3 | 320x180, 24/1 | 80 | `121e830d20142bc3f6441b930a31cd63c29f028ffc3e94d82a8b7c7eab438237` | 6323 ms |

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
projects, decode their video streams as RGBA with `ffmpeg -map 0:v:0 -pix_fmt
rgba -f framemd5 -`, discard every `#` comment line, canonicalize each remaining
six-field frame record as comma-separated LF-terminated text, and compare the
SHA-256 hash of those bytes. MP4 bytes and FFmpeg/Lavf header versions are not
compared because neither identifies decoded pixels. The expected hashes live in
`tests/cli.rs`, which is the fixture source of truth.

The suite checks static-image, hard-cuts, showcase, preview, and hidden-clip
paths. The preview fixture uses the showcase hash because its canvas is already
below the preview size bound. The hidden-clip fixture uses the static-image hash
because invisible clips never enter preparation or composition.

Golden fixtures never update during ordinary tests. To update one, render the
project deliberately, run its golden test to print the normalized value, inspect
frames before accepting the change, then update the constant and this document.
The command uses the exact FFmpeg arguments above and hashes the complete frame
sequence. Update a hash only for a reviewed visual correction or an intentional
fixture change.

## Final stabilization guarantees

Each compiled clip owns a start-sorted transition opacity envelope. An incoming
segment holds opacity at zero before its start and reaches one at its end. An
outgoing segment starts at one and reaches zero at its end. Between sequential
segments the previous endpoint remains in force, so a fade-in followed by a
later fade-out stays fully visible in the middle. Clip animation opacity and
transition opacity multiply, then the compositor clamps the result to zero
through one.

The portable normalized RGBA hashes are `static-image`
`fc7569a6e6fee60f72ef9a75b7d105e9bd88d701a1bdb80cfea0dba13e35c68f`,
`hard-cuts` `9e5c8ac6690da827a1df6c693a91208e4216591c701aec3250ae2add59df20ce`,
and `showcase` `121e830d20142bc3f6441b930a31cd63c29f028ffc3e94d82a8b7c7eab438237`.
These intentionally differ from the older manifest hashes because comments are
no longer included. The showcase value also reflects correct evaluation of its
later transition.

Commands follow a typed path: Clap parses arguments, application services build
serializable command results, and output modules present human text, JSON
envelopes, progress lines, and reports. The CLI does not assemble application
JSON or report documents. Validated projects and compiled render plans expose
no public mutable invariant fields. Validation and compilation remain the only
normal construction paths.

Render failures retain the stage, completed and attempted frame numbers, total
frames, timeline position where applicable, output paths, and cleanup result.
Reports carry that context instead of assigning a guessed completion value.
Requested reports cover project, plan, rendering, cancellation, and output
failures. If report writing fails, output keeps the original diagnostic and
adds the report-write diagnostic.

## Git and Syncthing

Do not synchronize an actively edited `.git` directory with Syncthing. Use
Git push and pull for repository history, or exclude `.git` when file
synchronization is unavoidable. If Syncthing leaves `index.sync-conflict-*`
copies behind, confirm `.git/index` is readable with `git status` and
`git fsck --full`, remove only those named copies, then repeat both checks.

Share source-only snapshots with `git archive --format=tar HEAD | gzip >
video-editor-source.tar.gz`. `git archive` reads tracked files, so ignored
render outputs, local reports, and benchmark products never enter the archive.

## Completion criteria

The final report will include stage timings and stable counters for decoded
images, static preparations, dynamic cache activity, animation parsing and
sorting, and scheduled-item consideration. These prove the structural changes
without brittle time limits. The same representative projects will be decoded
and compared to the normalized fixture hashes above.

## Final measurements

The final measurements were collected on the same machine after the migration.
`total_ms` below is the renderer timing from the report, not the outer `cargo
run` time. Millisecond fields can read zero for very small stages.

| Project | Final renderer time | Decode | Static preparation | Composition | FFmpeg write/finalize | Structural evidence |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `static-image.json` | 193 ms | 1 ms | 0 ms | 97 ms | 60 / 11 ms | 1 decode, 1 static crop, 1 static resize |
| `hard-cuts.json` | 505 ms | 2 ms | 52 ms | 341 ms | 60 / 12 ms | 2 decodes, 2 static crops, 2 static resizes |
| `showcase.json` | 3034 ms | 4 ms | 69 ms | 2827 ms | 70 / 17 ms | 3 decodes, 2 static preparations, 28 dynamic misses, 7 hits, peak 28 entries |

The initial final hashes matched the pre-stabilization renderer. The current
fixtures use normalized `framemd5` records instead, so their values are listed
in the golden-hash section above and are portable across FFmpeg header changes.
The wall-clock comparison is intentionally not a claim of speedup because the
old baseline included `cargo run` overhead and host load varies. The operation
counters establish the avoided work: static clips crop and resize once, images
decode once, animated bitmaps use a bounded 128-entry cache, and the active
schedule admits only current timeline items.

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
