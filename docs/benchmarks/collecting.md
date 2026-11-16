---
title: Run benchmarks
description: Set up native tools, collect validated benchmark results, and inspect or share a run.
---

# Run benchmarks

The collector builds this checkout, measures Vestra CPU and MoviePy CPU renders,
and adds hardware WGPU results when an eligible adapter is available. It checks
the rendered outputs and writes a readable `summary.md`, raw records and logs
under `target/benchmark-results/`. A sanitized ZIP beside the run directory is
the file to share; nothing is uploaded automatically.

The default collection runs these short suites. Hour-long output and decoder
cache experiments are optional.

| Stage | Workloads | Settings |
| --- | --- | --- |
| Vestra CPU smoke | `single_video`, `mixed_dynamic`, `video_heavy`, `combined`, `masks`, `mattes`, `nested_groups`, `particles`, `blend_modes`, `production_edit` | 128×72 / 60 fps; no warmup, one sample |
| Vestra CPU community | `single_video`, `mixed_dynamic`, `video_heavy`, `masks`, `mattes`, `production_edit` | 1920×1080 / 60 fps; one warmup, three samples |
| Hardware WGPU community, if available | The same six community workloads | The same community settings; separate hardware records |
| MoviePy comparison | `basic` (10 s), `editing` (12 s), `layered` (10 s), through Vestra CPU, MoviePy CPU and eligible hardware WGPU | 1920×1080 / 60 fps; one warmup, three samples per engine/workload |

See the [workload gallery](workloads.md) for scenes and definitions. The forest,
valley and tear clips are bundled in `assets/footage/` and resized automatically;
you do not need to collect or download footage separately.

<span id="quick-start"></span>
<span id="disk-memory-and-duration"></span>

## Before you start

