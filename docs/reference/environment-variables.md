# Environment variables

Only variables that affect user-visible runs belong here. Benchmark, CI, and
verification-only variables are intentionally excluded.

| Variable | Values and default | Scope |
| --- | --- | --- |
| `RUST_LOG` | A tracing filter understood by `tracing_subscriber`. Unset uses the CLI verbosity defaults. | CLI and Rust process logging. Takes precedence over `-v`, `-vv`, and `-vvv` when set. |
| `VESTRA_WGPU_BACKEND` | A WGPU backend name supported by the installed wgpu build, commonly `gl` or `vulkan`. Unset lets wgpu choose. | WGPU adapter discovery. It does not select Vestra's CPU/WGPU render backend. |

`VESTRA_WGPU_BACKEND` influences the graphics API used during WGPU adapter
discovery. `--render-backend wgpu` or `BackendPreference.WGPU` chooses Vestra's
renderer. The selected adapter and its device classification are reported
separately when available.

`VESTRA_REQUIRE_WGPU`, `VESTRA_REQUIRE_HARDWARE_WGPU`, and
`VESTRA_WGPU_FORCE_FALLBACK` are test or verification controls used by the
repository, not normal project configuration. `VESTRA_*` benchmark variables
are likewise development-only.
