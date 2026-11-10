<span id="vestra-reference"></span>

# Full reference index

Every reference topic is listed here by subject. Use the [reference overview](index.md)
to match a task to a guide and its exact contract, or start with
[your first render](../getting-started/first-render.md) if you are new to Vestra.

<span id="apis-and-features"></span>

## APIs and project format

- [Python API](python-api.md): authoring, snapshots, rendering and runtime objects.
- [Rust SDK](rust-sdk.md): project handles, preparation, frames and reports.
- [CLI](cli.md): commands, flags and validation behavior.
- [Project format](project-format.md): canonical JSON objects and validation layers.

[Project schema v1](https://github.com/evgen2571/vestra/blob/develop/schemas/project.schema.json)
is the machine-readable schema; the format and effect references explain semantic
validation and parser differences that schema acceptance alone cannot establish.

## Composition, timing and audio

- [Animation](animation.md): properties, keyframes, interpolation and bindings.
- [Nested compositions](nested-compositions.md): group ownership, local canvases and timing.
- [Masks and mattes](masks.md): owned coverage sources and sibling track mattes.
- [Audio](audio.md): clip timing, effects, crossfades and output policy.
- [Signals](signals.md): prepared scalar signals and audio analysis.

## Effects and transitions

- [Effects](effects.md): attachment, processing order and effect-specific behavior.
- [Effect parameters](effect-parameters.md): generated defaults, units, bounds and exact schema metadata.
- [Transitions](transitions.md): placements, definitions and normalized channels.
- [Presets and flashes](presets-and-flashes.md): editable authoring helpers and overlays.

<span id="source-reference"></span>

## Source types

- [Shapes](sources/shape.md): pixel geometry, fill and stroke.
- [Solid color](sources/solid-color.md): canvas fills and Python presentation adapters.
- [Text](sources/text.md): layout, font selection and preparation.
- [Images](sources/image.md): sizing, crop and file preparation.
- [Video](sources/video.md): source construction, placement timing and probing.
- [Particles](sources/particle-system.md): emitters, motion, lifetime and appearance.
- [Spectrum2D](sources/spectrum2d.md): prepared master-audio spectra and normalized layouts.

## Rendering and diagnosis

- [Render outputs](render-outputs.md): formats, previews, frame selection and publication.
- [Backends](backends.md): backend selection, adapters and parity expectations.
- [Feature support](feature-support.md): current coverage and verification evidence.
- [Diagnostics](diagnostics.md): diagnostic objects, codes and failure stages.
- [Environment variables](environment-variables.md): runtime, GPU and diagnostic controls.

<span id="concepts"></span>

For explanations of the model, use the
[concept routes in the overview](index.md#understand-the-model).
