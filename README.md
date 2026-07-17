# video-editor

`video-editor` is a standalone JSON-driven declarative video renderer written in Rust. It validates a version-1 project, composites a deterministic timeline of local images, optionally places one audio track, and writes a playable MP4 without interactive input.

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

See [the v1 specification](docs/specs/mvp-v1.md), [the machine-readable schema](schemas/project-v1.schema.json), and the example projects for the complete contract. Run all checks with:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