| Resource | Plan for |
| --- | --- |
| Time | The first native build can take tens of minutes. Repeated 1080p60 measurements can take an hour or more, depending on CPU, GPU and cooling. |
| Disk | At least 6 GiB free for builds, plus Cargo/uv caches and temporary media. This minimum is checked before building; it is not a total-space guarantee. Endurance runs need much more space, as listed [beside their commands](#optional-profiles). |
| Memory | At least 2 GiB; 8 GiB or more is recommended. Decoder scratch, caches and allocator retention can still exhaust a low-memory host. |
| Network | Initial tool, Python, package, Cargo and FFmpeg-source downloads need internet access. Useful downloads remain cached for retries. |
| Measurement conditions | Plug in the computer, keep the power mode consistent, close heavy applications and leave the checkout unchanged throughout a run. |

## Install tools once

Choose one OS route below, then follow [Run the suites](#run-the-suites).
The collector manages Python 3.13 and a separate `target/benchmark-env`
environment, installs the locked benchmark dependencies and builds a fresh
non-editable Vestra wheel from this checkout. You do not need a separate Python
installation, manual venv or package installs. It keeps Cargo's incremental
cache; a development extension already installed elsewhere cannot replace this
build.

Source builds download native FFmpeg libraries themselves. System FFmpeg
development libraries are not required; the command-line encoders below are
needed for fixtures and verification.

### Windows x64

Install:

- [uv](https://docs.astral.sh/uv/getting-started/installation/) using its Windows installer.
- [Rust](https://rustup.rs/) stable, with the `x86_64-pc-windows-msvc` host toolchain.
- Visual Studio 2022 Build Tools: **Desktop development with C++**, MSVC x64 compiler and Windows SDK.
- LLVM, including libclang for bindgen.
- Git for Windows, including its Unix shell tools (`sh`, `sed`, `awk`, `diff`, `tr`).
- GNU Make and NASM for the native FFmpeg build.
- FFmpeg, including FFprobe and functioning libx264 and FFV1 encoders.

After installation, open a new **x64 Native Tools Command Prompt for VS 2022**
from the Start menu. Run the remaining commands in that terminal; ordinary
PowerShell and Git Bash do not initialize the compiler/SDK environment.
Keep MSVC's `link.exe` ahead of Git's unrelated executable of the same name.

For standard installation locations, add any missing directories to this
terminal's PATH (adjust if installed elsewhere):

```bat
set "PATH=%PATH%;C:\Program Files\Git\bin;C:\Program Files\Git\usr\bin;C:\Program Files\NASM;C:\Program Files\LLVM\bin"
set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
```

### macOS (Apple Silicon or Intel)

Install:

- Xcode Command Line Tools for the C/C++ compiler, SDK and Make.
- [Rust](https://rustup.rs/) stable, [uv](https://docs.astral.sh/uv/getting-started/installation/) and Git.
- NASM, LLVM/libclang and pkg-config.
- FFmpeg with FFprobe and functioning libx264 and FFV1 encoders.

With [Homebrew](https://brew.sh/), use:

```bash
xcode-select --install
brew install rust uv git nasm llvm ffmpeg pkg-config
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
```

LLVM supplies libclang for bindgen. Metal hardware availability is established
by actual Vestra adapter discovery.

### Linux

Install:

- [uv](https://docs.astral.sh/uv/getting-started/installation/) and [Rust](https://rustup.rs/) stable.
- A C/C++ compiler and linker, GNU Make and NASM.
- Clang/libclang, pkg-config and Git.
- FFmpeg with FFprobe and functioning libx264 and FFV1 encoders.
- For hardware Vulkan measurements, your vendor's supported driver, Vulkan loader and hardware ICD. A loader alone cannot provide a hardware adapter.

On Debian/Ubuntu, the native packages are
`build-essential clang libclang-dev pkg-config nasm ffmpeg git`; NumPy wheels
may also need `libstdc++6`.

On Nix systems, `nix develop .#comparison` supplies the build tools and
wheel/graphics runtime libraries instead of the manual package setup. Nix and
Just are optional. The software-WGPU shell selects a software adapter and cannot
provide hardware performance results. If portable Python cannot load extensions
against the shell's newer libc, use the [Nix interpreter fix](#troubleshooting-and-privacy).

## Run the suites

After installing tools, use these steps in the Windows developer terminal,
macOS Terminal or a Linux shell. The preflight check comes before any project
dependency installation or expensive build.

1. **Check out the repository.**

    ```bash
    git clone https://github.com/evgen2571/vestra.git
    cd vestra
    ```

2. **Choose the revision.** Replace `COMMIT` with the exact revision to measure;
   use the same revision on every computer in a comparison.

    ```bash
    git checkout COMMIT
    git status --short
    ```

    Use a clean checkout. Keep its source files unchanged until collection ends.

3. **Check setup.**

    ```bash
    uv run --no-project --python 3.13 benchmarks/collect.py --preflight-only
    ```

    Every check should report `PASSED`. This tests tools, encoders, disk space,
    RAM, a loadable libclang, and compiling/linking/running small C and C++
    programs. It writes its own result directory, including `preflight.json`
    and diagnostics, without installing project dependencies or benchmarking.
    Resolve failures with the guidance printed beside each check or
    [troubleshooting](#troubleshooting-and-privacy), then rerun this step.

4. **Collect results.**

    ```bash
    uv run --no-project --python 3.13 benchmarks/collect.py
    ```

    Each invocation creates a unique child of `target/benchmark-results/`;
    it does not replace a previous capture. `--output DIRECTORY` changes that
    parent. The terminal reports task duration and activity every fifteen
    seconds while workers run. Ctrl+C terminates owned worker/FFmpeg trees,
    removes incomplete media and preserves partial diagnostics.

5. **Open the summary and keep the ZIP.** The final terminal output prints the
   ZIP path. Read `summary.md` in the corresponding run directory or archive
   for execution/validation status, revision, actual hardware and render times.
   Failed runs still retain completed results and useful diagnostics.

Exit code `0` means required requested work completed, `1` means failure and
`130` means cancellation. Check individual task statuses: optional unavailable
hardware is reported as unsupported.

## Read and share results

A default run has this layout; individual files or hardware directories can be
absent when a step fails or is unsupported:

```text
target/benchmark-results/
├── vestra-bench-<os>-<timestamp>-<id>.zip  # share this archive
└── vestra-bench-<os>-<timestamp>-<id>/
    ├── summary.md                       # start here: status and timings
    ├── manifest.json                    # revision, source hash, settings, tasks
    ├── system.json                      # machine, OS and tool/package versions
    ├── preflight.json                   # native setup checks
    ├── input-manifest.json              # prepared input identity
    ├── diagnostics/                     # worker/build logs and measurements
    ├── vestra/<backend>/<suite>/         # suite.json, scenario records and logs
    ├── moviepy/<engine>/                # comparison.json and worker records
    ├── moviepy/verification/            # visual comparison evidence
    └── local/                           # local fixture/build scratch; never shared
```

Read the summary's validation statuses before interpreting its timing tables.
Keep raw records when comparing samples or investigating a failure. Optional
profiles add `cache/results.json` and `sustained/<engine>/result.json` with
resource time series.

Share the ZIP, including for failed runs. It excludes media, images, binaries
and local scratch, and sanitizes private paths and credentials. Original local
logs and project snapshots can contain absolute paths; review any extra files
you choose to share. No telemetry or upload occurs.

## Choose CPU or hardware measurements

For the default profile:

| Selection | CPU evidence | Hardware requirement |
| --- | --- | --- |
| Default (`--backend auto`) | Always retained | Adds WGPU only after integrated/discrete adapter discovery; unavailable hardware is unsupported. |
| `--backend cpu` | CPU suites and MoviePy CPU only | No GPU discovery or WGPU measurement. |
| `--backend hardware-wgpu` | CPU measurements still run | Missing eligible hardware is a failure. Each hardware result must confirm actual hardware execution. |

Use `--graphics-api vulkan`, `metal`, `dx12` or `gl` to constrain discovery.
Software, virtual, unknown or failed WGPU results cannot replace CPU results.
WGPU rendering still uses CPU decoding and H264 encoding. See
[GPU validation](../development/gpu-validation.md) for adapter evidence.

## Optional profiles

**Endurance profiles can run for many hours.** Reserve about 36 GiB per hour
of output for the active engine, multiplied by retained engines when using
`--retain-media`; two-hour output doubles that reservation.

Append the listed arguments to
`uv run --no-project --python 3.13 benchmarks/collect.py`.
Scroll the table horizontally on narrow screens.

<div class="wide-table" markdown="1">

| Purpose | Arguments | Runtime, disk and hardware implications |
| --- | --- | --- |
| Decoder/cache access | `--profile cache` | CPU only. Builds and checks a real two-hour 640×360 / 30 fps source; does not render two hours of output. Random access can reach worker timeouts. |
| One-hour endurance | `--profile sustained --hours 1` | **Potentially many hours.** Serial Vestra CPU, MoviePy CPU and eligible hardware WGPU renders, each 108,000 frames at 640×360 / 30 fps. Reserve about 36 GiB for the active output, multiplied by retained engines with `--retain-media`. |
| All short, cache and two-hour runs | `--profile all --hours 2` | **Potentially many hours; 216,000 output frames per engine.** Endurance disk reservation doubles; hardware is added when eligible. |
| Small integration check | `--profile all --smoke --backend cpu` | Still builds and validates a two-hour 128×72 source, but sustained outputs last three seconds. Timings prove execution only. |

</div>

For endurance, the conservative reservation uses raw YUV output size; actual
H264 files are usually smaller. The collector keeps a 1 GiB free-space reserve
and stops if space runs low. Without `--retain-media`, engines run serially and
completed outputs are removed after verification. Rendering may be slower than
real time. Neither smoke nor an ordinary short run establishes multi-hour
stability, files larger than 4 GiB or constant memory for audio-heavy projects.
No endurance run is required in CI.

Other controls:

| Option | Effect / default |
| --- | --- |
| `--only cpu moviepy` | Selects tasks within the chosen profile. |
| `--suite canonical` | Replaces the community suite with all ten native workloads at 1080p60, one warmup and five samples. |
| `--timeout SECONDS` | Per short suite/worker limit; 7,200 seconds by default. |
| `--build-timeout SECONDS` | Build limit; 7,200 seconds by default. |
| `--sustained-timeout SECONDS` | Per-engine endurance limit; 14,400 seconds by default. Increase limits on slower hosts. |
| `--retain-media` | Keeps validated inputs and completed MoviePy/sustained media locally. Canonical Rust outputs and the cache placement companion stay temporary; media is always excluded from the ZIP. |

`--smoke` keeps the MoviePy comparison canvas at 1080p60 and reduces warmups and
samples. Smaller canvases make detailed footage overly sensitive to known
rounding/resampling differences under the existing visual tolerance. Cache and
sustained smoke instead share a 128×72 two-hour source to bound decoding cost.

## What the results mean

Builds, dependency installation, fixture preparation and validation sit outside
render intervals. MoviePy comparisons use normal public APIs, identical prepared
inputs and output settings: silent H264/libx264, CRF 23, veryfast. Decoded
frame/region checks establish representative visual equivalence; rounding,
resampling and mask edges need not match pixel for pixel. Failed or missing
visual checks prohibit a fully validated performance claim.

Process wall time includes startup, imports, project construction and
publication. Vestra stage timings have different scopes from MoviePy's
`write_videofile` timing, and stages can overlap. Use the
[measurement methodology](../development/performance.md#measurement-scope)
before comparing them. Repeat runs when differences are small. The
[same-machine comparator](../development/performance.md#measure-and-compare)
is authoritative for compatible captures; cross-machine results describe hosts
and cannot establish a code regression.

For specialist interpretation, use the collector's
[cache/decoder methodology](../development/performance-probes.md#collector-cache-and-decoder-methodology)
and [resource sampling limits](../development/performance-probes.md#collector-resource-sampling).

## Troubleshooting and privacy

| Symptom | Check | Fix | Confirm |
| --- | --- | --- | --- |
| Preflight reports a missing tool or encoder | Read the failed row in `preflight.json`. | Install the matching OS prerequisite and restart the terminal so PATH updates. | Rerun `--preflight-only`; all checks pass. |
| Windows compiler/linker or SDK check fails | Check the terminal title and MSVC linker order. | Use the x64 developer terminal from the [Windows setup](#windows-x64). | Preflight compiles, links and runs both C and C++ programs. |
| libclang check fails despite a working `clang` command | Check the `libclang` row; the library must load, not just the executable. | Set `LIBCLANG_PATH` to the installed LLVM library directory. | The libclang preflight row passes. |
| A native extension fails to load in a Nix shell | Inspect `diagnostics/uv-sync.log` for a libc mismatch. | Use the shell interpreter as shown below. | Preflight and the environment-build task pass with that interpreter. |
| Downloads or builds fail | Read `diagnostics/uv-dependencies.log`, `uv-sync.log` or the failed build log. | Repair network/proxy access or the native setup, then rerun. Cached downloads remain useful. | The failed task passes in the new summary. |
| Hardware is unsupported or discovery fails | Read `diagnostics/adapters.log` and the hardware task status. | Install a supported driver/ICD or select an available graphics API; use CPU measurements when no eligible adapter exists. | The summary records the actual integrated/discrete adapter for successful hardware samples. |
| A worker times out or runs out of disk | Check its log and `.measurement.json` for `timed_out` or `disk_exhausted`. | Increase the relevant timeout or free the required space; expensive long-source access is expected for some patterns. | The rerun completes the task and its output validation. |
| Source stability or visual validation fails | Read the named task and verification logs. | Restore a clean, fixed revision; investigate output differences before using the timings. | The new run keeps its revision/source hash fixed and passes required output validation. |

For the Nix interpreter fix, replace the Python selection in each collector
command with the shell's interpreter:

```bash
uv run --no-project --python "$(command -v python3)" benchmarks/collect.py --preflight-only
```

For an unresolved failure, include the sanitized ZIP, exact command, revision,
OS and any native setup changes. Windows/macOS execution still requires testing
on those platforms; mocked setup checks cannot establish native build or
rendering success.
