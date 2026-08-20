# Render pipeline

`Editor` loads or receives a `Project`, validates canonical data, performs target-specific preflight, prepares an owned render snapshot, then renders frames. One-shot rendering also checks output/FFmpeg readiness. Preparation owns decoded assets, compiled plans, backend state, and warnings; a `PreparedProject` can render frames or video without repeating that setup.

The engine emits `RenderEvent` values while work progresses. Cancellation is cooperative and prevents successful publication when observed before the publication boundary. Media writes temporary output, finalizes it, then publishes the final path. Results report requested/actual backend and adapter information; failures retain diagnostics, stage, timing, and cleanup context.
