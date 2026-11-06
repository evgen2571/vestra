---
description: Install Vestra and FFmpeg, check the environment, and review supported Python versions and platforms.
---

# Installation

You need **Python 3.11–3.14**, Vestra, and the **FFmpeg command-line tools**.
The Python package includes the Rust engine. A matching wheel means you can
start without installing Rust or a compiler.

## 1. Create a Python project

With [uv](https://docs.astral.sh/uv/getting-started/installation/), create a
project for the tutorials:

```bash
uv init vestra-playground
cd vestra-playground
uv add vestra
```

We use `uv run python` throughout the tutorials so each command runs in this
project's environment.

??? note "Install with pip"

    Create and activate a virtual environment first:

    ```bash
    python -m venv .venv
    ```

    On macOS and Linux:

    ```bash
    source .venv/bin/activate
    ```

    On Windows PowerShell:

    ```powershell
    .venv\Scripts\Activate.ps1
    ```

    Then install Vestra:

    ```bash
    python -m pip install vestra
    ```

    Use `python` wherever the tutorials say `uv run python`.

## 2. Install FFmpeg

Vestra uses the `ffmpeg` executable to encode video and `ffprobe` to inspect
media. **Both must be on `PATH` in the terminal running your script.** A
successful `import vestra` does not check this.

=== "macOS"

    With [Homebrew](https://brew.sh/):

    ```bash
    brew install ffmpeg
    ```

=== "Ubuntu / Debian"

    ```bash
    sudo apt update
    sudo apt install ffmpeg
    ```

    Other Linux distributions provide FFmpeg through their package manager.

=== "Windows"

    With [WinGet](https://learn.microsoft.com/windows/package-manager/winget/):

    ```powershell
    winget install --id Gyan.FFmpeg --exact
    ```

    Open a **new terminal** after installation so it picks up changes to
    `PATH`. You can also install a Windows build linked from
    [FFmpeg's download page](https://ffmpeg.org/download.html) and add the
    directory containing `ffmpeg.exe` and `ffprobe.exe` to `PATH` yourself.

The executables are separate from the native FFmpeg libraries bundled in the
Vestra wheel; you need them even when you installed a wheel.

## 3. Check the environment

Run all three commands from your project directory:

```bash
uv run python -c "import vestra; print('Vestra', vestra.__version__)"
ffmpeg -version
ffprobe -version
```

You should see a Vestra version and version information for **each** FFmpeg
tool. The next tutorial checks the whole pipeline by encoding an actual MP4.

!!! tip "CPU rendering"

    These tutorials select `backend="cpu"` explicitly. You do not need a
    graphics adapter or WGPU setup for your first videos. See
    [rendering and preparation](../guides/python/rendering-and-preparation.md)
    when you are ready to choose a backend.

## Which platforms have wheels?

Release builds target these platforms for CPython 3.11–3.14:

| System | Architecture | Detail |
| --- | --- | --- |
| Linux | x86_64, aarch64 | glibc 2.28 or newer; not musl/Alpine wheels |
| macOS | Intel, Apple Silicon | Install with the Python architecture you intend to use |
| Windows | x64 | Use 64-bit CPython |

A wheel must match your Python version, operating system and architecture.
See the [release artifacts](https://github.com/evgen2571/vestra/releases) for
available versions. The separately built `ve` CLI is not installed by the
Python package and is not needed for these tutorials.

If pip starts compiling Rust or FFmpeg, it did not find a compatible wheel.
Check your Python version and architecture before starting a source build.
To require a wheel and get a clear error instead of compiling:

```bash
python -m pip install --only-binary=vestra vestra
```

For an intentional source build, use the existing
[development installation instructions](../getting-started/installation.md#build-from-source--development).
They cover Nix, native build dependencies and the editable package. Source
builds need more than the FFmpeg executables.

## If a check fails

| What you see | What to check |
| --- | --- |
| `No module named vestra` | Run with the environment where you installed it; use `uv run python` inside your project. |
| `ffmpeg` or `ffprobe` not found | Reopen the terminal, check `PATH`, and run both version commands again. |
| A native import or wheel error | Check CPython version/architecture, then [Python troubleshooting](../troubleshooting/python.md). |
| FFmpeg fails while encoding | Keep the error output and follow [FFmpeg troubleshooting](../troubleshooting/ffmpeg.md). |

[Continue to your first render](first-render.md){ .md-button .md-button--primary }
