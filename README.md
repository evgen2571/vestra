# video-editor

`video-editor` is a standalone JSON-driven declarative video renderer written in Rust. It accepts compatible version-1 projects and version-2 typed animation projects, composites a deterministic timeline of image and solid-color layers, optionally places one audio track, and writes a playable MP4 without interactive input.

It requires Rust 1.85+ to build and FFmpeg/FFprobe 7+ at runtime. The supported output is H.264 MP4 with `yuv420p` video and AAC audio. Image inputs use formats supported by the Rust `image` crate (including PNG, JPEG, GIF, WebP, BMP, TIFF, and QOI); audio inputs are probed and decoded by FFmpeg (WAV and MP3 are practical baseline formats).

```bash
cargo build --release

# Run these from the repository root.
./target/release/video-editor validate examples/projects/showcase.json
./target/release/video-editor inspect examples/projects/showcase.json --format json
./target/release/video-editor render examples/projects/showcase.json --progress json
```

The reference output is written under `examples/output/` and is protected from accidental replacement. Re-render it only with `--overwrite`.

Key commands are non-interactive:

```text
video-editor validate <project> [--format human|json]
video-editor inspect <project> [--preview] [--format human|json]
video-editor render <project> [--output PATH] [--overwrite] [--preview]
                    [--format human|json] [--progress human|json|none] [--report PATH]
video-editor version
```

`--format json` writes a versioned result envelope. `render --progress json` writes JSON Lines events, starting at zero and ending with `completed` at 1.0 only after publication. Exit statuses are 0 (success), 1 (internal), 2 (usage), 3 (project), 4 (asset/media), 5 (backend/render), 6 (output), and 130 (interrupt cancellation).

See [the v1 specification](docs/specs/mvp-v1.md), [the v2 animation design](docs/specs/animation-effects-v2-design.md), [the staged render architecture and performance notes](docs/specs/render-architecture-performance.md), [the v1 schema](schemas/project-v1.schema.json), [the v2 schema](schemas/project-v2.schema.json), and the example projects for the complete contract. The v2 example is [animation-effects-v2.json](examples/projects/animation-effects-v2.json). Run all checks with:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-features
```

Create a source package from tracked files only. This omits ignored render
outputs, reports, temporary files, benchmark output, and Cargo build artifacts.

```bash
git archive --format=zip --output=video-editor.zip HEAD
```
