# Install Vestra

Use Python 3.11, 3.12, 3.13, or 3.14. Install the public package from PyPI:

```bash
python -m pip install vestra==0.1.0
python -c "import vestra; print(vestra.__version__)"
```

Vestra uses the `ffmpeg` executable to encode output and `ffprobe` for media
inspection. Install FFmpeg with your platform's package manager and confirm
both commands are on `PATH`. The [Python quickstart](python-quickstart.md) renders
a small CPU project after installation.

## Develop from a checkout

Building Vestra from source also needs Rust, `uv`, a C compiler, `make`, `git`,
and `nasm` for the bundled FFmpeg build. The repository's Nix flake
provides a pinned development environment:

```bash
nix develop
```

On other systems, install these tools with your package manager. The bundled
FFmpeg build fetches its source on the first build.

## Set up a checkout

From the repository root, create the locked development environment:

```bash
uv sync --locked --extra dev
```

The project uses Maturin as its Python build backend. `uv sync` builds the
editable package from `crates/vestra-python`, so commands run through `uv run`
can import the native extension:

```bash
uv run python -c "import vestra; print(vestra.__version__)"
```

To build the CLI binary, use Cargo:

```bash
cargo build -p vestra-cli
./target/debug/ve version
```

For a Nix-based setup, the equivalent commands can be run inside `nix develop`.

## Verify a source checkout

Check the two entry points before starting a tutorial:

```bash
uv run python -c "import vestra; print(vestra.__version__)"
cargo run -q -p vestra-cli -- version
ffmpeg -version
```

If the Python import fails, rerun `uv sync --locked --extra dev` and check that
the native build can find the FFmpeg development libraries. If a render fails
while encoding, check the FFmpeg executable. The [Python quickstart](python-quickstart.md)
and [CLI quickstart](cli-quickstart.md) use the CPU backend so the first render
does not depend on a hardware WGPU adapter.

Next, choose a [Python first render](python-quickstart.md) or a [CLI first render](cli-quickstart.md).
