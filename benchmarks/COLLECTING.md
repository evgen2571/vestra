# Collect benchmark results

The collector builds this checkout, runs benchmarks, checks their output, and
creates **one ZIP to send to the maintainer**. It never uploads anything.
The first build may take tens of minutes. Keep the computer plugged in, use a
consistent power mode, and close other heavy applications during measurements.

## Install tools once

Install [uv](https://docs.astral.sh/uv/getting-started/installation/) using its
Windows installer or macOS/Linux instructions. uv downloads Python 3.13 for you:
**you do not need a separate Python installation or a manually created venv**.
Install [Rust](https://rustup.rs/) using the stable toolchain, and Git.

uv manages Vestra's Python extension, its Maturin build backend, MoviePy v2,
NumPy, Pillow, psutil and their
locked dependencies. It cannot install native compilers, drivers or FFmpeg.
Source builds download native FFmpeg libraries themselves; **system FFmpeg
development libraries are not required**. Initial builds need internet access.

### Windows x64

Install Visual Studio 2022 Build Tools with **Desktop development with C++**,
the MSVC x64 compiler, and a Windows SDK. Install LLVM (including libclang),
Git for Windows, GNU Make, NASM, and FFmpeg with libx264/FFprobe. Chocolatey
users can install `make nasm ffmpeg llvm git`; package installation may require
an administrator terminal. The collector itself makes no system changes.

Open **x64 Native Tools Command Prompt for VS 2022** from the Start menu.
Run all commands below there. A normal PowerShell window does not initialize
the compiler/SDK environment. Git Bash alone is not a C++ build environment.
Ensure `cl`, `link`, `make`, `nasm`, `sh`, `sed`, `awk`, `diff`, `tr`, `cargo`,
`rustc`, `uv`, `ffmpeg` and `ffprobe` are on PATH. Keep MSVC's `link.exe` ahead of
Git's unrelated `link.exe`. Rust must use `x86_64-pc-windows-msvc`.

For standard installation locations, these cmd commands add missing tools to
this terminal only (adjust if you installed elsewhere):

```bat
set "PATH=%PATH%;C:\Program Files\Git\bin;C:\Program Files\Git\usr\bin;C:\Program Files\NASM;C:\Program Files\LLVM\bin"
set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
```

Restart the developer terminal after installing tools so PATH includes them.
The collector compiles and links small C and C++ programs before building Vestra.

### macOS (Apple Silicon or Intel)

Install Xcode Command Line Tools with `xcode-select --install`. Install Make,
NASM, LLVM/libclang and FFmpeg/FFprobe with libx264. Homebrew is convenient but
optional:

```bash
brew install rust uv git nasm llvm ffmpeg pkg-config
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
```

Apple Clang and the SDK come from Xcode tools. LLVM supplies libclang for bindgen.
Metal hardware availability is established by actual Vestra adapter discovery.

### Linux

Install a working C/C++ compiler and linker, GNU Make, NASM, Clang/libclang,
pkg-config and FFmpeg/FFprobe with libx264. On Debian/Ubuntu, the native packages
are `build-essential clang libclang-dev pkg-config nasm ffmpeg git`. Install Rust
and uv as described above. NumPy wheels may also need the C++ runtime (`libstdc++6`).
Install your GPU vendor's supported driver, Vulkan loader and hardware ICD for
hardware Vulkan measurements. A Vulkan loader alone is insufficient.

The existing `nix develop .#comparison` environment supplies build tools and
wheel/graphics runtime libraries on Nix systems. Nix and Just are optional.
If uv's portable Python cannot load a native extension built against the Nix
shell's newer libc, select that shell's interpreter with
`uv run --no-project --python "$(command -v python3)" scripts/collect-benchmarks.py`.
uv still manages the benchmark environment and all Python packages.
The software-WGPU Nix shell deliberately selects a software adapter and cannot
provide hardware performance results.

## Collect and share

The same command works in the Windows developer terminal, macOS Terminal and
a Linux shell after native tools are installed:

```bash
git clone -b develop https://github.com/evgen2571/vestra.git
cd vestra
uv run --no-project --python 3.13 scripts/collect-benchmarks.py
```

Until this implementation is merged, testers should clone with
`-b feat/portable-benchmarks` instead of `-b develop` once the maintainer makes
that branch available. The collector is currently under review on that branch.

Default collection runs the established ten-workload CPU smoke, a versioned
640×360 community suite (six representative workloads, one warmup and three
samples), all three MoviePy comparisons, and a separate hardware WGPU suite
when Vestra discovers an eligible adapter. Shared moving footage is generated
automatically. It does not use a personal footage directory.
Portable patterns are generated at twice the input resolution and downsampled
with bicubic filtering and a one-pixel Gaussian filter to retain motion/detail without single-pixel aliasing
dominating the comparison between different render resamplers.

At the end, the terminal prints an archive under `target/benchmark-results/`,
for example `vestra-bench-linux-20261029T120000Z-1234abcd.zip`. **Send that ZIP**,
not the entire directory. Read `summary.md` inside it for actual pass/fail/skip
statuses. Failures still produce archives and retain successful results.
Exit 0 means required requested work completed; failures return 1, cancellation
returns 130. Unavailable optional hardware is unsupported; explicitly requesting
`--backend hardware-wgpu` makes missing eligible hardware a failure.

## Other profiles

```bash
# Long-source cache access only; does not render two hours of output.
uv run --no-project --python 3.13 scripts/collect-benchmarks.py --profile cache

# Actual one-hour renders, serially through Vestra CPU and MoviePy CPU, plus
# eligible hardware WGPU. --hours 2 requests two hours per engine.
uv run --no-project --python 3.13 scripts/collect-benchmarks.py --profile sustained --hours 1

# All short suites, cache experiments and explicitly requested two-hour output.
uv run --no-project --python 3.13 scripts/collect-benchmarks.py --profile all --hours 2

# Small integration check, including a real two-hour low-resolution source and
# three-second sustained outputs. These timings are not performance baselines.
uv run --no-project --python 3.13 scripts/collect-benchmarks.py --profile all --smoke --backend cpu

# Check native setup without installing project dependencies or benchmarking.
uv run --no-project --python 3.13 scripts/collect-benchmarks.py --preflight-only
```

Use `--output DIRECTORY` to choose the parent result folder; every invocation
creates a unique child. `--backend cpu` disables hardware testing.
`--graphics-api vulkan`, `metal`, `dx12`, or `gl` selects an API for discovery.
Only an actual integrated/discrete adapter qualifies. Software, virtual, unknown
or failed WGPU results never become CPU replacements. CPU evidence is preserved.

`--only cpu moviepy` selects tasks within the chosen profile; `--suite canonical`
uses the original larger CPU suite. `--timeout` bounds each short suite/worker;
`--build-timeout` defaults to two hours, `--sustained-timeout` to four hours per
engine. Increase them on slower computers. `--retain-media` retains validated
inputs and completed MoviePy/sustained media locally; it is always excluded
from the ZIP. Canonical Rust benchmark outputs and the cache placement companion
remain temporary. Ctrl+C terminates owned
worker/FFmpeg trees, removes incomplete media, and packages partial diagnostics.

Smoke keeps the 640×360 MoviePy comparison canvas and reduces warmups/samples.
Its dense moving pattern makes smaller canvases overly sensitive to the engines'
known rounding and resampling differences under the existing visual tolerance.
Cache and sustained smoke use a separate 128×72 two-hour source to bound decoding
cost; the native cache A/B and MoviePy access workers share that exact source.

## Disk, memory and duration

Allow at least 6 GiB for native builds and more for Cargo/uv caches. Default
media is small and removed after verification unless retained. A two-hour
source repeats a detailed moving 12-second H264 segment with stream copying;
it needs roughly 600 times the segment size, not a two-hour encode. Preparation
decodes all presentation timestamps and checks stream-copy boundaries and
early/middle/late seek pixels. This takes time even without rendering output.

Sustained output is 640×360 / 30 fps, silent, with two moving placements,
opacity, brightness and animated positioning. One hour means **108,000 frames**;
two hours means **216,000 frames**, for each engine. Before execution the
collector reserves a conservative raw-YUV-sized disk estimate (about 36 GiB
per hour per retained engine); actual H264 files are usually much smaller.
It checks free disk during execution and stops below a 1 GiB reserve.
Rendering may run slower than real time. An all/two-hour run can take many hours.

Decoder/worker scratch, bounded caches and allocator retention consume RAM;
low-memory hosts may fail. This workload avoids duration-dependent audio
analysis, but does not establish that arbitrary audio-heavy projects have
constant memory. A three-second smoke is not evidence of multi-hour stability
or files larger than 4 GiB. No endurance runs are required in CI.

## What the results mean

Builds, dependency installation, fixture generation and output verification
are outside render intervals. MoviePy comparisons use the existing `basic`,
`editing` and `layered` definitions and normal public APIs, identical prepared
inputs, output FPS/resolution/duration, silent H264/libx264 CRF 23/veryfast.
Vestra hardware rendering still decodes and encodes on the CPU. Resampling,
rounding and mask edges differ intentionally; decoded frame/region checks are
representative visual equivalence, not exact pixel equality. Failed or missing
visual checks prohibit a fully validated performance claim.

Process wall time includes worker startup, imports, project construction and
publication. Raw Vestra stage timings have different scopes from MoviePy's
`write_videofile` timing; do not equate them or sum overlapping stages.
Compare distributions and repeat runs for small differences. The original
same-machine regression comparator remains authoritative for compatible runs;
cross-machine results cannot establish a code regression.

Cache tests use a single native worker and the existing isolated decoded-RGBA
cache option, with zero vs eight-frame budgets per decoder. Static/bitmap/crop
caches are not disabled or attributed to it. Exact bytes are compared with a
cache-disabled reference in a separate correctness pass. Sequential access,
same-frame holds, local backward reuse, seeded random access, deliberate budget
overflow/revisits, and two decoder sessions at different source offsets use
fixed request sequences. The access matrix measures isolated session reuse. A separate three-second
public-API encoded render places the same two-hour source concurrently at
offsets zero and six seconds to exercise the CPU worker-pool placement policy.
Its wall time is kept separate from decoder-access A/B measurements.

Forward jumps in the current native decoder decode intervening frames; backward
requests seek. Random long-source access can therefore be expensive and can
hit the worker timeout. The collector measures this behavior without changing
the engine. Same-frame retention works even with zero LRU budget; low sequential
cache-hit counts are expected. MoviePy public `get_frame` latency and process
memory are separate observations; no equivalent cache/decode counters are
invented. Native eviction counts are unavailable; budget overflow is explicit
in the sequence. MoviePy reports initial VideoFileClip opens separately; internal
decoder/process reopen counts are unavailable.
Cache correctness, A/B benefit for a pattern, and MoviePy
end-to-end access latency are three separate questions.

psutil samples simultaneous process-tree RSS, including observed FFmpeg
descendants. Shared pages may be counted multiple times; RSS is not VRAM.
Peaks are sampled lower bounds. CPU is an observed-tree estimate with one core
at 100%; short-lived children may be missed. Missing metrics remain null.
Sustained sampling uses five-second intervals (100 milliseconds in smoke), progress
where exposed, output size and file-handle counts. Live decoder/cache metrics
are unavailable; final Vestra counters are retained. Memory growth alone does
not prove a leak: compare early/steady/late throughput, buffering, allocator
retention and cache warmup.

## Troubleshooting and privacy

Missing tools are listed before expensive work, with setup guidance. Inspect
`preflight.json` and `diagnostics/` inside a failed archive. Compiler checks
catch missing SDK/linker environments; libclang needs a loadable library,
not just a `clang` executable. If downloads fail, fix network/proxy access and
retry; useful existing Cargo/uv downloads are cached. The collector's managed
Python environment is `target/benchmark-env`, separate from your development
environment. It always synchronizes the locked `benchmark` extra and rebuilds
Vestra from this checkout; no manual package installs are needed.
It first installs locked dependencies without the project, then builds Vestra
using that environment's pinned Maturin backend. Each collection builds a fresh
non-editable wheel, retaining Cargo's incremental cache. Development builds in
the checkout cannot replace the collector's installed native extension.

The archive allowlists JSON/JSONL/Markdown/log evidence, sanitizes home/private
paths and credential-like fields, drops FFprobe tags and unapproved environment
variables, and excludes media, images, binaries and local scratch. Review it
before sharing, especially when using optional local footage with `--assets`.
Original local logs/project snapshots may still contain absolute paths; only
the ZIP is prepared for sharing. No full environment dump or automatic upload
is collected. Native Windows/macOS execution requires platform testing; mocked
platform checks do not establish native build or rendering success.
