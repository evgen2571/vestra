# video-editor

`video-editor` is a standalone JSON-driven declarative video renderer written in Rust. It accepts one typed-track JSON project format, composites a deterministic timeline of image and full-canvas solid-colour layers, optionally places one audio track, and writes a playable MP4 without interactive input.

It requires Rust 1.85+ to build and FFmpeg/FFprobe 7+ at runtime. The supported output is H.264 MP4 with `yuv420p` video and AAC audio. Image inputs use formats supported by the Rust `image` crate (including PNG, JPEG, GIF, WebP, BMP, TIFF, and QOI); audio inputs are probed and decoded by FFmpeg (WAV and MP3 are practical baseline formats).

```bash
cargo build --release

# Run these from the repository root.
./target/release/video-editor validate examples/projects/animation-effects.json
./target/release/video-editor inspect examples/projects/animation-effects.json --format json
./target/release/video-editor render examples/projects/animation-effects.json --progress json
```

The reference output is written under `examples/output/` and is protected from accidental replacement. Re-render it only with `--overwrite`.

Key commands are non-interactive:

```text
video-editor validate <project> [--format human|json]
video-editor inspect <project> [--preview] [--format human|json]
video-editor render <project> [--output PATH] [--overwrite] [--preview]
                    [--render-backend auto|cpu|wgpu]
                    [--format human|json] [--progress human|json|none] [--report PATH]
video-editor version
```

`--format json` writes a result envelope. `render --progress json` writes JSON Lines events, starting at zero and ending with `completed` at 1.0 only after publication. Exit statuses are 0 (success), 1 (internal), 2 (usage), 3 (project), 4 (asset/media), 5 (backend/render), 6 (output), and 130 (interrupt cancellation).

Rendering backend selection is a runtime option and is never stored in project
JSON. `cpu` always uses the deterministic CPU compositor. `wgpu` requires a
headless compatible adapter and fails without falling back. `auto` attempts
WGPU during preparation and falls back to CPU only before the first frame;
reports expose `requested_render_backend`, selected `render_backend`, optional
`backend_fallback`, and adapter metadata. WGPU is headless: decoded image
textures, pipelines, and readback storage persist for a render; each output
frame is still read back to CPU RGBA for FFmpeg, so it is not zero-copy or
hardware video encoding.
Set `VIDEO_EDITOR_WGPU_FORCE_FALLBACK=1` to prefer a fallback adapter, or
`VIDEO_EDITOR_WGPU_BACKEND=vulkan|gl|metal|dx12` to constrain adapter discovery
for CI or headless troubleshooting.

See [the project format](docs/specs/project-format.md), [the canonical schema](schemas/project.schema.json), and [the animation/effects example](examples/projects/animation-effects.json) for the complete contract. Run the canonical check suite with:

```bash
python3 -m pip install -r requirements-dev.txt
./scripts/check.sh
cargo bench --bench animation_effects
```

The benchmark uses the canonical animation/effects fixture at 720×1280. Run it
with `VIDEO_EDITOR_BENCH_BACKEND=cpu` or `VIDEO_EDITOR_BENCH_BACKEND=wgpu` to
select the renderer; the output records the selected backend and available GPU
preparation timings. WGPU benchmark results require a compatible adapter.

`requirements-dev.txt` pins the Python package used by the JSON Schema check;
Rust dependencies are locked in `Cargo.lock`.

Create a source package from tracked files only. This omits ignored render
outputs, reports, temporary files, benchmark output, and Cargo build artifacts.

```bash
git archive --format=zip --output=video-editor.zip HEAD
```
