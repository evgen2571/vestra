# CPU renderer

The CPU backend receives prepared decoded resources and evaluated frame plans. It rasterizes sources, composites layers, applies effect passes, resolves nested compositions, and returns RGBA frame output for sinks or frame callers. Worker-local caches and reusable resources sit at the hot boundaries for crops, static layers, decoded video, geometry, and effect work.

Keep the backend a consumer of plan semantics. Do not introduce project-model rules into CPU dispatch, and avoid allocation or I/O in the per-frame path.
