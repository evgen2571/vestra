---
description: Install Vestra, check the environment, and review supported Python versions and platforms.
---

# Installation

You need **Python 3.11–3.14** and Vestra. The Python package includes the Rust
engine and installs FFmpeg automatically through `imageio-ffmpeg`. A matching
wheel means you can start without installing Rust, a compiler or system FFmpeg.

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

<span id="3-check-the-environment"></span>

## 2. Check the environment

Run this command from your project directory:

```bash
uv run python -c "import vestra; print('Vestra', vestra.__version__)"
```

You should see a Vestra version. The next tutorial checks the whole pipeline
by encoding an actual MP4. No separate `ffmpeg` or `ffprobe` installation is
needed. If you need your own FFmpeg build, see the
[optional executable override](../getting-started/installation.md#choose-a-different-ffmpeg-executable).

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
builds need more than the FFmpeg executable.

## If a check fails

| What you see | What to check |
| --- | --- |
| `No module named vestra` | Run with the environment where you installed it; use `uv run python` inside your project. |
| Cannot run FFmpeg | Reinstall `imageio-ffmpeg` in this environment and check any `IMAGEIO_FFMPEG_EXE` override. |
| A native import or wheel error | Check CPython version/architecture, then [Python troubleshooting](../troubleshooting/python.md). |
| FFmpeg fails while encoding | Keep the error output and follow [FFmpeg troubleshooting](../troubleshooting/ffmpeg.md). |

[Continue to your first render](first-render.md){ .md-button .md-button--primary }
