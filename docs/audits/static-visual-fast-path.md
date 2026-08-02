# Static visual fast path

The user-provided baseline rendered 16,319 frames in 591,364 ms. CPU frame
composition consumed 564,011 ms and FFmpeg writes consumed 26,524 ms. A
layer cache alone still entered the compositor for every frame.

The compiler now records `RenderPlan::visual_dependency`. It is `Static` only
when every layer is static, every layer covers the complete output frame range,
and the post-effect chain is static. Any cut, transition, flash, partial layer,
or time-varying effect is dynamic. Background-only output is static.

CPU cached layer entries retain a one-time `fully_opaque` scan. Normal blend,
opacity exactly one, an opaque cached image, and equal canvas dimensions copy
the RGBA bytes directly. Other cases retain the existing blend path. A cached
opaque first layer also skips the explicit background fill.

Prepared projects retain one immutable final RGBA template when it fits the
existing cache budget. Random frame requests reuse it after the first static
render. File rendering uses the same first final frame as a lossless temporary
PNG. FFmpeg loops that PNG as input zero, keeps audio inputs at one and above,
uses the existing audio graph and codec arguments, and receives `-frames:v`
from the compiled frame count. The PNG is deleted on finish or abort.

Reports remain schema version 1 because the added fields are additive. They
include static-cache, scratch/copy, whole-visual, opaque-copy, and encoder
input metrics. A looped static image reports zero video frames pushed from
Rust. FFmpeg still encodes every output frame and mixes audio.

## Finalization notes

The static-image FFmpeg path is polled instead of blocking in `finish()`. FFmpeg
emits machine-readable `-progress pipe:1` frame counters; the runner forwards
those counters through the existing progress observer and checks the shared
cancellation token while the encoder is active. Cancellation therefore aborts
and reaps FFmpeg and removes the temporary static image instead of becoming
unresponsive for the duration of the encode.

Static-image report metrics are operation-local. Backend lifetime counters are
snapshotted before and after the one-frame population render and converted to
operation deltas with the same helper used by the generic frame path. In
looped-image mode `static_visual_frame_cache_hits` counts actual template reads
used to materialize the temporary image (zero on first population, one when an
existing PreparedProject template is reused), not the output frames generated
internally by FFmpeg. `rendered_frame_count` still reports the encoded output
frame count, while `encoder_video_frames_pushed_from_rust` remains zero.

Generic whole-frame reuse reports every owned-frame copy in
`static_visual_frame_copy_bytes`, including frame zero after first population.
A whole-frame template that exceeds `maximum_cache_bytes` records one
operation-level static-visual cache-budget bypass while rendering continues
correctly without persistent retention.
