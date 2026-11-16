---
description: Install the Python package, check the import, and continue to your first render.
---

# Install Vestra

<span id="install-into-a-python-project-with-uv"></span>

In your Python project, run:

```bash
uv add vestra
```

For a new project, run `uv init my-video` and `cd my-video` first.
Install [uv](https://docs.astral.sh/uv/getting-started/installation/) if needed.

<span id="install-into-a-python-project-with-uv"></span>
<span id="3-check-the-environment"></span>

<span id="check-the-environment"></span>

## Check the installation

```bash
uv run python -c "import vestra; print(vestra.__version__)"
```

This prints the installed version. Continue with
[your first render](first-render.md) to make a three-second MP4 on CPU; no GPU
or separate native tools are needed on the supported platforms below.

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
