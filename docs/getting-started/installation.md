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

Vestra supports CPython 3.11–3.14. Published wheels cover Linux x86_64
and aarch64 (glibc 2.28+), macOS Intel and Apple Silicon, and Windows x64.
The `ve` CLI is built separately from the Rust checkout.

<a id="build-from-source--development"></a>

## Build from source / development

This section is for contributors and platforms without a matching wheel.
For contribution guidance, see [Contributing](https://github.com/evgen2571/vestra/blob/develop/CONTRIBUTING.md).

A source build compiles the Rust engine and bundled FFmpeg libraries. It needs
Rust, Python 3.11–3.14, uv, a C compiler, libclang, `make`, `git`, and NASM,
plus network access for the initial FFmpeg source fetch. The first build can
take a while. Runtime rendering still needs the FFmpeg executables above.

Cargo builds the Rust workspace. Maturin, configured in `pyproject.toml`, builds
the Python extension from `crates/vestra-python`; uv invokes it when installing
the editable package. The Justfile provides shortcuts for build and test tasks.

### With Nix (Linux and macOS)

The repository's Nix flake provides a pinned development environment, including
Just and the native build dependencies. Install
[Nix](https://nix.dev/install-nix) with
[flakes enabled](https://nix.dev/manual/nix/stable/command-ref/conf-file.html#conf-experimental-features), then
enter the shell from the repository root:

```bash
nix develop
```

Run the build and checks inside that shell:

```bash
just python-sync
uv run python -c "import vestra; print(vestra.__version__)"
just check
just python-test
```

### Without Nix

Install these tools and make sure they are available on `PATH`:

- The current stable [Rust toolchain](https://rust-lang.org/tools/install/),
  including rustfmt and Clippy.
- Python 3.11–3.14; uv can
  [install a compatible Python](https://docs.astral.sh/uv/guides/install-python/)
  if needed.
- [uv](https://docs.astral.sh/uv/getting-started/installation/) for the Python
  environment and editable package build.
- [Just](https://just.systems/man/en/packages.html) for build and test commands.

Native build and runtime dependencies:

- A C/C++ compiler and `make` to build bundled FFmpeg.
- libclang to generate native bindings.
- `pkg-config` for library discovery.
- Git for the initial FFmpeg source fetch.
- **NASM** for FFmpeg's x86 assembly.
- `ffmpeg` and `ffprobe` for runtime encoding and media inspection.

On Ubuntu/Debian, install the native build tools and FFmpeg runtime before
running `uv sync`:

```bash
sudo apt-get update
sudo apt-get install -y build-essential clang libclang-dev \
  pkg-config nasm git ffmpeg
```

`build-essential` supplies the C/C++ compiler and `make`. Installing the `ffmpeg`
executable alone does not supply the build tools above. A build error such as
`nasm not found or too old` means NASM must be installed or updated.

On other platforms, install equivalent native tools with your platform's
package manager. From the repository root, build the editable package and
check the installation:

```bash
uv sync --locked --extra dev
uv run python -c "import vestra; print(vestra.__version__)"
just check
just python-test
```

See [testing](../development/testing.md) for focused contributor checks.

### Build the CLI

Build the separate CLI with Cargo:

```bash
cargo build -p vestra-cli
cargo run -q -p vestra-cli -- version
```

The binary is `target/debug/ve` (`ve.exe` on Windows). Add it to `PATH` if you
want to use the short CLI commands. Continue with the
[CLI quickstart](cli-quickstart.md) or [Python quickstart](python-quickstart.md).
