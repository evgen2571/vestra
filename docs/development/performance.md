# Performance

Measure a named stage, not an undifferentiated "render time". Vestra reports preparation timing for semantic validation, preflight, plan compilation, decode, audio analysis and backend initialization. Render timing then separates track evaluation, frame rendering, encoder writes/finalization, output publication, and WGPU upload/encode/submission/readback work where applicable.

For every result record the revision, command, scene/project, output size/frame rate/quality, frame count, CPU/OS, FFmpeg build, requested renderer preference, actual selected renderer, graphics backend, adapter name and classification. Record whether assets and renderer resources were cold or already prepared. A `PreparedProject` may make later frame/video operations much cheaper than a one-shot render, and that is expected rather than a comparable baseline.

Use separate measurements for planning/preparation, CPU frame rendering, hardware WGPU frame rendering, software WGPU fallback, audio execution, video decode, encoding and end-to-end render. Encoder, muxer and publication time can dominate a short scene; do not call that a renderer regression without a frame-only comparison. Likewise, a media decode change needs a controlled decoder workload.

Do not compare llvmpipe/Lavapipe results with hardware GPU numbers or call them GPU performance. First run [GPU validation](gpu-validation.md) and capture the actual adapter. Historical benchmark files under `docs/history/benchmarks/` record their own environment and are historical evidence, not current performance claims.
