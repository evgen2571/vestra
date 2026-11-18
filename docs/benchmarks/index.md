---
title: Benchmarks
description: Explore benchmark workloads, measured results, and the collection workflow.
---

# Benchmarks

The scenes, resolution and frame rate matter as much as the hardware. These measurements use the shared footage at **1920 × 1080 / 60 fps**.

[See the workloads](workloads.md){ .md-button }
[Run benchmarks](collecting.md){ .md-button }

<span id="measured-on-this-machine"></span>

## 1080p60 editing benchmarks

Measured on **18 November 2026** on Windows 11 with a Core i5-14600KF and an RTX 4060 Ti using Vulkan.

<figure class="comparison" markdown="span">
![Windows render times for basic export, editing and layered composition on MoviePy CPU, Vestra CPU and Vestra GPU. Lower is better.](../assets/benchmarks/moviepy-times.svg)
</figure>

| Timeline | MoviePy CPU | Vestra CPU | Vestra hardware GPU |
| --- | ---: | ---: | ---: |
| Basic export | 10.684 s | 6.900 s | 6.032 s |
| Three-shot edit | 110.567 s | 16.686 s | 7.357 s |
| Layered composition | 46.392 s | 17.754 s | 8.741 s |

Values are median end-to-end render times; **lower is better**. Basic and layered produce 10-second videos; editing produces 12 seconds. All use silent H.264/libx264 CRF 23 / veryfast. GPU refers to WGPU rendering; decoding and encoding use the CPU.

We also measured these workloads on Linux with a Ryzen 5 3600 and GTX 1650 SUPER. See the [Windows capture](vestra-vs-moviepy.md#windows-capture) for sample ranges and one-hour renders, and the [Linux capture](vestra-vs-moviepy.md#linux-capture) for the editing and native benchmark results.

## What each suite answers

| Suite or profile | Question | Workload |
| --- | --- | --- |
| Default collector · 1080p60 | How does this machine handle representative rendering and matched MoviePy edits? | [Editing and layers](workloads.md#editing) |
| Canonical · 1080p60 | How did the engine change on this machine? | [Rendering workloads](workloads.md#rendering-suites) |
| Effects · 1080p60 | What does each treatment cost over the shared control? | [Effect comparison](workloads.md#effects-at-a-glance) |
| Sustained · optional | Does a one- or two-hour output finish, and how do resources behave over time? | [Endurance workload](workloads.md#sustained) |
| Decoder/cache · optional | Which request patterns benefit from cache reuse? | [Probe descriptions](../development/performance-probes.md) |

## Run benchmarks

Use the [collection guide](collecting.md) for one-time setup and one command for the maintained suites. Keep the source, input files and settings matched when comparing machines. Failed or unavailable GPU tests remain visible in the report.

For an optimization, follow the [same-machine baseline procedure](../development/performance.md). Raw stage timings overlap and must not be added into an end-to-end total. Smoke timings only confirm execution.
