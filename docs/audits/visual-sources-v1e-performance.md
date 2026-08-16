# Visual Sources performance audit

Date: 2026-08-16

## Final conclusion

Visual Sources v1 implementation and the v1E performance/instrumentation work
are complete. CPU correctness, software-WGPU correctness, and hardware-backed
GL correctness pass in the current WSL environment. This report makes no
software-WGPU performance claim. Hardware performance measurements remain
diagnostic rather than a release benchmark.

## Scope

The implementation covers Image, Shape, Text, Video, Spectrum2D,
ParticleSystem, Groups, nested timing, effects, transitions, random access,
and backend parity. Video instrumentation records decoder sessions, requests,
actual decodes, seeks, cache hits/misses, decode time, WGPU uploads, and upload
bytes. Static sources are prepared once; Video sessions and WGPU source
textures are retained and reused where the selected PTS permits it.

No new feature phase was started. Reverse playback, speed ramps/time remapping,
frame interpolation, hardware video decoding, a YUV GPU path, automatic Video
audio, proxy media, full color management, advanced Text, SVG/path sources,
and masks/mattes remain outside the v1 contract.

## Adapter discovery and policy

The renderer-owned adapter discovery and classification utility reported:

| Backend | Adapter | Device type | Classification |
| --- | --- | --- | --- |
| Vulkan | `llvmpipe (LLVM 21.1.8, 256 bits)` | `cpu` | software |
| GL | `D3D12 (NVIDIA GeForce GTX 1650 SUPER)` | `other` | discrete_gpu |

GL is selected for strict WSL hardware validation because it exposes the real
NVIDIA adapter through D3D12. Vulkan/Lavapipe is reserved for software
correctness. Neither software adapter nor software timings are treated as
hardware performance evidence.

## Performance evidence

The retained Video measurements used 320×180 generated workloads and separate
render/encode accounting. They establish behavior and observability, not a
cross-machine performance promise:

| Workload | Backend | Frames | Wall | Render | Video metrics |
| --- | --- | ---: | ---: | ---: | --- |
| single Video | CPU | 90 | 214 ms | 211 ms | 8 sessions, 90 requests, 320 decodes |
| mixed dynamic Video | CPU | 90 | 258 ms | 254 ms | 16 sessions, 119 requests, 533 decodes |
| single Video | GL | 90 | 902 ms | 827 ms | 40 uploads / 9,216,000 bytes |
| mixed dynamic Video | GL | 90 | 914 ms | 846 ms | 62 uploads / 14,284,800 bytes |

The CPU worker measurements and cache/session limits remain covered by the
existing tests. No scheduler redesign or speculative optimization was added.

## Validation evidence

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo check --workspace --all-features` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace --all-features` via `scripts/check.sh` | passed with WSL GL serialization |
| Python pytest with declared test environment | 573 passed |
| schema validation and generated-schema comparison | passed |
| Python compileall | passed |
| software WGPU Vulkan/Lavapipe suite | passed; correctness only |
| strict hardware GL adapter-dependent suite | passed; 47 renderer tests and CLI render regression |
| public Python wheel build/import smoke | passed on CPython 3.13 |

The WSL GL path must serialize tests because Mesa EGL permits only one active
context in this environment. Generic CI does not claim hardware; its explicit
Nix/Lavapipe job validates software WGPU correctness separately.

## Native environment

The host validation used FFmpeg `8.0.1-3ubuntu2`, with libavcodec `62.11.100`,
libavformat `62.3.100`, libavutil `60.8.100`, and libswscale `9.1.100`.
The repository CI source of truth is the pinned `flake.lock` Nix environment,
which supplies FFmpeg 8, native libav libraries, `pkg-config`, Rust, and
software Vulkan tools. CI prints these versions before native builds.

## Reproducible commands

```bash
nix develop
./scripts/check.sh
VESTRA_WGPU_BACKEND=vulkan scripts/verify-wgpu.sh
VESTRA_WGPU_BACKEND=gl scripts/verify-wgpu-hardware.sh
```

`verify-wgpu.sh` is software-capable correctness validation and prints backend,
adapter, device type, and classification. `verify-wgpu-hardware.sh` discovers
and rejects non-hardware adapters before running strict adapter-dependent tests.
