---
name: media-output-validation
description: "Use when validating Vestra decoding, encoded media, AV synchronization, long-duration or large-file output, finalization, publication, cancellation or media failure cleanup."
---

# Verify published media and failure outcomes

Read [media ownership and output verification](../../../docs/development/media-pipeline.md#verify-encoded-output)
and [render lifecycle](../../../docs/concepts/rendering-lifecycle.md).
Use the existing test seams in [testing](../../../docs/development/testing.md#timeline-and-media-checks).

1. Reuse generated test media and helpers: explicit resolution/rational rate,
   identifiable moving frames and audio markers, fixed seeds, lossless fixtures
   where exact pixels matter. Keep outputs under `target/` or test temp dirs.
2. Render through the public SDK/CLI/Python operation being tested. Record
   source/project/output time, requested and selected render backend, adapter,
   FFmpeg build, quality and actual encoder. WGPU rendering does not imply
   hardware decoding or encoding; current output is H264/yuv420p MP4.
3. Probe the **published** output for codec/format, dimensions, time base,
   decoded frame count and duration. Inspect frame presentation timestamps
   and decoded audio markers at the beginning/middle/end; account for B-frame
   reordering, codec delay and container rounding instead of requiring PTS=DTS.
4. For long/large outputs, state actual elapsed timeline, bytes, disk budget,
   timestamp range and sampling coverage. Respect current caps and use existing
   internal test-limit seams only where supported. A short low-FPS smoke is not
   evidence of sustained throughput, multi-hour stability or >4 GiB behavior.
5. Exercise decode/write/finalize/publish errors and cancellation with existing
   fault seams. Check worker/process joins, temporary cleanup, preserved prior
   destination, structured failure context and terminal result/event. File
   existence or a finished frame loop alone is not successful publication.

Use [visual-regression](../visual-regression/SKILL.md) for raw frame/alpha fidelity;
lossy encoded output is a separate contract. Keep CPU, software-WGPU and actual
hardware-WGPU evidence separate. Report unsupported/unverified paths explicitly;
resumability, alternate codecs and hardware encoder interop remain research.
