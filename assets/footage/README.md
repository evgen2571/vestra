# Shared footage

`forest.mp4`, `valley.mp4` and `tear.mp4` are the shared video inputs for the
showcases and benchmarks. Each is a silent 12-second, 1280×720, 30 fps H.264 clip
with yuv420p pixels and a two-second GOP. All three together are about 19 MiB.

The [manifest](manifest.json) records encoding settings, decoded frame counts
and SHA-256 checksums. `just showcase-assets` verifies the bundled checksums.
Benchmark collection resizes these files locally and records those input hashes
in the results. No separate footage download or asset folder is required.

Changing these files changes the workloads. Update their manifest, regenerate
affected previews and collect a fresh baseline rather than comparing against
measurements made with different inputs.
