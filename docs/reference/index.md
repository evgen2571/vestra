---
title: Reference overview
description: Find Vestra's Python API, guides, source types, effects, JSON format and rendering contracts.
---

<span id="guides-and-reference"></span>

# Reference overview

Find a task below for a practical guide and its exact contract. For an introduction,
start with [your first render](../learn/first-render.md). For a complete list of
signatures, formats and feature topics, use the [full reference index](catalog.md).

## Work in Python

| I want to… | Practical guide | Detailed reference |
| --- | --- | --- |
| Arrange layers and sources | [Sources and layers](../guides/python/sources-and-layers.md) | [Python API](python-api.md) |
| Animate a property | [Animation and keyframes](../guides/python/animation.md) | [Animation contracts](animation.md) |
| Put layers in a group | [Nested compositions](../guides/python/nested-compositions.md) | [Composition contracts](nested-compositions.md) |
| Apply visual effects | [Effects](../guides/python/effects.md) | [Effect contracts](effects.md), [parameters](effect-parameters.md) |
| Reveal part of an image | [Masks](../guides/python/masks.md) | [Masks and mattes](masks.md) |
| Connect two shots | [Transitions and presets](../guides/python/transitions-flashes-and-presets.md) | [Transitions](transitions.md), [presets and flashes](presets-and-flashes.md) |
| Mix a soundtrack | [Audio](../guides/python/audio.md) | [Audio contracts](audio.md) |
| Make graphics react to sound | [Signals and audio reactivity](../guides/python/signals-and-audio-reactivity.md) | [Signals](signals.md) |
| Render videos, frames or previews | [Rendering and preparation](../guides/python/rendering-and-preparation.md) | [Render outputs](render-outputs.md), [backends](backends.md) |

## Choose a source

Sources describe what a layer draws. Start with [shapes](sources/shape.md), [text](sources/text.md), or a [solid color](sources/solid-color.md). Bring in an [image](sources/image.md) or [video](sources/video.md), or generate a [particle system](sources/particle-system.md) or [audio spectrum](sources/spectrum2d.md).

## Understand the model

- [Projects and compositions](../guides/python/projects-and-compositions.md): organize a project before adding complexity.
- [The authoring model](../concepts/authoring-model.md): sources become independently editable layers.
- [Timeline and time](../concepts/timeline-and-time.md): composition, clip, and source timing.
- [The rendering lifecycle](../concepts/rendering-lifecycle.md): validation, preparation, and output.
- [Feature support](feature-support.md): current CPU and WGPU coverage. A working adapter is separate from feature support.

## Use JSON, the CLI, or Rust

The Python API lowers to a canonical project model. You can also author that model directly or use the Rust SDK.

- [JSON authoring](../guides/json-authoring.md) and [project-format reference](project-format.md).
- [CLI quickstart](../getting-started/cli-quickstart.md), [rendering commands](../guides/cli/rendering.md), and [CLI reference](cli.md). Download a separate native CLI archive or build from source; installing the Python wheel does not install `ve`.
- [Rust SDK](rust-sdk.md).

## Diagnose a problem

Start with the failing part: [Python installation](../troubleshooting/python.md), [FFmpeg](../troubleshooting/ffmpeg.md), [rendering](../troubleshooting/rendering.md), or [WGPU](../troubleshooting/wgpu.md). Use the [diagnostic codes](diagnostics.md) and [environment variables](environment-variables.md) when you need precise control.

## Contribute

Start with [Contributing to Vestra](../development/README.md) to choose a development
task; [Documentation authoring](../development/documentation.md) covers this site.
