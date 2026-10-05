# Install Vestra

Install Vestra into a Python project with uv or pip. The Python wheel includes
the native engine.

## Install into a Python project with uv

In a Python project, add Vestra and use its environment to run your scripts:

```bash
uv add vestra
uv run python -c "import vestra; print(vestra.__version__)"
```

For a new project, run `uv init my-video`, then `cd my-video` first.
Follow the [Python quickstart](python-quickstart.md), save its code as
`quickstart.py`, and run `uv run python quickstart.py` to receive
`quickstart.mp4`.

## Install with pip

In your chosen Python environment:

```bash
python -m pip install vestra
python -c "import vestra; print(vestra.__version__)"
```

Run the same quickstart with `python quickstart.py`. Wheel users do not need
Rust, a C compiler or NASM. If no wheel matches your platform/interpreter,
pip may attempt a source build, which needs the tools below.

## Install the FFmpeg runtime

Install FFmpeg with your platform's package manager. Both `ffmpeg` (encoding)
and `ffprobe` (media inspection) must be available on `PATH` in the shell that
runs Vestra:

```bash
ffmpeg -version
ffprobe -version
```

These executables are required even though wheels bundle the native FFmpeg
libraries used by the engine. A working Python import does not check them.
For failures, see [FFmpeg troubleshooting](../troubleshooting/ffmpeg.md).

## Supported Python versions and platforms

Vestra 0.1.1 supports CPython 3.11–3.14. Published wheels cover Linux x86_64
and aarch64 (glibc 2.28+), macOS Intel and Apple Silicon, and Windows x64.
The `ve` CLI is built separately from the Rust checkout.

## Build from source / development

This section is for contributors and platforms without a matching wheel.
For contribution guidance, see [Contributing](../../CONTRIBUTING.md).

A source build needs Rust, uv, a C compiler, libclang, `make`, `git`, and NASM,
plus network access for the initial bundled FFmpeg source fetch. Runtime
rendering still needs the FFmpeg executables above. The repository's Nix flake
provides a pinned development environment:

```bash
nix develop
just python-sync
uv run python -c "import vestra; print(vestra.__version__)"
just check
```

Without Nix, install equivalent tools with your platform's package manager and
run `uv sync --locked --extra dev` from the checkout. Maturin builds the editable
native extension from `crates/vestra-python`. See [testing](../development/testing.md)
for contributor checks.

Build the separate CLI with Cargo:

```bash
cargo build -p vestra-cli
cargo run -q -p vestra-cli -- version
```

The binary is `target/debug/ve` (`ve.exe` on Windows). Add it to `PATH` if you
want to use the short CLI commands. Continue with the
[CLI quickstart](cli-quickstart.md) or [Python quickstart](python-quickstart.md).
