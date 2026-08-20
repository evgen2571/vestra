# Install Vestra

Vestra is currently used from a source checkout. The Rust crates are marked
`publish = false`, and the Python package is built locally with Maturin. There
is no supported `pip install vestra` or `cargo install vestra` command.

## Prerequisites

You need:

- Python 3.11 or newer.
- A Rust toolchain supported by the repository's `Cargo.lock` and edition 2024
  workspace.
- `uv` for the locked Python environment.
- FFmpeg for runtime encoding and FFmpeg development libraries discoverable by
  `pkg-config` when building the Rust media crate.

The repository's Nix flake supplies a convenient development environment with
Rust, Python, `uv`, `pkg-config`, FFmpeg, and the FFmpeg development outputs:

```bash
nix develop
```

Nix is convenient, not mandatory. On other systems, install the prerequisites
with the platform's package manager and make sure both `ffmpeg` and the native
FFmpeg libraries are available.

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

## Verify the setup

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
