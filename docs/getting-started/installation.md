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

Install FFmpeg with your platform's package manager. The `ffmpeg` executable
must be available on `PATH` in the shell that runs Vestra for encoding and
audio processing:

```bash
ffmpeg -version
```

This executable is required even though wheels bundle the native FFmpeg
libraries used for media probing and video decoding. A working Python import
does not check it. `ffprobe` is optional for independent media verification;
Vestra does not invoke it at runtime.
For failures, see [FFmpeg troubleshooting](../troubleshooting/ffmpeg.md).

## Supported Python versions and platforms

Vestra supports CPython 3.11–3.14. Published wheels cover Linux x86_64
and aarch64 (glibc 2.28+), macOS Intel and Apple Silicon, and Windows x64.
Wheels use CPython's stable ABI (`cp311-abi3`): the same platform wheel
supports all four versions. Standard CPython builds are supported; PyPy and
free-threaded CPython are not covered.

## Install the native CLI

Download the archive matching your machine from
[GitHub Releases](https://github.com/evgen2571/vestra/releases). The CLI runs
without Python; installing the Python wheel does not install `ve`.

| Platform | Archive target | Compatibility |
| --- | --- | --- |
| Linux Intel/AMD 64-bit | `x86_64-unknown-linux-gnu.tar.gz` | glibc 2.28+, not musl/Alpine |
| Linux ARM64 | `aarch64-unknown-linux-gnu.tar.gz` | glibc 2.28+ |
| macOS Intel | `x86_64-apple-darwin.tar.gz` | macOS 11.0+ |
| macOS Apple Silicon | `aarch64-apple-darwin.tar.gz` | macOS 11.0+ |
| Windows x64 | `x86_64-pc-windows-msvc.zip` | Windows 10+/Server 2022 tested |

Each filename starts with `ve-<version>-`. Download its `.sha256` sidecar too.
On Linux, verify and install (substitute the downloaded version):

```bash
sha256sum -c ve-<version>-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf ve-<version>-x86_64-unknown-linux-gnu.tar.gz
mkdir -p "$HOME/.local/bin"
chmod +x ve-<version>-x86_64-unknown-linux-gnu/ve
cp ve-<version>-x86_64-unknown-linux-gnu/ve "$HOME/.local/bin/ve"
ve version
```

Ensure `$HOME/.local/bin` is on PATH. Keep the extracted license files and
README. On macOS, compare `shasum -a 256 <archive>` with the sidecar, extract,
and install `ve` similarly. Binaries are not signed/notarized; follow your
organization's macOS security policy when using downloaded executables.

On Windows, compare `Get-FileHash <archive> -Algorithm SHA256` with the
sidecar, use `Expand-Archive <archive>`, and add the extracted folder containing
`ve.exe` to your user PATH. Open a new terminal and run `ve version`. Windows
may require the Microsoft Visual C++ 2015–2022 x64 Redistributable.

The native FFmpeg libraries are statically linked, but the `ffmpeg`
**executable remains an external requirement** for encoding and audio processing.
Linux also needs its standard C/C++ runtime libraries; macOS needs its system
frameworks. A GPU and compatible driver are needed only for WGPU rendering;
use `--render-backend cpu` for a first render. Release smoke coverage is Ubuntu
24.04, macOS 15 and Windows Server 2022; older compatible systems are not
exhaustively tested. Continue with the [CLI quickstart](cli-quickstart.md).

<a id="build-from-source--development"></a>

## Build from source / development

This section is for contributors and platforms without a matching wheel.
For contribution guidance, see [Contributing](https://github.com/evgen2571/vestra/blob/develop/CONTRIBUTING.md).

A source build compiles the Rust engine and bundled FFmpeg libraries. It needs
Rust, Python 3.11–3.14, uv, a C compiler, libclang, `make`, `git`, and NASM,
plus network access for the initial FFmpeg source fetch. The first build can
take a while. Runtime encoding and audio processing still need the `ffmpeg`
executable above.

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
- `ffmpeg` for runtime encoding and audio processing.

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
