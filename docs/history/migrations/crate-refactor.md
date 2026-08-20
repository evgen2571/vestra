# Phase 4 crate architecture

The repository root is a virtual Cargo workspace. `vestra` is the
supported public Rust SDK; the other crates are internal implementation
details and have `publish = false`.

```text
vestra-cli
        |
        v
vestra
   |       |       |
   v       v       v
 core    render   media
```

`vestra-cli` depends only on `vestra` in production. It owns Clap,
Ctrl-C registration, terminal and JSON presentation, logging setup, and exit
codes. It does not compile plans, select renderer implementations, probe media,
or manage temporary output.

`vestra` owns project loading, deterministic-validation coordination,
environment preflight, inspection, plan compilation coordination, renderer and
media-sink setup, the staged frame loop, progress events, cancellation, and
structured SDK results. It has no Clap, Ctrl-C, terminal, logging-subscriber,
or process-exit dependency. It does not expose WGPU or FFmpeg handles.

`vestra-core` owns project model, diagnostics, deterministic validation,
timeline, and planning. `vestra-render` owns CPU/WGPU execution and
staged backend contracts. `vestra-media` owns FFmpeg/FFprobe, media
probing, sink lifecycle, temporary output, and publication. Media depends on
renderer contracts with renderer defaults disabled.

The public SDK surface is `Editor`, `RenderRequest`, `BackendPreference`,
`CancellationToken`, structured reports/results, events, and `EditorError`.
The event callback is presentation-neutral. Cancellation is one-shot: it
aborts rendering and the sink, removes temporary output, and never publishes a
partial result.

Feature forwarding supports the default CPU+WGPU SDK and a CPU-only SDK build:
`cargo check -p vestra --no-default-features --features cpu`.

Future Python bindings must depend on the SDK only:

```text
vestra-python -> vestra
```

They must never depend on the CLI.
