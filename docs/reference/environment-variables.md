# Environment variables

The supported user-facing settings are listed first. Internal tuning and
verification controls are identified separately below.

## User-facing settings

| Variable | Values and default | Scope |
| --- | --- | --- |
| `RUST_LOG` | A tracing filter understood by `tracing_subscriber`. Unset uses the configured logging policy (CLI verbosity by default). | Shared CLI/Rust logging and Python after explicit `configure_logging()`. Takes precedence over verbosity and explicit filters when set. |
| `IMAGEIO_FFMPEG_EXE` | FFmpeg executable path or command name. Unset uses the binary installed through `imageio-ffmpeg`. | Python encoding and audio processing. Set before preparing or rendering; prepared sessions retain the selected path. See [executable selection](#choose-a-different-ffmpeg-executable). |
| `VESTRA_WGPU_BACKEND` | Case-insensitive `vulkan`, `gl`/`gles`, `metal`, `dx12`, or `browser_webgpu`. Unset or an unrecognized value lets wgpu consider all compiled backends. | WGPU adapter discovery. It does not select Vestra's CPU/WGPU render backend. |

`VESTRA_WGPU_BACKEND` influences the graphics API used during WGPU adapter
discovery. `--render-backend wgpu` or `BackendPreference.WGPU` chooses Vestra's
renderer. The selected adapter and its device classification are reported
separately when available.

Vestra does not read `WGPU_BACKEND` for this selection.

## Choose a different FFmpeg executable

Python uses the executable packaged by `imageio-ffmpeg`, even when a system
FFmpeg is on `PATH`. To use your own build, set `IMAGEIO_FFMPEG_EXE` before
preparing or rendering:

```bash
export IMAGEIO_FFMPEG_EXE=/absolute/path/to/ffmpeg
```

In PowerShell:

```powershell
$env:IMAGEIO_FFMPEG_EXE = 'C:\tools\ffmpeg.exe'
```

Paths with spaces, relative paths and command names on `PATH` are accepted;
an absolute path is recommended. Invalid overrides fail instead of silently
selecting another executable. Prepared projects retain their selected path;
prepare again after changing the override. Your build must support H.264 via
`libx264`, AAC and Vestra's audio filters.

The extension separately bundles native FFmpeg libraries for probing and video
decoding. `ffprobe` is optional for independent media verification; Vestra does
not invoke it at runtime. Import and semantic validation do not run FFmpeg.
The separate native CLI and Rust SDK default to `ffmpeg` on `PATH`; this Python
override does not configure them. For failures, see
[FFmpeg troubleshooting](../troubleshooting/ffmpeg.md).

## Internal tuning and verification controls

`VESTRA_WGPU_IN_FLIGHT` is an internal implementation tuning variable. It sets
the WGPU pipeline depth, defaults to `3`, accepts only integers `1` through
`3`, and produces a backend diagnostic for an invalid value. It is not normal
user configuration. `VESTRA_CPU_PROFILE` is a
development-only presence toggle for CPU-renderer profiling logs; its value is
not parsed.

`VESTRA_REQUIRE_WGPU`, `VESTRA_REQUIRE_HARDWARE_WGPU`, and
`VESTRA_WGPU_FORCE_FALLBACK` are test or verification controls. The
`VESTRA_ANALYSIS_BENCH_*`, `VESTRA_BENCH_*`, and `VESTRA_RENDER_BENCH*`
families are benchmark/test controls. None are supported project configuration.
