# Environment variables

Only variables that affect user-visible runs belong here. Benchmark, CI, and
verification-only variables are intentionally excluded.

| Variable | Values and default | Scope |
| --- | --- | --- |
| `RUST_LOG` | A tracing filter understood by `tracing_subscriber`. Unset uses the CLI verbosity defaults. | CLI and Rust process logging. Takes precedence over `-v`, `-vv`, and `-vvv` when set. |
| `VESTRA_WGPU_BACKEND` | Case-insensitive `vulkan`, `gl`/`gles`, `metal`, `dx12`, or `browser_webgpu`. Unset or an unrecognized value lets wgpu consider all compiled backends. | WGPU adapter discovery. It does not select Vestra's CPU/WGPU render backend. |

`VESTRA_WGPU_BACKEND` influences the graphics API used during WGPU adapter
discovery. `--render-backend wgpu` or `BackendPreference.WGPU` chooses Vestra's
renderer. The selected adapter and its device classification are reported
separately when available.

`VIDEO_EDITOR_WGPU_BACKEND` is a legacy compatibility alias. Vestra reads it
only when `VESTRA_WGPU_BACKEND` is unset; new configurations must use the
`VESTRA_*` name. Vestra does not read `WGPU_BACKEND` for this selection.

`VESTRA_WGPU_IN_FLIGHT` is an internal implementation tuning variable. It sets
the WGPU pipeline depth, defaults to `3`, accepts only integers `1` through
`3`, and produces a backend diagnostic for an invalid value. The legacy
`VIDEO_EDITOR_WGPU_IN_FLIGHT` alias is used only when the current name is
unset. Neither name is normal user configuration. `VESTRA_CPU_PROFILE` is a
development-only presence toggle for CPU-renderer profiling logs; its value is
not parsed.

`VESTRA_REQUIRE_WGPU`, `VESTRA_REQUIRE_HARDWARE_WGPU`, and
`VESTRA_WGPU_FORCE_FALLBACK` are test or verification controls. The
`VESTRA_ANALYSIS_BENCH_*`, `VESTRA_BENCH_*`, and `VESTRA_RENDER_BENCH*`
families are benchmark/test controls. None are supported project configuration.
