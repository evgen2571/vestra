---
title: Benchmarks
description: Explore Vestra's benchmark workloads, read dated results, and collect results from your own machine.
---

# Benchmarks

Rendering speed depends on the scene, resolution, CPU and graphics backend.
Start with the scene you care about, then compare results captured on the same
machine with the same workload.

[See the workloads](workloads.md){ .md-button }
[Run benchmarks](collecting.md){ .md-button }

## What has been measured

The published capture is a **historical Linux run from 31 October 2026**, at
revision `34dec665`, using a Ryzen 5 3600 and GTX 1650 SUPER with Vulkan.
It is not a measurement of the current checkout, a Windows RTX 4070 Ti, or a Mac.

[![Historical render times for basic export, editing and layered composition, with MoviePy CPU, Vestra CPU and Vestra hardware GPU shown separately. Lower is better.](../assets/benchmarks/moviepy-times.svg)](vestra-vs-moviepy.md#matched-moviepy-v2-workloads)

At 640 × 360 / 30 fps, Vestra CPU completed the three-shot edit in 1.759 s and
MoviePy CPU in 8.652 s. Hardware GPU rendering took 1.239 s. The basic export
shows the other side of the tradeoff: GPU startup and transfer overhead can
outweigh the benefit of rendering a simple scene.

| Timeline | MoviePy CPU | Vestra CPU | Vestra hardware GPU |
| --- | ---: | ---: | ---: |
| Basic export · 10 s video | 0.997 s | 0.835 s | 1.064 s |
| Three-shot edit · 12 s video | 8.652 s | 1.759 s | 1.239 s |
| Layered composition · 10 s video | 4.227 s | 1.789 s | 1.305 s |

Values are median end-to-end render times; **lower is better**. The shared
output uses silent H.264/libx264 CRF 23, veryfast. GPU refers to WGPU rendering;
decoding and encoding still use the CPU. These timelines do not measure every
codec, effect or editing task.

[Read the complete historical results, variability and limitations](vestra-vs-moviepy.md).

## What each suite answers

| Suite or profile | Question | Scene preview |
| --- | --- | --- |
| Default collector | How does this machine handle representative rendering and matched MoviePy edits? | [Editing and layers](workloads.md#editing) |
| Canonical · 720p | How did the engine change on this machine? | [Rendering workloads](workloads.md#rendering-suites) |
| Focused effects · 1080p | What does a particular treatment cost over the shared control? | [ASCII](workloads.md#ascii_video), [dither](workloads.md#dither_video), [CRT](workloads.md#crt_video) |
| Sustained · optional | Does a one- or two-hour output finish, and how do resources behave over time? | [Actual one-hour output excerpt](workloads.md#sustained-v2) |
| Decoder/cache · optional | Which frame-request sequences benefit from cache reuse? | [Probe descriptions](../development/performance-probes.md) |

## Run benchmarks

Use the [collection guide](collecting.md) for one-time platform setup and one
command for the maintained suites. Use the same pinned commit on every computer;
present each machine and graphics backend separately. Failed or unavailable
GPU tests remain visible in the report.

For an optimization, follow the [same-machine baseline procedure](../development/performance.md).
Raw stage timings overlap and are diagnostic; they should not be stacked into
an end-to-end total. Smoke timings only confirm execution.

Current video workloads use bundled footage and suite version 2. The historical
results above retain the earlier definitions; collect a fresh baseline before
comparing the updated workloads.
