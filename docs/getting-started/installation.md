---
description: Install the Python package, check the import, and continue to your first render.
---

# Install Vestra

<span id="install-into-a-python-project-with-uv"></span>

Install [uv](https://docs.astral.sh/uv/getting-started/installation/) and use
Python 3.11–3.14. In an existing Python project, run:

```bash
uv add vestra
```

Starting from an empty directory? Create a project first:

```bash
uv init my-video
cd my-video
uv add vestra
```

<span id="3-check-the-environment"></span>
<span id="check-the-environment"></span>

## Check the installation

```bash
uv run python -c "import vestra; print(vestra.__version__)"
```

Vestra 0.1.5 and later install the FFmpeg executable as a Python dependency.
For 0.1.4 or earlier, install FFmpeg separately and make sure both
`ffmpeg` and `ffprobe` are on your `PATH`.

Once the import works and FFmpeg is available,
[render your first video](first-render.md). The tutorial runs on the CPU.

<span id="install-with-pip"></span>

## Alternative: pip

Use an activated virtual environment, then install and check with the same
interpreter:

```bash
python -m pip install vestra
python -c "import vestra; print(vestra.__version__)"
```

Use `python` wherever the tutorials say `uv run python`.

If you need a virtual environment, create one with `python -m venv .venv`.
Activate it with `source .venv/bin/activate` on macOS/Linux or
`.venv\Scripts\Activate.ps1` in Windows PowerShell before installing.

## Supported Python versions and platforms

| Requirement | Supported |
| --- | --- |
| Interpreter | Standard CPython 3.11–3.14; PyPy and free-threaded CPython are not covered. |
| Linux | x86_64 and aarch64, glibc 2.28+; not musl/Alpine. |
| macOS | Intel and Apple Silicon. |
| Windows | x64. |

Published wheels use CPython's stable ABI (`cp311-abi3`): one wheel per platform
supports all four Python versions. Wheel installs need no compiler.

## If installation fails

Start with [Python troubleshooting](../troubleshooting/python.md) for an
installed-package problem. If no wheel matches your interpreter or platform,
installers may attempt a source build: use the advanced
[source-build route](../development/source-builds.md) for its native prerequisites.

## Other installation routes

<span id="install-the-native-cli"></span>

- For terminal rendering of JSON, [install the separate CLI](cli-installation.md),
  then follow the [CLI quickstart](cli-quickstart.md).

<span id="build-from-source-development"></span>
<span id="build-from-source--development"></span>
<span id="with-nix-linux-and-macos"></span>
<span id="without-nix"></span>
<span id="build-the-cli"></span>

- For a development checkout, [build from source](../development/source-builds.md).
  That page covers Nix, native compiler prerequisites and CLI builds.

<span id="choose-a-different-ffmpeg-executable"></span>

- To select an executable for an existing install, use
  [advanced FFmpeg configuration](../reference/environment-variables.md#choose-a-different-ffmpeg-executable).
